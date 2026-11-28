//! One file per command, or per family of commands. This file holds what
//! they share: asking for a missing argument and a few table cells.

pub(crate) mod clean;
pub(crate) mod disk;
pub(crate) mod doctor;
pub(crate) mod ide;
pub(crate) mod limits;
pub(crate) mod pause;
pub(crate) mod profiles;
pub(crate) mod retention;
pub(crate) mod rules;
pub(crate) mod setup;
pub(crate) mod status;
pub(crate) mod uninstall;
pub(crate) mod usage;

use anyhow::{anyhow, bail, Result};
use orberus_core::config::Config;
use orberus_core::paths::Env;
use orberus_core::view::account;

use crate::ui::{Cell, Style, Ui};

/// `on` or `off`, as typed after `orberus limits` or `orberus ide`.
#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum Switch {
    On,
    Off,
}

/// The account as a table cell or a status value.
pub(crate) fn account_cell(env: &Env, config: &Config, name: &str) -> Cell {
    match account(env, config, name) {
        Some(email) => Cell::new(email),
        None => Cell::styled("not logged in", Style::Yellow),
    }
}

pub(crate) fn unknown_profile(config: &Config, name: &str) -> anyhow::Error {
    if config.profiles.is_empty() {
        return anyhow!(
            "unknown profile '{name}': no profile exists yet, create one with: orberus add {name}"
        );
    }
    let names: Vec<&str> = config.profiles.keys().map(String::as_str).collect();
    anyhow!("unknown profile '{name}' (available: {})", names.join(", "))
}

/// Returns the profile to act on: the one given on the command line when it
/// exists, otherwise the one picked in a list. In a script, where nobody can
/// answer, a missing name is an error that says what to type.
pub(crate) fn pick_profile(
    ui: &Ui,
    config: &Config,
    given: Option<String>,
    question: &str,
) -> Result<String> {
    if let Some(name) = given {
        if !config.profiles.contains_key(&name) {
            return Err(unknown_profile(config, &name));
        }
        return Ok(name);
    }
    if config.profiles.is_empty() {
        bail!("no profile yet, create one with: orberus add");
    }
    let names: Vec<String> = config.profiles.keys().cloned().collect();
    if !ui.interactive() {
        bail!("missing profile name (available: {})", names.join(", "));
    }
    let options: Vec<String> = names
        .iter()
        .map(|name| {
            let mark = if config.default.as_deref() == Some(name) {
                ui.paint("  default", Style::Dim)
            } else {
                String::new()
            };
            format!("{name}{mark}")
        })
        .collect();
    let index = ui.select(question, &options, &names, None)?;
    // `swap_remove` moves the chosen name out of the list without cloning it.
    let mut names = names;
    Ok(names.swap_remove(index))
}

/// Asks before a change that removes something. `--yes` skips the question;
/// without a terminal there is nobody to ask, so `--yes` is required.
pub(crate) fn confirmed(ui: &Ui, question: &str, yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !ui.interactive() {
        bail!("this needs a confirmation: run it again with --yes");
    }
    ui.confirm(question, false)
}

/// Cuts a text to `width` characters, with "…" when something was cut.
pub(crate) fn shorten(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    // `chars` and not bytes: a cut in the middle of "é" would panic.
    let kept: String = text.chars().take(width - 1).collect();
    format!("{kept}…")
}
