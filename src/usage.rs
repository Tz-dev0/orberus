//! Where the usage of a profile went: how much each working folder
//! consumed, and whether today's rules would have sent that folder to this
//! profile.
//!
//! The figures come from the conversation files Claude Code keeps in
//! `projects/`. Claude Code's documentation says their format is internal and
//! can change with any release, so everything here is read defensively: a
//! line that does not look as expected is skipped, never an error. The
//! result is an estimate for this machine only; usage made on claude.ai or on
//! another computer is not in these files.
//!
//! Read-only, and never on the shim's path.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;
use serde::Deserialize;

use crate::config::{self, Loaded};
use crate::paths::Env;
use crate::resolve::{self, Reason};

/// The token counts of one or many answers.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
}

impl Tokens {
    /// One number to compare folders with.
    ///
    /// The four kinds of tokens do not weigh the same: a token written by
    /// the model costs far more than one read back from the cache. The
    /// factors follow the relative prices of the API (output 5, cache write
    /// 1.25, cache read 0.1, for input 1). A subscription does not publish
    /// how it counts, so this is an approximation, good for shares between
    /// folders and not for an amount.
    pub fn weight(&self) -> f64 {
        self.input as f64
            + self.output as f64 * 5.0
            + self.cache_write as f64 * 1.25
            + self.cache_read as f64 * 0.1
    }

    fn add(&mut self, other: &Tokens) {
        self.input += other.input;
        self.output += other.output;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
    }

    /// Keeps, field by field, the larger of the two. The same answer can be
    /// written several times while it streams, with counts that only grow.
    fn max(&mut self, other: &Tokens) {
        self.input = self.input.max(other.input);
        self.output = self.output.max(other.output);
        self.cache_write = self.cache_write.max(other.cache_write);
        self.cache_read = self.cache_read.max(other.cache_read);
    }
}

/// What one working folder consumed in a profile.
#[derive(Debug, Clone)]
pub struct FolderUsage {
    pub folder: PathBuf,
    pub tokens: Tokens,
    /// Answers from Claude, each counted once.
    pub answers: u64,
}

/// What one conversation consumed, its subagents included.
#[derive(Debug, Clone)]
pub struct ConversationUsage {
    /// The session id, as `claude --resume` takes it.
    pub id: String,
    /// Its main file. It may be gone when only subagent files are left.
    pub transcript: PathBuf,
    /// The folder of its latest answer.
    pub folder: PathBuf,
    pub tokens: Tokens,
    pub answers: u64,
    /// Tokens sent to the model with each answer, on average: the whole
    /// conversation so far, read again every time. Subagents are left out,
    /// since each starts from a context of its own.
    pub context: u64,
}

/// The usage of one profile over a period, heaviest first in both lists.
#[derive(Debug, Default)]
pub struct ProfileUsage {
    pub folders: Vec<FolderUsage>,
    pub conversations: Vec<ConversationUsage>,
    pub total: Tokens,
}

impl Tokens {
    /// What was sent to the model: everything but the answer itself.
    pub fn context(&self) -> u64 {
        self.input + self.cache_write + self.cache_read
    }

    /// The share of the weight that went to reading context rather than to
    /// writing answers, between 0 and 1.
    pub fn reread_share(&self) -> f64 {
        let weight = self.weight();
        if weight <= 0.0 {
            return 0.0;
        }
        (weight - self.output as f64 * 5.0) / weight
    }
}

/// One transcript file, and the conversation it belongs to.
struct Source {
    conversation: String,
    transcript: PathBuf,
    subagent: bool,
}

/// Works out which conversation a file under `projects/` belongs to, from
/// its place: `<project>/<id>.jsonl` is the conversation itself, and
/// `<project>/<id>/subagents/*.jsonl` are its subagents.
///
/// The place and not the `sessionId` written inside: a resumed or forked
/// conversation carries lines copied from another one, with the other id.
fn source(projects: &Path, file: &Path) -> Option<Source> {
    let relative = file.strip_prefix(projects).ok()?;
    let mut parts = relative.components();
    let project = parts.next()?;
    let second = Path::new(parts.next()?.as_os_str());
    // Anything left after the second part means the file sits deeper.
    let subagent = parts.next().is_some();
    let conversation = if subagent {
        second.to_str()?
    } else {
        second.file_stem()?.to_str()?
    };
    Some(Source {
        conversation: conversation.to_string(),
        transcript: projects.join(project).join(format!("{conversation}.jsonl")),
        subagent,
    })
}

