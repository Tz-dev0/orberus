//! Finds the real `claude` binary and starts it with the right account.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

use crate::config::Config;
use crate::paths::Env;

/// Set on every process we start. A `claude` started from inside a running
/// session inherits it and keeps the session's account instead of resolving
/// again from its own folder.
pub const SESSION_ENV: &str = "ORBERUS_SESSION";
/// Explicit choice for one command: `ORBERUS_PROFILE=work claude`.
pub const EXPLICIT_ENV: &str = "ORBERUS_PROFILE";
pub const CONFIG_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";

pub enum Target<'a> {
    /// Run `claude` exactly as if this tool was not installed.
    Passthrough,
    /// `dir` is `None` for a native profile: CLAUDE_CONFIG_DIR stays unset.
    Profile {
        name: &'a str,
        dir: Option<&'a Path>,
    },
}

#[cfg(windows)]
const CANDIDATES: &[&str] = &["claude.exe", "claude.cmd"];
#[cfg(not(windows))]
const CANDIDATES: &[&str] = &["claude"];

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Every `claude` found in PATH, in order, with a flag telling whether it is
/// this program (the shim) rather than the real one.
pub fn claude_in_path() -> Vec<(PathBuf, bool)> {
    let me = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok());
    let Some(path) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for dir in std::env::split_paths(&path) {
        for name in CANDIDATES {
            let candidate = dir.join(name);
            if is_executable(&candidate) {
                let is_shim = me.is_some() && candidate.canonicalize().ok() == me;
                found.push((candidate, is_shim));
            }
        }
    }
    found
}

pub fn find_real_claude(env: &Env, config: &Config) -> Result<PathBuf> {
    if let Some(custom) = &config.claude_bin {
        let path = env.expand_tilde(custom);
        if !is_executable(&path) {
            bail!(
                "claude_bin points to {}, which is not an executable file",
                path.display()
            );
        }
        return Ok(path);
    }
    claude_in_path()
        .into_iter()
        .find(|(_, is_shim)| !is_shim)
        .map(|(path, _)| path)
        .ok_or_else(|| {
            anyhow!(
                "cannot find the real `claude` in your PATH; install Claude Code, \
                 or set claude_bin in {}",
                env.config_file().display()
            )
        })
}

/// Starts the real `claude`. On macOS and Linux this replaces the current
/// process, so signals and the terminal behave as if `claude` was started
/// directly. Returns the exit code only on platforms without `exec`.
pub fn launch(real: &Path, target: Target, args: Vec<OsString>) -> Result<i32> {
    let mut command = Command::new(real);
    command.args(args);
    if let Target::Profile { name, dir } = target {
        command.env(SESSION_ENV, name);
        match dir {
            Some(dir) => {
                fs::create_dir_all(dir)
                    .with_context(|| format!("cannot create {}", dir.display()))?;
                command.env(CONFIG_DIR_ENV, dir);
            }
            None => {
                command.env_remove(CONFIG_DIR_ENV);
            }
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        Err(anyhow!(error).context(format!("cannot start {}", real.display())))
    }
    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .with_context(|| format!("cannot start {}", real.display()))?;
        Ok(status.code().unwrap_or(1))
    }
}
