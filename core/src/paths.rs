use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

/// Overrides the data folder (default `~/.orberus`). Used by tests.
pub const ROOT_ENV: &str = "ORBERUS_HOME";

/// Where things live on this machine. Passed around explicitly so the core
/// never reads global state, which keeps it easy to test.
#[derive(Debug, Clone)]
pub struct Env {
    pub home: PathBuf,
    pub root: PathBuf,
}

impl Env {
    pub fn from_process() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("cannot find your home folder (HOME is not set)"))?;
        let root = root_folder(&home, std::env::var_os(ROOT_ENV));
        Ok(Self { home, root })
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// Rules provided by a company. Same format as the `[[rule]]` entries of
    /// the user config, checked before them.
    pub fn team_file(&self) -> PathBuf {
        self.root.join("team.toml")
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }

    /// Where the wrappers that editor extensions start are kept.
    pub fn ide_dir(&self) -> PathBuf {
        self.root.join("ide")
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }

    /// Expands a leading `~` to the home folder.
    pub fn expand_tilde(&self, path: &str) -> PathBuf {
        if path == "~" {
            self.home.clone()
        } else if let Some(rest) = path.strip_prefix("~/") {
            self.home.join(rest)
        } else {
            Path::new(path).to_path_buf()
        }
    }
}

/// Creates a directory for local credentials, with owner-only access on Unix.
pub(crate) fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}

/// The data folder: the override when it names a folder, `~/.orberus`
/// otherwise.
///
/// A variable that is set but empty (`ORBERUS_HOME= orberus ...`)
/// counts as not set. An empty path would otherwise mean "the current
/// folder", and the config would be written wherever the command was run.
///
/// It takes the value as a parameter instead of reading the environment
/// itself, so a test can call it without changing the process's variables.
fn root_folder(home: &Path, overridden: Option<OsString>) -> PathBuf {
    match overridden {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => home.join(".orberus"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_under_home_by_default() {
        let home = Path::new("/home/me");
        assert_eq!(root_folder(home, None), Path::new("/home/me/.orberus"));
    }

    #[test]
    fn root_can_be_overridden() {
        let home = Path::new("/home/me");
        assert_eq!(
            root_folder(home, Some(OsString::from("/tmp/elsewhere"))),
            Path::new("/tmp/elsewhere")
        );
    }

    #[test]
    fn an_empty_override_is_ignored() {
        let home = Path::new("/home/me");
        assert_eq!(
            root_folder(home, Some(OsString::new())),
            Path::new("/home/me/.orberus")
        );
    }
}
