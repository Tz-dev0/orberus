//! The shim: what this program does when it is started under a tool's name.
//! It picks the account for the current folder, then hands over to the real
//! tool. `orberus run` goes through the same code.

use std::ffi::OsString;
use std::io::IsTerminal;

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
    let explicit = std::env::var(launch::EXPLICIT_ENV)
        .ok()
        .filter(|name| !name.is_empty());
    run_tool(tool, &env, &loaded, explicit, args)
}

pub(crate) fn run_tool(
    tool: &Tool,
    env: &Env,
    loaded: &Loaded,
    explicit: Option<String>,
    args: Vec<OsString>,
) -> Result<i32> {
    let real = launch::find_real(env, &loaded.config, tool)?;
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