/// One answer, as kept while the files are read.
struct Answer {
    cwd: String,
    timestamp: String,
    tokens: Tokens,
    /// Index of its file in the list of sources.
    source: usize,
}

/// What is added up for one conversation.
#[derive(Default)]
struct Tally {
    tokens: Tokens,
    answers: u64,
    /// Context of the answers of the conversation itself, and how many.
    context: u64,
    own_answers: u64,
    /// Time and folder of the latest answer.
    latest: (String, String),
}

// Only the fields we need. Serde ignores the others, and `Option` everywhere
// means a record missing one of them is skipped instead of failing.
#[derive(Deserialize)]
struct Record {
    #[serde(rename = "type")]
    kind: Option<String>,
    cwd: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    usage: Option<RawUsage>,
}

#[derive(Deserialize)]
struct RawUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

/// Every `.jsonl` file under `dir`, at any depth: conversations, and the
/// subagent transcripts kept in a folder next to them.
fn transcripts(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        // `file_type` does not follow links, so a link cannot send the walk
        // outside the profile or into a loop.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            transcripts(&path, found);
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
            found.push(path);
        }
    }
}

/// `time` as Claude Code writes it in its files: `2026-11-26T19:01:14`, in
/// UTC. Two such texts compare like the moments they stand for, which lets us
/// filter by date without parsing every timestamp.
fn iso_utc(time: SystemTime) -> String {
    let seconds = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Days since 1970 to a calendar date, without a date library. This is
    // the well-known "civil from days" computation (Howard Hinnant).
    let shifted = days as i64 + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

/// Reads what the profile stored in `config_dir` consumed since `since`.
pub fn scan(config_dir: &Path, since: SystemTime) -> ProfileUsage {
    let projects = config_dir.join("projects");
    let mut found = Vec::new();
    transcripts(&projects, &mut found);
    let cutoff = iso_utc(since);

    // A file not written since the start of the period holds nothing from
    // it: leaving it out avoids reading gigabytes of old history.
    let mut files: Vec<(SystemTime, PathBuf)> = found
        .into_iter()
        .filter_map(|file| {
            let changed = fs::metadata(&file).and_then(|metadata| metadata.modified());
            match changed {
                Ok(changed) if changed < since => None,
                Ok(changed) => Some((changed, file)),
                Err(_) => Some((SystemTime::UNIX_EPOCH, file)),
            }
        })
        .collect();
    // Most recently written first. An answer copied into several files then
    // counts for the conversation that is still alive, the one the user can
    // act on. The path makes the order the same on every run.
    files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    // One entry per answer. Claude Code copies an answer into several files
    // when a conversation is resumed or forked, always with the same two
    // identifiers: counting each pair once is what keeps the totals honest.
    let mut answers: HashMap<(String, String), Answer> = HashMap::new();
    let mut sources: Vec<Source> = Vec::new();

    let mut line = Vec::new();
    for (_, file) in files {
        let Some(origin) = source(&projects, &file) else {
            continue;
        };
        let Ok(opened) = File::open(&file) else {
            continue;
        };
        sources.push(origin);
        let source = sources.len() - 1;
        let mut reader = BufReader::new(opened);
        loop {
            line.clear();
            // Bytes and not `read_line`: one invalid character must not end
            // the whole file.
            match reader.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let Ok(text) = std::str::from_utf8(&line) else {
                continue;
            };
            // Most lines are prompts and tool results, some of them huge.
            // This cheap test spares them a full parse.
            if !text.contains("\"usage\"") {
                continue;
            }
            let Ok(record) = serde_json::from_str::<Record>(text) else {
                continue;
            };
            if record.kind.as_deref() != Some("assistant") {
                continue;
            }
            // `let ... else` with several patterns: any missing piece skips
            // the line.
            let (Some(message), Some(cwd), Some(timestamp)) =
                (record.message, record.cwd, record.timestamp)
            else {
                continue;
            };
            let (Some(id), Some(usage)) = (message.id, message.usage) else {
                continue;
            };
            if timestamp.as_str() < cutoff.as_str() {
                continue;
            }
            let tokens = Tokens {
                input: usage.input_tokens.unwrap_or(0),
                output: usage.output_tokens.unwrap_or(0),
                cache_write: usage.cache_creation_input_tokens.unwrap_or(0),
                cache_read: usage.cache_read_input_tokens.unwrap_or(0),
            };
            let key = (id, record.request_id.unwrap_or_default());
            answers
                .entry(key)
                .and_modify(|seen| seen.tokens.max(&tokens))
                .or_insert(Answer {
                    cwd,
                    timestamp,
                    tokens,
                    source,
                });
        }
    }

    let mut by_folder: HashMap<String, FolderUsage> = HashMap::new();
    let mut by_conversation: HashMap<&str, (usize, Tally)> = HashMap::new();
    let mut total = Tokens::default();
    // `into_values` consumes the map: each folder name is moved, not copied.
    for answer in answers.into_values() {
        let tokens = answer.tokens;
        total.add(&tokens);

        let origin = &sources[answer.source];
        let (_, tally) = by_conversation
            .entry(origin.conversation.as_str())
            .or_insert_with(|| (answer.source, Tally::default()));
        tally.tokens.add(&tokens);
        tally.answers += 1;
        if !origin.subagent {
            tally.context += tokens.context();
            tally.own_answers += 1;
        }
        if answer.timestamp > tally.latest.0 {
            tally.latest = (answer.timestamp, answer.cwd.clone());
        }

        let entry = by_folder
            .entry(answer.cwd)
            .or_insert_with_key(|cwd| FolderUsage {
                folder: PathBuf::from(cwd),
                tokens: Tokens::default(),
                answers: 0,
            });
        entry.tokens.add(&tokens);
        entry.answers += 1;
    }
    let mut folders: Vec<FolderUsage> = by_folder.into_values().collect();
    sort(&mut folders);

    let mut conversations: Vec<ConversationUsage> = by_conversation
        .into_iter()
        .map(|(id, (source, tally))| ConversationUsage {
            id: id.to_string(),
            transcript: sources[source].transcript.clone(),
            folder: PathBuf::from(tally.latest.1),
            tokens: tally.tokens,
            answers: tally.answers,
            // `checked_div` gives `None` for a conversation seen only
            // through its subagents, where there is nothing to average.
            context: tally.context.checked_div(tally.own_answers).unwrap_or(0),
        })
        .collect();
    conversations.sort_by(|a, b| {
        b.tokens
            .weight()
            .total_cmp(&a.tokens.weight())
            .then_with(|| a.id.cmp(&b.id))
    });
    ProfileUsage {
        folders,
        conversations,
        total,
    }
}

/// Heaviest first; the name breaks ties so the order never changes between
/// two runs.
fn sort(folders: &mut [FolderUsage]) {
    folders.sort_by(|a, b| {
        b.tokens
            .weight()
            .total_cmp(&a.tokens.weight())
            .then_with(|| a.folder.cmp(&b.folder))
    });
}

/// The project a folder belongs to: the closest folder, itself or a parent,
/// that holds a `.git`. A folder outside any repository is its own project.
fn project_root(folder: &Path) -> &Path {
    // `ancestors` yields the folder itself first, then each parent.
    folder
        .ancestors()
        .find(|parent| parent.join(".git").symlink_metadata().is_ok())
        .unwrap_or(folder)
}

impl ProfileUsage {
    /// Merges the folders of one repository into one line. Claude Code is
    /// often started in a subfolder (`apps/client`, `packages/ui`); listing
    /// each one apart would hide how much the project weighs as a whole.
    pub fn by_project(self) -> ProfileUsage {
        let mut projects: HashMap<PathBuf, FolderUsage> = HashMap::new();
        for folder in self.folders {
            let root = project_root(&folder.folder).to_path_buf();
            let entry = projects.entry(root).or_insert_with_key(|root| FolderUsage {
                folder: root.clone(),
                tokens: Tokens::default(),
                answers: 0,
            });
            entry.tokens.add(&folder.tokens);
            entry.answers += folder.answers;
        }
        let mut folders: Vec<FolderUsage> = projects.into_values().collect();
        sort(&mut folders);
        let conversations = self
            .conversations
            .into_iter()
            .map(|mut conversation| {
                conversation.folder = project_root(&conversation.folder).to_path_buf();
                conversation
            })
            .collect();
        ProfileUsage {
            folders,
            conversations,
            total: self.total,
        }
    }
}

/// The usage of a profile over the last `days` days.
pub fn profile_usage(env: &Env, loaded: &Loaded, name: &str, days: u64) -> Result<ProfileUsage> {
    let dir = match config::profile_dir(env, &loaded.config, name)? {
        Some(dir) => dir,
        None => env.home.join(".claude"),
    };
    let since = SystemTime::now()
        .checked_sub(Duration::from_secs(days * 86_400))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    Ok(scan(&dir, since).by_project())
}

/// What today's rules say about a folder that was used with a profile.
#[derive(Debug, PartialEq)]
pub enum Verdict {
    /// A rule or a `.orberus-profile` file sends this folder to the profile.
    Covered(Reason),
    /// Nothing names this folder: it uses the profile only because it is
    /// the default. Where work done with the wrong account usually hides.
    NoRule,
    /// `claude` started there today would use another profile.
    Elsewhere { profile: String, reason: Reason },
    /// The folder is gone, so its git remote cannot be read any more and a
    /// rule on the remote cannot be checked. Not counted as a problem.
    Gone,
    /// The rules could not be applied, with the reason.
    Unknown(String),
}

/// Judges `folder`, used with `profile`, against the rules as they are now.
///
/// It answers "where would `claude` go if I started it there today", which
/// may differ from what happened: a rule added since, or a session started
/// with an explicit choice, both show as `Elsewhere`.
pub fn verdict(env: &Env, loaded: &Loaded, profile: &str, folder: &Path) -> Verdict {
    let found = match resolve::resolve(env, loaded, folder, None) {
        Ok(found) => found,
        Err(error) => return Verdict::Unknown(format!("{error:#}")),
    };
    match found {
        Some(found) if found.profile == profile => match found.reason {
            Reason::Default => Verdict::NoRule,
            reason => Verdict::Covered(reason),
        },
        // A path rule can still match a folder that was deleted, and the
        // arm above saw it. Past that point we cannot tell: a rule on the
        // git remote needs the folder to read the remote from.
        _ if !folder.exists() => Verdict::Gone,
        Some(found) => Verdict::Elsewhere {
            profile: found.profile,
            reason: found.reason,
        },
        // No rule and no default profile at all.
        None => Verdict::NoRule,
    }
}

/// From this many tokens sent with each answer, a conversation is flagged
/// as heavy.
pub const HEAVY_CONTEXT: u64 = 200_000;

/// A share of a whole, for people: "62%", or "<1%" for a trace.
pub fn percent(part: f64, whole: f64) -> String {
    if whole <= 0.0 {
        return "0%".to_string();
    }
    let share = part / whole * 100.0;
    if share > 0.0 && share < 1.0 {
        "<1%".to_string()
    } else {
        format!("{share:.0}%")
    }
}

/// A number of tokens, short: "850", "482k", "1.2M".
pub fn tokens_text(count: u64) -> String {
    if count < 1_000 {
        count.to_string()
    } else if count < 999_500 {
        format!("{:.0}k", count as f64 / 1_000.0)
    } else {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    }
}

/// Why a folder goes to a profile, short enough for a table cell.
pub fn rule_text(reason: &Reason) -> String {
    match reason {
        Reason::Locked { rule, source } | Reason::Rule { rule, source } => {
            let locked = if matches!(reason, Reason::Locked { .. }) {
                "locked "
            } else {
                ""
            };
            let team = if *source == config::RuleSource::Team {
                "team "
            } else {
                ""
            };
            format!("{locked}{team}rule {rule}")
        }
        Reason::RepoFile(_) => format!("pinned with {}", resolve::REPO_FILE),
        Reason::Explicit => "explicit choice".to_string(),
        Reason::Default => "default profile".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn answer(id: &str, request: &str, cwd: &str, time: &str, output: u64) -> String {
        format!(
            r#"{{"type":"assistant","cwd":"{cwd}","timestamp":"{time}","requestId":"{request}","message":{{"id":"{id}","usage":{{"input_tokens":10,"output_tokens":{output},"cache_creation_input_tokens":0,"cache_read_input_tokens":100}}}}}}"#
        )
    }

    fn write(path: &Path, lines: &[String]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = File::create(path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
    }

    #[test]
    fn dates_are_written_as_claude_code_writes_them() {
        let at = |seconds| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
        assert_eq!(iso_utc(at(0)), "1970-01-01T00:00:00");
        // A leap day, and the last second of a year.
        assert_eq!(iso_utc(at(1_709_208_000)), "2024-02-29T12:00:00");
        assert_eq!(iso_utc(at(1_798_761_599)), "2026-12-31T23:59:59");
    }

    #[test]
    fn an_answer_copied_into_several_files_counts_once() {
        let temp = tempfile::tempdir().unwrap();
        let projects = temp.path().join("projects/-p");
        let now = "2026-11-26T10:00:00.000Z";
        write(
            &projects.join("a.jsonl"),
            &[
                // The same answer twice while it streamed: the larger wins.
                answer("m1", "r1", "/work/api", now, 5),
                answer("m1", "r1", "/work/api", now, 40),
                answer("m2", "r2", "/work/api", now, 10),
            ],
        );
        // A resumed conversation and a subagent, with a copy of m1.
        write(
            &projects.join("b.jsonl"),
            &[answer("m1", "r1", "/work/api", now, 40)],
        );
        write(
            &projects.join("b/subagents/agent.jsonl"),
            &[answer("m3", "r3", "/work/blog", now, 1)],
        );

        let usage = scan(temp.path(), SystemTime::UNIX_EPOCH);
        assert_eq!(usage.total.output, 51);
        assert_eq!(usage.folders.len(), 2);
        // Heaviest first.
        assert_eq!(usage.folders[0].folder, Path::new("/work/api"));
        assert_eq!(usage.folders[0].answers, 2);
        assert_eq!(usage.folders[0].tokens.output, 50);
        assert_eq!(usage.folders[1].folder, Path::new("/work/blog"));
    }

    #[test]
    fn answers_before_the_period_and_odd_lines_are_left_out() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("projects/-p/a.jsonl");
        write(
            &file,
            &[
                answer("old", "r0", "/work/api", "2020-01-01T00:00:00.000Z", 99),
                answer("new", "r1", "/work/api", "2026-11-26T10:00:00.000Z", 7),
                // Not an answer, not JSON, and a format we do not know.
                r#"{"type":"user","cwd":"/work/api","message":{"usage":{}}}"#.to_string(),
                "not json but it says \"usage\"".to_string(),
                r#"{"type":"assistant","message":{"usage":"soon a string?"}}"#.to_string(),
            ],
        );
        // 2026-01-01.
        let since = SystemTime::UNIX_EPOCH + Duration::from_secs(1_767_225_600);
        let usage = scan(temp.path(), since);
        assert_eq!(usage.total.output, 7);
        assert_eq!(usage.folders.len(), 1);
    }

    #[test]
    fn a_conversation_counts_its_subagents_but_not_in_its_context() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("projects/-p");
        let line = |id: &str, time: &str, cwd: &str, read: u64| {
            format!(
                r#"{{"type":"assistant","cwd":"{cwd}","timestamp":"{time}","requestId":"r-{id}","message":{{"id":"{id}","usage":{{"input_tokens":0,"output_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":{read}}}}}}}"#
            )
        };
        write(
            &project.join("long.jsonl"),
            &[
                line("a", "2026-11-26T10:00:00.000Z", "/work/api", 100_000),
                line("b", "2026-11-26T11:00:00.000Z", "/work/api/src", 300_000),
            ],
        );
        write(
            &project.join("long/subagents/agent-1.jsonl"),
            &[line("c", "2026-11-26T10:30:00.000Z", "/work/api", 5_000)],
        );
        write(
            &project.join("short.jsonl"),
            &[line("d", "2026-11-26T12:00:00.000Z", "/work/blog", 2_000)],
        );

        let usage = scan(temp.path(), SystemTime::UNIX_EPOCH);
        assert_eq!(usage.conversations.len(), 2);
        let long = &usage.conversations[0];
        assert_eq!(long.id, "long");
        assert_eq!(long.transcript, project.join("long.jsonl"));
        assert_eq!(long.answers, 3);
        assert_eq!(long.tokens.cache_read, 405_000);
        // The average of its own two answers: the subagent is left out.
        assert_eq!(long.context, 200_000);
        // Where it was last used.
        assert_eq!(long.folder, Path::new("/work/api/src"));
        assert_eq!(usage.conversations[1].id, "short");
        assert_eq!(usage.conversations[1].context, 2_000);
    }

    #[test]
    fn a_copied_answer_goes_to_the_conversation_written_last() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("projects/-p");
        let now = "2026-11-26T10:00:00.000Z";
        let copy = [answer("m1", "r1", "/work/api", now, 5)];
        write(&project.join("original.jsonl"), &copy);
        write(&project.join("fork.jsonl"), &copy);
        // The original was last written an hour before its fork.
        let earlier = SystemTime::now() - Duration::from_secs(3_600);
        File::options()
            .write(true)
            .open(project.join("original.jsonl"))
            .unwrap()
            .set_modified(earlier)
            .unwrap();

        let usage = scan(temp.path(), SystemTime::UNIX_EPOCH);
        assert_eq!(usage.conversations.len(), 1);
        assert_eq!(usage.conversations[0].id, "fork");
        assert_eq!(usage.total.output, 5);
    }

    #[test]
    fn shares_and_token_counts_read_short() {
        assert_eq!(percent(0.0, 0.0), "0%");
        assert_eq!(percent(1.0, 400.0), "<1%");
        assert_eq!(percent(62.4, 100.0), "62%");
        assert_eq!(tokens_text(850), "850");
        assert_eq!(tokens_text(482_012), "482k");
        assert_eq!(tokens_text(999_700), "1.0M");
        assert_eq!(tokens_text(1_240_000), "1.2M");
    }

    #[test]
    fn most_of_a_long_conversation_is_reading_it_again() {
        let tokens = Tokens {
            input: 0,
            output: 1_000,
            cache_write: 0,
            cache_read: 450_000,
        };
        assert_eq!(tokens.context(), 450_000);
        // 45 000 for the context against 5 000 for the answer.
        assert!((tokens.reread_share() - 0.9).abs() < 1e-9);
        assert_eq!(Tokens::default().reread_share(), 0.0);
    }

    #[test]
    fn subfolders_of_a_repository_count_as_one_project() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("code/shop");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("apps/client")).unwrap();
        let alone = temp.path().join("notes");
        fs::create_dir_all(&alone).unwrap();
        let now = "2026-11-26T10:00:00.000Z";
        let cwd = |path: PathBuf| path.display().to_string();
        write(
            &temp.path().join("config/projects/-p/a.jsonl"),
            &[
                answer("m1", "r1", &cwd(repo.clone()), now, 10),
                answer("m2", "r2", &cwd(repo.join("apps/client")), now, 20),
                answer("m3", "r3", &cwd(alone.clone()), now, 1),
                // Deleted since: nothing to look at, it stays as it is.
                answer("m4", "r4", "/gone/project/src", now, 1),
            ],
        );
        let usage = scan(&temp.path().join("config"), SystemTime::UNIX_EPOCH).by_project();
        assert_eq!(usage.folders.len(), 3);
        assert_eq!(usage.folders[0].folder, repo);
        assert_eq!(usage.folders[0].answers, 2);
        assert_eq!(usage.folders[0].tokens.output, 30);
        assert_eq!(usage.total.output, 32);
        assert!(usage.folders.iter().any(|f| f.folder == alone));
        assert!(usage
            .folders
            .iter()
            .any(|f| f.folder == Path::new("/gone/project/src")));
    }

    #[test]
    fn a_profile_without_conversations_has_no_usage() {
        let temp = tempfile::tempdir().unwrap();
        let usage = scan(temp.path(), SystemTime::UNIX_EPOCH);
        assert!(usage.folders.is_empty());
        assert_eq!(usage.total, Tokens::default());
    }
}
