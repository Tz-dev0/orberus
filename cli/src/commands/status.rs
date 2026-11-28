//! `orberus status`, and `orberus` alone: which profile this folder gets, and why.

use anyhow::{Context, Result};
use orberus_core::config::{self, Loaded};
use orberus_core::launch;
use orberus_core::paths::Env;
use orberus_core::pause;
use orberus_core::resolve;
use orberus_core::tool;
use orberus_core::view::{account, location, plan, short_path};

use crate::ui::{Style, Ui};
use crate::Cli;

/// `orberus` with no argument: where you stand, then every command.
pub(crate) fn overview(ui: &Ui, env: &Env) -> Result<i32> {
    let loaded = config::load(env)?;
    if loaded.config.profiles.is_empty() {
        // Do not send the user back to a step that is already done.
        let shim_ready = launch::in_path(&tool::CLAUDE)
            .first()
            .is_some_and(|(_, is_shim)| *is_shim);
        let next = if shim_ready {
            "orberus add"
        } else {
            "orberus setup"
        };
        println!("No profile yet. Start with: {}", ui.command(next));
    } else {
        // Only where you stand. The table of profiles is one command
        // away (`orberus list`) and would push the commands off screen.
        status_block(ui, env, &loaded, None)?;
    }
    println!();
    // The same text as `orberus --help`, built by clap from the `Cli` type.
    // `CommandFactory` is the trait that gives access to it.
    <Cli as clap::CommandFactory>::command()
        .print_help()
        .context("cannot print the help")?;
    Ok(0)
}

fn status_block(ui: &Ui, env: &Env, loaded: &Loaded, explicit: Option<&str>) -> Result<()> {
    let cwd = std::env::current_dir().context("cannot read the current folder")?;
    let claude = match launch::find_real(env, &loaded.config, &tool::CLAUDE) {
        Ok(real) => short_path(env, &real),
        Err(error) => ui.paint(&format!("not found ({error})"), Style::Red),
    };
    match resolve::resolve(env, loaded, &cwd, explicit)? {
        Some(found) => ui.fields(&[
            ("Profile", ui.paint(&found.profile, Style::Green)),
            (
                "Account",
                match account(env, &loaded.config, &found.profile) {
                    // The subscription, when known, follows the address.
                    Some(email) => match plan(env, &loaded.config, &found.profile) {
                        Some(plan) => format!("{email} ({plan})"),
                        None => email,
                    },
                    None => ui.paint("not logged in", Style::Yellow),
                },
            ),
            ("Why", found.reason.describe()),
            ("Config", location(env, &loaded.config, &found.profile)?),
            ("Claude", claude),
        ]),
        None => ui.fields(&[
            (
                "Profile",
                "none (claude runs as if orberus was not installed)".to_string(),
            ),
            ("Claude", claude),
        ]),
    }
    // What is printed above is what the rules say; a pause sets them aside.
    if pause::is_paused(env) {
        ui.warning(
            "Orberus is paused: claude starts as if it was not installed, whatever the rules say",
        );
        ui.detail("to pick the account by folder again: orberus resume");
    }
    // Inside a session the shim does not resolve again, so what is printed
    // above is not what a nested `claude` would use.
    if let Ok(session) = std::env::var(launch::SESSION_ENV) {
        ui.warning(&format!(
            "You are inside a Claude Code session started with '{session}': claude keeps that account here"
        ));
    }
    Ok(())
}

pub(crate) fn status(ui: &Ui, env: &Env, explicit: Option<String>) -> Result<i32> {
    let loaded = config::load(env)?;
    status_block(ui, env, &loaded, explicit.as_deref())?;
    Ok(0)
}
