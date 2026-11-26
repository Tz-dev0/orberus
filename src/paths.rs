use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

/// Overrides the data folder (default `~/.claude-profiles`). Used by tests.
pub const ROOT_ENV: &str = "CLAUDE_PROFILES_HOME";

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
        let root = std::env::var_os(ROOT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude-profiles"));
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
