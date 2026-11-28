//! The shim: what this program does when it is started under a tool's name.
//! It picks the account for the current folder, then hands over to the real
//! tool. `orberus run` goes through the same code, and so does the wrapper
//! that an editor extension starts in place of its own copy of the tool.

use std::ffi::{OsStr, OsString};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use orberus_core::config::{self, Loaded};
use orberus_core::launch::{self, Target};
use orberus_core::paths::Env;
use orberus_core::pause;
use orberus_core::resolve;
use orberus_core::tool::{Tool, CLAUDE, CODEX};

use crate::ui::{self, Style};

/// Shim mode: every argument is forwarded to the real tool unchanged.
pub(crate) fn shim(tool: &Tool, args: Vec<OsString>) -> Result<i32> {
    let env = Env::from_process()?;
    if pause::is_paused(&env) {
        return paused(tool, &env, None, args);
    }
    // The marker belongs to Claude Code. Codex resolves its own account even
    // when it was started from inside a Claude session.
    if tool == &CLAUDE && std::env::var_os(launch::SESSION_ENV).is_some() {
        let config = config::load(&env)
            .map(|loaded| loaded.config)
            .unwrap_or_default();
        let real = launch::find_real(&env, &config, tool)?;
        return launch::launch(tool, &real, Target::Passthrough, args);
    }
    let loaded = config::load(&env)?;
    start(tool, &env, &loaded, explicit_choice(), None, args)
}

/// Wrapper mode: an editor extension starts this program in place of its
/// own copy of the tool, and passes that copy as the first argument. The
/// account is picked the same way, from the folder the editor has open,
/// then the extension's copy is started with it.
pub(crate) fn wrapper(tool: &Tool, args: Vec<OsString>) -> Result<i32> {
    let env = Env::from_process()?;
    let (handed, args) = launch::handed_over(args)?;
    if pause::is_paused(&env) {
        return paused(tool, &env, handed, args);
    }
    // No "inside a session" shortcut here, unlike the shim: only an editor
    // starts the wrapper, never a running session. If that variable is set,
    // the editor itself was opened from a session's terminal, and its
    // windows must still get the account of their own folder.
    let loaded = config::load(&env)?;
    start(tool, &env, &loaded, explicit_choice(), handed, args)
}

/// While Orberus is paused: the tool starts untouched, as if Orberus was
/// not installed. No rule is read, so a config that cannot be read does
/// not stop it either. Said on the terminal each time, so that a pause is
/// not forgotten.
fn paused(tool: &Tool, env: &Env, handed: Option<PathBuf>, args: Vec<OsString>) -> Result<i32> {
    // The config is only needed for two settings: where the real tool is,
    // and whether to keep quiet.
    let config = config::load(env)
        .map(|loaded| loaded.config)
        .unwrap_or_default();
    let real = match handed {
        Some(real) => real,
        None => launch::find_real(env, &config, tool)?,
    };
    if !config.quiet && std::io::stderr().is_terminal() {
        let notice = "orberus: paused, no account is picked (orberus resume)";
        eprintln!("{}", ui::paint_stderr(notice, Style::Yellow));
    }
    launch::launch(tool, &real, Target::Passthrough, args)
}

/// `ORBERUS_PROFILE=work`, when set to something.
fn explicit_choice() -> Option<String> {
    std::env::var(launch::EXPLICIT_ENV)
        .ok()
        .filter(|name| !name.is_empty())
}

pub(crate) fn run_tool(
    tool: &Tool,
    env: &Env,
    loaded: &Loaded,
    explicit: Option<String>,
    args: Vec<OsString>,
) -> Result<i32> {
    start(tool, env, loaded, explicit, None, args)
}

/// Picks the account, then starts the tool: the copy that was `handed`
/// over when there is one, the real one found in `PATH` otherwise.
fn start(
    tool: &Tool,
    env: &Env,
    loaded: &Loaded,
    explicit: Option<String>,
    handed: Option<PathBuf>,
    args: Vec<OsString>,
) -> Result<i32> {
    let real = match handed {
        Some(real) => real,
        None => launch::find_real(env, &loaded.config, tool)?,
    };
    if loaded.is_empty() && explicit.is_none() {
        return launch::launch(tool, &real, Target::Passthrough, args);
    }
    let mut cwd = std::env::current_dir().context("cannot read the current folder")?;
    if tool == &CODEX {
        if let Some(dir) = codex_cwd(&args) {
            cwd.push(dir);
        }
    }
    let Some(found) = resolve::resolve(env, loaded, &cwd, explicit.as_deref())? else {
        return launch::launch(tool, &real, Target::Passthrough, args);
    };
    let dir = tool.profile_dir(env, &loaded.config, &found.profile)?;
    if !loaded.config.quiet && std::io::stderr().is_terminal() {
        let notice = format!("orberus: {} ({})", found.profile, found.reason.describe());
        eprintln!("{}", ui::paint_stderr(&notice, Style::Dim));
    }
    let target = Target::Profile {
        name: &found.profile,
        dir: dir.as_deref(),
    };
    launch::launch(tool, &real, target, args)
}

/// Codex accepts directory options before or after a subcommand. A later
/// option wins, and once `--` appears, any following text is a tool argument.
fn codex_cwd(args: &[OsString]) -> Option<&Path> {
    let mut dir = None;
    let mut args = args.iter().map(OsString::as_os_str);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--") => break,
            Some("-C" | "--cd") => dir = args.next(),
            _ => {
                let value = strip_prefix(arg, "--cd=")
                    .or_else(|| strip_prefix(arg, "-C="))
                    .or_else(|| strip_prefix(arg, "-C"));
                if let Some(value) = value {
                    dir = Some(value);
                }
            }
        }
    }
    dir.map(Path::new)
}

fn strip_prefix<'a>(arg: &'a OsStr, prefix: &str) -> Option<&'a OsStr> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        arg.as_bytes()
            .strip_prefix(prefix.as_bytes())
            .map(OsStr::from_bytes)
    }
    #[cfg(not(unix))]
    {
        arg.to_str()?.strip_prefix(prefix).map(OsStr::new)
    }
}
