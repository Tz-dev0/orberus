//! How much disk space each profile takes, and what takes it.
//!
//! Read only: nothing here deletes or changes a file. It walks whole
//! folders, so it is never called on the shim's path.

use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::config::{self, Config};
use crate::paths::Env;

/// The space a profile takes, in bytes, split by what holds it.
///
/// The folder names are the ones Claude Code 2.1.323 was seen to use on
/// macOS. They are not a documented interface: a folder this code does not
/// know simply counts as `other`, so the total stays right.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// `projects/`: the conversations, one file per session.
    pub sessions: u64,
    /// `file-history/`: copies of files kept to undo edits.
    pub file_history: u64,
    /// `plugins/` and `skills/`.
    pub extensions: u64,
    /// Everything else: settings, caches, logs.
    pub other: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.sessions + self.file_history + self.extensions + self.other
    }
}

/// The space a file takes on disk. On Unix this is what `du` reports: the
/// blocks really allocated, which differs from the length for tiny files
/// (a whole block each) and for sparse ones.
fn size_of(metadata: &fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // `blocks` is always counted in 512-byte units.
        metadata.blocks() * 512
    }
    #[cfg(not(unix))]
    {
        metadata.len()
    }
}

/// The size of a file, or of a folder with all it contains.
///
/// A symbolic link counts for itself and is not followed: a skills folder
/// shared between profiles must not be counted once per profile. What cannot
/// be read is skipped, since a rough number beats no number here.
pub fn tree_size(path: &Path) -> u64 {
    let mut total = 0;
    // A list of folders still to visit, instead of a function calling
    // itself: a very deep tree cannot overflow the stack this way.
    let mut pending = vec![path.to_path_buf()];
    while let Some(current) = pending.pop() {
        // `symlink_metadata` describes the link itself, not its target.
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            continue;
        };
        total += size_of(&metadata);
        if metadata.is_dir() {
            let Ok(entries) = fs::read_dir(&current) else {
                continue;
            };
            // `flatten` drops the entries that could not be read.
            pending.extend(entries.flatten().map(|entry| entry.path()));
        }
    }
    total
}

/// Measures a Claude Code config folder. A folder that does not exist yet
/// measures zero.
pub fn measure(dir: &Path) -> Usage {
    let mut usage = Usage::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return usage;
    };
    for entry in entries.flatten() {
        let size = tree_size(&entry.path());
        // `to_str` is `None` for a name that is not valid text: `other`.
        match entry.file_name().to_str() {
            Some("projects") => usage.sessions += size,
            Some("file-history") => usage.file_history += size,
            Some("plugins" | "skills") => usage.extensions += size,
            _ => usage.other += size,
        }
    }
    usage
}

/// Measures a profile. The native one is `~/.claude` plus the
/// `~/.claude.json` file Claude Code keeps next to it.
pub fn profile_usage(env: &Env, config: &Config, name: &str) -> Result<Usage> {
    Ok(match config::profile_dir(env, config, name)? {
        Some(dir) => measure(&dir),
        None => {
            let mut usage = measure(&crate::tool::CLAUDE.native_dir(env));
            usage.other += tree_size(&env.home.join(".claude.json"));
            usage
        }
    })
}

/// A size for people: "3.4 GB", "31 MB", "720 KB". Units are decimal
/// (1 MB = 1,000,000 bytes), as in the macOS Finder.
pub fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    // `as f64` loses precision only far beyond any real disk size.
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 999.5 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    // One decimal where it says something: small numbers, and gigabytes.
    if value < 9.95 || (unit >= 2 && value < 99.95) {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_the_finder() {
        assert_eq!(human(0), "0 B");
        assert_eq!(human(999), "999 B");
        assert_eq!(human(1_000), "1.0 KB");
        assert_eq!(human(720_000), "720 KB");
        assert_eq!(human(31_588_000), "32 MB");
        assert_eq!(human(999_600_000), "1.0 GB");
        assert_eq!(human(3_371_124_000), "3.4 GB");
        assert_eq!(human(42_000_000_000), "42.0 GB");
    }

    #[test]
    fn usage_is_split_by_folder_and_links_are_not_followed() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("profile");
        // A missing folder is an empty profile, not an error.
        assert_eq!(measure(&dir), Usage::default());

        let big = vec![0u8; 200_000];
        fs::create_dir_all(dir.join("projects/-Users-me-app")).unwrap();
        fs::write(dir.join("projects/-Users-me-app/session.jsonl"), &big).unwrap();
        fs::create_dir_all(dir.join("file-history")).unwrap();
        fs::write(dir.join("file-history/a"), &big[..50_000]).unwrap();
        fs::create_dir_all(dir.join("plugins")).unwrap();
        fs::write(dir.join("plugins/p"), &big[..50_000]).unwrap();
        fs::write(dir.join("settings.json"), "{}").unwrap();

        // A big folder outside the profile, reached only through a link.
        let shared = temp.path().join("shared-skills");
        fs::create_dir_all(&shared).unwrap();
        fs::write(shared.join("skill"), &big).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&shared, dir.join("skills")).unwrap();

        let usage = measure(&dir);
        assert!(usage.sessions >= 200_000, "{usage:?}");
        assert!(usage.file_history >= 50_000, "{usage:?}");
        // The plugin is counted, the linked folder is not.
        assert!((50_000..200_000).contains(&usage.extensions), "{usage:?}");
        assert!(usage.other > 0 && usage.other < 50_000, "{usage:?}");
        assert_eq!(
            usage.total(),
            usage.sessions + usage.file_history + usage.extensions + usage.other
        );
    }
}
