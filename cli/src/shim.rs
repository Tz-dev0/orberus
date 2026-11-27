//! The shim: what this program does when it is started under a tool's name.
//! It picks the account for the current folder, then hands over to the real
//! tool. `orberus run` goes through the same code, and so does the wrapper
//! that an editor extension starts in place of its own copy of the tool.

use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result};
use orberus_core::config::{self, Loaded};
use orberus_core::launch::{self, Target};
use orberus_core::paths::Env;
use orberus_core::resolve;
use orberus_core::tool::Tool;

use crate::ui::{self, Style};

/// Shim mode: every argument belongs to the tool, nothing is parsed here.
pub(crate) fn shim(tool: &Tool, args: Vec<OsString>) -> Result<i32> {
    let env = Env::from_process()?;
    // Inside a session we already started: keep that session's account.
    if std::env::var_os(launch::SESSION_ENV).is_some() {
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
    // No "inside a session" shortcut here, unlike the shim: only an editor
    // starts the wrapper, never a running session. If that variable is set,
    // the editor itself was opened from a session's terminal, and its
    // windows must still get the account of their own folder.
    let loaded = config::load(&env)?;
    start(tool, &env, &loaded, explicit_choice(), handed, args)
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
    let cwd = std::env::current_dir().context("cannot read the current folder")?;
    let Some(found) = resolve::resolve(env, loaded, &cwd, explicit.as_deref())? else {
        return launch::launch(tool, &real, Target::Passthrough, args);
    };
    let dir = config::profile_dir(env, &loaded.config, &found.profile)?;
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
