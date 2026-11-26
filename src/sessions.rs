//! The conversations a profile keeps on disk, and moving one to the Trash.
//!
//! Layout seen with Claude Code 2.1.323 on macOS, inside a config folder:
//!
//! ```text
//! projects/<project>/<session id>.jsonl    the transcript
//! projects/<project>/<session id>/         subagents, tool results
//! projects/<project>/memory/               notes kept between sessions
//! ```
//!
//! This is not a documented interface. Only what carries a session id is
//! ever listed or moved; `memory/` and anything unknown are left alone.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context, Result};

use crate::config::{self, Config};
use crate::disk;
use crate::paths::Env;

/// A conversation changed more recently than this may still be open in a
/// terminal: it is not moved.
pub const IN_USE: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone)]
pub struct Session {
    /// The id Claude Code gave it, as in `claude --resume <id>`.
    pub id: String,
    /// The folder under `projects/` that holds it.
    pub project_dir: PathBuf,
    /// The folder the conversation worked in, when the transcript says so.
    pub folder: Option<String>,
    /// The title Claude Code shows for it, when it has one.
    pub title: Option<String>,
    /// Space on disk: the transcript plus its folder of attachments.
    pub size: u64,
    /// When any of its files last changed.
    pub last_used: SystemTime,
    /// What belongs to it: the `.jsonl` file, the folder, or both.
    pub paths: Vec<PathBuf>,
}

impl Session {
    pub fn in_use(&self, now: SystemTime) -> bool {
        // `duration_since` fails when the file is dated in the future: a
        // clock that moved. Treat that as "just used", the careful answer.
        now.duration_since(self.last_used)
            .map_or(true, |age| age < IN_USE)
    }
}

/// True for a name shaped like a session id: 36 characters, hex digits and
/// dashes (`19b2f129-9a44-4b7f-a81c-c8d5cc10536b`).
fn is_session_id(name: &str) -> bool {
    name.len() == 36
        && name.matches('-').count() == 4
        && name.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// The newest change time of a file, or of anything inside a folder.
fn newest(path: &Path) -> Option<SystemTime> {
    let mut latest = None;
    let mut pending = vec![path.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            continue;
        };
        if metadata.is_dir() {
            if let Ok(entries) = fs::read_dir(&current) {
                pending.extend(entries.flatten().map(|entry| entry.path()));
            }
        } else if let Ok(changed) = metadata.modified() {
            // `max` on `Option`: `None` is smaller than any `Some`.
            latest = latest.max(Some(changed));
        }
    }
    latest
}

/// Reads the title and the working folder from the start of a transcript.
///
/// A transcript can be hundreds of megabytes, with single lines of several
/// megabytes, so only its beginning is read, and only the short lines that
/// mention what we look for are parsed.
fn describe(transcript: &Path) -> (Option<String>, Option<String>) {
    const BUDGET: usize = 4 * 1024 * 1024;
    const LONGEST_LINE: usize = 64 * 1024;
    let (mut title, mut folder) = (None, None);
    let Ok(file) = File::open(transcript) else {
        return (title, folder);
    };
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let mut read = 0;
    while read < BUDGET && (title.is_none() || folder.is_none()) {
        line.clear();
        // Bytes and not `read_line`: one invalid character must not end it.
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(count) => read += count,
        }
        if line.len() > LONGEST_LINE {
            continue;
        }
        let text = String::from_utf8_lossy(&line);
        if !text.contains("\"customTitle\"") && !text.contains("\"cwd\"") {
            continue;
        }
        let Ok(record) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let field = |key: &str| record.get(key)?.as_str().map(str::to_string);
        // The title can be set again later in the file: the first one read
        // is enough to recognize the conversation.
        title = title.or_else(|| field("customTitle"));
        folder = folder.or_else(|| field("cwd"));
    }
    (title, folder)
}

