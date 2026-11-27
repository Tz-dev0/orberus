//! Finds the real binary of a tool (`claude`) and starts it with the right
//! account.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

use crate::config::Config;
use crate::paths::Env;
use crate::tool::Tool;

/// Set on every process we start. A `claude` started from inside a running
/// session inherits it and keeps the session's account instead of resolving
/// again from its own folder.
pub const SESSION_ENV: &str = "ORBERUS_SESSION";
/// Explicit choice for one command: `ORBERUS_PROFILE=work claude`.
pub const EXPLICIT_ENV: &str = "ORBERUS_PROFILE";

pub enum Target<'a> {
    /// Run the tool exactly as if Orberus was not installed.
    Passthrough,
    /// `dir` is `None` for a native profile: the tool's config variable
    /// (CLAUDE_CONFIG_DIR) stays unset.
    Profile {
        name: &'a str,
        dir: Option<&'a Path>,
    },
}

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

/// Every binary of the tool found in PATH, in order, with a flag telling
/// whether it is this program (the shim) rather than the real one.
pub fn in_path(tool: &Tool) -> Vec<(PathBuf, bool)> {
    let me = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok());
    let Some(path) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for dir in std::env::split_paths(&path) {
        for name in tool.executables() {
            let candidate = dir.join(name);
            if is_executable(&candidate) {
                let is_shim = me.is_some() && candidate.canonicalize().ok() == me;
                found.push((candidate, is_shim));
            }
        }
    }
    found
}

pub fn find_real(env: &Env, config: &Config, tool: &Tool) -> Result<PathBuf> {
    if let Some(custom) = tool.forced_bin(config) {
        let path = env.expand_tilde(custom);
        if !is_executable(&path) {
            bail!(
                "{} points to {}, which is not an executable file",
                tool.bin_setting(),
                path.display()
            );
        }
        return Ok(path);
    }
    in_path(tool)
        .into_iter()
        .find(|(_, is_shim)| !is_shim)
        .map(|(path, _)| path)
        .ok_or_else(|| {
            anyhow!(
                "cannot find the real `{}` in your PATH; install {}, \
                 or set {} in {}",
                tool.command,
                tool.name,
                tool.bin_setting(),
                env.config_file().display()
            )
        })
}

/// Starts the real tool. On macOS and Linux this replaces the current
/// process, so signals and the terminal behave as if it was started
/// directly. Returns the exit code only on platforms without `exec`.
pub fn launch(tool: &Tool, real: &Path, target: Target, args: Vec<OsString>) -> Result<i32> {
    let mut command = Command::new(real);
    command.args(args);
    if let Target::Profile { name, dir } = target {
        command.env(SESSION_ENV, name);
        match dir {
            Some(dir) => {
                fs::create_dir_all(dir)
                    .with_context(|| format!("cannot create {}", dir.display()))?;
                command.env(tool.config_env, dir);
            }
            None => {
                command.env_remove(tool.config_env);
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
