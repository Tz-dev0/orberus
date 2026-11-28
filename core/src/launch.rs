//! Finds the real binary of a tool (`claude`) and starts it with the right
//! account.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

use crate::config::Config;
use crate::paths::{create_private_dir, Env};
use crate::tool::{Tool, CLAUDE, CODEX};

/// Set on managed Claude Code processes. A `claude` started from inside a running
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

/// Splits what an editor extension passes to the wrapper: its own copy of
/// the tool first, when it has one, then the tool's arguments.
///
/// The tool's arguments start with an option or a subcommand (`auth`),
/// never with an absolute path, so an absolute first argument is the copy
/// to start. One that cannot be started is an error: passing it on would
/// hand a path to the tool as if it was a prompt.
pub fn handed_over(mut args: Vec<OsString>) -> Result<(Option<PathBuf>, Vec<OsString>)> {
    let first_is_a_path = args
        .first()
        .is_some_and(|first| Path::new(first).is_absolute());
    if !first_is_a_path {
        return Ok((None, args));
    }
    // `remove(0)` takes the first argument out and keeps the others in
    // order.
    let binary = PathBuf::from(args.remove(0));
    if !is_executable(&binary) {
        bail!(
            "the editor asked to start {}, which is not an executable file",
            binary.display()
        );
    }
    Ok((Some(binary), args))
}

/// Starts the real tool. On macOS and Linux this replaces the current
/// process, so signals and the terminal behave as if it was started
/// directly. Returns the exit code only on platforms without `exec`.
pub fn launch(tool: &Tool, real: &Path, target: Target, args: Vec<OsString>) -> Result<i32> {
    let mut command = Command::new(real);
    command.args(args);
    if let Target::Profile { name, dir } = target {
        if tool == &CLAUDE {
            command.env(SESSION_ENV, name);
        }
        if tool == &CODEX {
            // These process-level credentials can override the selected home's login.
            command
                .env_remove("CODEX_API_KEY")
                .env_remove("CODEX_ACCESS_TOKEN");
        }
        match dir {
            Some(dir) => {
                let created = if tool == &CODEX {
                    create_private_dir(dir)
                } else {
                    fs::create_dir_all(dir)
                };
                created.with_context(|| format!("cannot create {}", dir.display()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[cfg(unix)]
    #[test]
    fn an_absolute_first_argument_is_the_copy_to_start() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let copy = temp.path().join("claude");
        fs::write(&copy, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&copy, fs::Permissions::from_mode(0o755)).unwrap();
        let copy_text = copy.to_str().unwrap();

        let (binary, rest) = handed_over(args(&[copy_text, "auth", "status"])).unwrap();
        assert_eq!(binary, Some(copy.clone()));
        assert_eq!(rest, args(&["auth", "status"]));

        // A file that cannot be run is refused, not passed on as a prompt.
        fs::set_permissions(&copy, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(handed_over(args(&[copy_text, "--print"])).is_err());
        assert!(handed_over(args(&["/nowhere/claude", "--print"])).is_err());
    }

    #[test]
    fn anything_else_first_means_no_copy_was_handed_over() {
        for list in [
            &["--output-format", "stream-json"][..],
            &["auth", "status", "--json"][..],
            &["relative/claude"][..],
            &[][..],
        ] {
            let (binary, rest) = handed_over(args(list)).unwrap();
            assert_eq!(binary, None);
            assert_eq!(rest, args(list));
        }
    }
}