/// Lists the conversations kept in a Claude Code config folder, biggest
/// first. A folder without any measures as an empty list.
pub fn list(config_dir: &Path) -> Vec<Session> {
    let mut sessions: Vec<Session> = Vec::new();
    let Ok(projects) = fs::read_dir(config_dir.join("projects")) else {
        return sessions;
    };
    for project in projects.flatten() {
        let project_dir = project.path();
        let Ok(entries) = fs::read_dir(&project_dir) else {
            continue;
        };
        // Where this project's sessions start in the list, to find the one
        // an entry belongs to (its file and its folder are two entries).
        let first = sessions.len();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let id = name.strip_suffix(".jsonl").unwrap_or(name);
            if !is_session_id(id) {
                continue;
            }
            let index = match sessions[first..].iter().position(|s| s.id == id) {
                Some(offset) => first + offset,
                None => {
                    sessions.push(Session {
                        id: id.to_string(),
                        project_dir: project_dir.clone(),
                        folder: None,
                        title: None,
                        size: 0,
                        last_used: SystemTime::UNIX_EPOCH,
                        paths: Vec::new(),
                    });
                    sessions.len() - 1
                }
            };
            let session = &mut sessions[index];
            session.size += disk::tree_size(&path);
            if let Some(changed) = newest(&path) {
                session.last_used = session.last_used.max(changed);
            }
            if name.ends_with(".jsonl") {
                (session.title, session.folder) = describe(&path);
            }
            session.paths.push(path);
        }
    }
    // Biggest first; `Reverse` flips the order.
    sessions.sort_by_key(|session| std::cmp::Reverse(session.size));
    sessions
}

/// The conversations of a profile.
pub fn profile_sessions(env: &Env, config: &Config, name: &str) -> Result<Vec<Session>> {
    let dir = match config::profile_dir(env, config, name)? {
        Some(dir) => dir,
        None => env.home.join(".claude"),
    };
    Ok(list(&dir))
}

/// The user's Trash. Only macOS for now: the other systems keep theirs
/// elsewhere, with rules of their own.
pub fn trash_dir(env: &Env) -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        bail!("moving conversations to the Trash is only available on macOS for now");
    }
    Ok(env.home.join(".Trash"))
}

/// Moves a conversation into `trash`, in a folder of its own, and returns
/// that folder. A note inside says where it came from, to put it back.
///
/// Nothing is deleted: the files are renamed, which also makes this
/// instant whatever their size. It fails rather than copy when `trash` is
/// on another disk.
pub fn move_to_trash(session: &Session, trash: &Path, now: SystemTime) -> Result<PathBuf> {
    if session.in_use(now) {
        bail!(
            "conversation {} was used in the last hour and may still be open: close it first",
            &session.id[..8]
        );
    }
    // A free name: the Trash may already hold an older copy.
    let mut holder = trash.join(format!("Claude conversation {}", session.id));
    let mut copy = 2;
    while holder.exists() {
        holder = trash.join(format!("Claude conversation {} {copy}", session.id));
        copy += 1;
    }
    fs::create_dir_all(&holder).with_context(|| format!("cannot create {}", holder.display()))?;
    let note = format!(
        "Moved here by Claude Profiles (cprofile clean).\n\
         To restore this conversation, move the other items of this folder back into:\n\
         {}\n",
        session.project_dir.display()
    );
    fs::write(holder.join("where it came from.txt"), note)
        .with_context(|| format!("cannot write in {}", holder.display()))?;
    for path in &session.paths {
        let Some(name) = path.file_name() else {
            continue;
        };
        fs::rename(path, holder.join(name))
            .with_context(|| format!("cannot move {} to the Trash", path.display()))?;
    }
    Ok(holder)
}

