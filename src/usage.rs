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

/// The usage of one profile over a period, heaviest folder first.
#[derive(Debug, Default)]
pub struct ProfileUsage {
    pub folders: Vec<FolderUsage>,
    pub total: Tokens,
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
    let mut files = Vec::new();
    transcripts(&config_dir.join("projects"), &mut files);
    let cutoff = iso_utc(since);

    // One entry per answer. Claude Code copies an answer into several files
    // when a conversation is resumed or forked, always with the same two
    // identifiers: counting each pair once is what keeps the totals honest.
    let mut answers: HashMap<(String, String), (String, Tokens)> = HashMap::new();

    let mut line = Vec::new();
    for file in files {
        // A file not written since the start of the period holds nothing
        // from it: skipping it avoids reading gigabytes of old history.
        let changed = fs::metadata(&file).and_then(|metadata| metadata.modified());
        if changed.is_ok_and(|changed| changed < since) {
            continue;
        }
        let Ok(opened) = File::open(&file) else {
            continue;
        };
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
                .and_modify(|(_, seen)| seen.max(&tokens))
                .or_insert((cwd, tokens));
        }
    }

    let mut by_folder: HashMap<String, FolderUsage> = HashMap::new();
    let mut total = Tokens::default();
    // `into_values` consumes the map: each folder name is moved, not copied.
    for (cwd, tokens) in answers.into_values() {
        total.add(&tokens);
        let entry = by_folder.entry(cwd).or_insert_with_key(|cwd| FolderUsage {
            folder: PathBuf::from(cwd),
            tokens: Tokens::default(),
            answers: 0,
        });
        entry.tokens.add(&tokens);
        entry.answers += 1;
    }
    let mut folders: Vec<FolderUsage> = by_folder.into_values().collect();
    sort(&mut folders);
    ProfileUsage { folders, total }
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
        ProfileUsage {
            folders,
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
    /// A rule or a `.claude-profile` file sends this folder to the profile.
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