/// How long ago, in words: "3 hours ago", "2 days ago".
pub fn ago(then: SystemTime, now: SystemTime) -> String {
    let seconds = now.duration_since(then).map_or(0, |age| age.as_secs());
    let (count, unit) = match seconds {
        0..=59 => return "just now".to_string(),
        60..=3599 => (seconds / 60, "minute"),
        3600..=86_399 => (seconds / 3600, "hour"),
        _ => (seconds / 86_400, "day"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "19b2f129-9a44-4b7f-a81c-c8d5cc10536b";
    const OTHER: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

    /// A config folder with two conversations and a memory folder.
    fn world() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("config");
        let project = dir.join("projects/-Users-me-app");
        fs::create_dir_all(project.join(ID).join("tool-results")).unwrap();
        fs::write(
            project.join(format!("{ID}.jsonl")),
            concat!(
                "{\"type\":\"custom-title\",\"customTitle\":\"Fix the login\"}\n",
                "not json at all\n",
                "{\"type\":\"user\",\"cwd\":\"/Users/me/app\"}\n",
            ),
        )
        .unwrap();
        fs::write(
            project.join(ID).join("tool-results/shot.jpg"),
            vec![0u8; 300_000],
        )
        .unwrap();
        fs::write(project.join(format!("{OTHER}.jsonl")), "{}\n").unwrap();
        fs::create_dir_all(project.join("memory")).unwrap();
        fs::write(project.join("memory/MEMORY.md"), "- a note\n").unwrap();
        fs::write(project.join("notes.txt"), "not a session").unwrap();
        (temp, dir)
    }

    #[test]
    fn only_what_carries_a_session_id_is_listed() {
        assert!(is_session_id(ID));
        assert!(!is_session_id("memory"));
        assert!(!is_session_id("19b2f129-9a44-4b7f-a81c-c8d5cc10536"));
        assert!(!is_session_id("zzzzzzzz-9a44-4b7f-a81c-c8d5cc10536b"));

        let (_temp, dir) = world();
        let sessions = list(&dir);
        assert_eq!(sessions.len(), 2);
        // Biggest first, with the transcript and its folder counted together.
        let big = &sessions[0];
        assert_eq!(big.id, ID);
        assert!(big.size >= 300_000);
        assert_eq!(big.paths.len(), 2);
        assert_eq!(big.title.as_deref(), Some("Fix the login"));
        assert_eq!(big.folder.as_deref(), Some("/Users/me/app"));
        // A transcript that says nothing still lists, without a title.
        assert_eq!(sessions[1].id, OTHER);
        assert_eq!(sessions[1].title, None);

        // No projects folder at all: an empty list, not an error.
        assert!(list(&dir.join("nowhere")).is_empty());
    }

    #[test]
    fn a_conversation_moves_to_the_trash_whole_and_memory_stays() {
        let (temp, dir) = world();
        let trash = temp.path().join("Trash");
        fs::create_dir_all(&trash).unwrap();
        let session = list(&dir).remove(0);
        let project = session.project_dir.clone();

        // Just written: it may still be open, so it is refused.
        let now = SystemTime::now();
        let error = move_to_trash(&session, &trash, now)
            .unwrap_err()
            .to_string();
        assert!(error.contains("last hour"), "{error}");
        assert!(project.join(format!("{ID}.jsonl")).exists());

        // Two hours later it can go.
        let later = now + Duration::from_secs(2 * 60 * 60);
        let holder = move_to_trash(&session, &trash, later).unwrap();
        assert!(holder.join(format!("{ID}.jsonl")).is_file());
        assert!(holder.join(ID).join("tool-results/shot.jpg").is_file());
        let note = fs::read_to_string(holder.join("where it came from.txt")).unwrap();
        assert!(note.contains("-Users-me-app"), "{note}");

        // Gone from the profile; everything else is untouched.
        assert!(!project.join(format!("{ID}.jsonl")).exists());
        assert!(!project.join(ID).exists());
        assert!(project.join("memory/MEMORY.md").is_file());
        assert!(project.join("notes.txt").is_file());
        assert!(project.join(format!("{OTHER}.jsonl")).is_file());
        assert_eq!(list(&dir).len(), 1);
    }

    #[test]
    fn ages_read_like_a_person_would_say_them() {
        let now = SystemTime::now();
        let back = |seconds| now - Duration::from_secs(seconds);
        assert_eq!(ago(back(5), now), "just now");
        assert_eq!(ago(back(60), now), "1 minute ago");
        assert_eq!(ago(back(3 * 3600), now), "3 hours ago");
        assert_eq!(ago(back(86_400), now), "1 day ago");
        assert_eq!(ago(back(9 * 86_400), now), "9 days ago");
        // A file dated in the future does not panic.
        assert_eq!(ago(now + Duration::from_secs(60), now), "just now");
    }
}
