//! `orberus desktop`: Claude Desktop, one copy per profile.

use anyhow::Result;
use orberus_core::config::{self, Config};
use orberus_core::desktop::{self, Opened, State};
use orberus_core::paths::Env;
use orberus_core::view::short_path;

use crate::commands::unknown_profile;
use crate::ui::{Cell, Style, Ui};

/// Without a name, the profiles and where their Claude Desktop stands.
/// With one, that profile's Claude Desktop is opened.
pub(crate) fn desktop_command(ui: &Ui, env: &Env, name: Option<String>) -> Result<i32> {
    let config = config::load(env)?.config;
    match name {
        Some(name) => open(ui, env, &config, &name),
        None => list(ui, env, &config),
    }
}

fn list(ui: &Ui, env: &Env, config: &Config) -> Result<i32> {
    if config.profiles.is_empty() {
        println!("No profile yet. Start with: {}", ui.command("orberus add"));
        return Ok(0);
    }
    let app = desktop::app(env);
    // Without the app nothing can be running. A list that cannot be read
    // only costs the "open" marks: this command changes nothing.
    let copies = match &app {
        Some(app) => desktop::running(app).unwrap_or_default(),
        None => Vec::new(),
    };
    let mut rows = Vec::new();
    for name in config.profiles.keys() {
        let state = match desktop::state(env, config, name, &copies)? {
            State::Open(_) => Cell::styled("open", Style::Green),
            State::Closed => Cell::new("closed"),
            State::New => Cell::styled("not opened yet", Style::Dim),
        };
        let data = match desktop::data_dir(env, config, name)? {
            Some(dir) => short_path(env, &dir),
            None => format!(
                "{} (Claude Desktop's own)",
                short_path(env, &desktop::default_data_dir(env))
            ),
        };
        rows.push(vec![
            Cell::new(name.as_str()),
            state,
            Cell::styled(data, Style::Dim),
        ]);
    }
    ui.table(&["Profile", "Claude Desktop", "Its data"], &rows);
    println!();
    if app.is_none() {
        ui.warning(&desktop::not_found(env));
        return Ok(0);
    }
    println!("Open one with: {}", ui.command("orberus desktop <profile>"));
    ui.detail("Each profile has its own Claude Desktop, signed in on its own. They all show as");
    ui.detail("\"Claude\" in the Dock: this command brings the right one to the front.");
    Ok(0)
}

fn open(ui: &Ui, env: &Env, config: &Config, name: &str) -> Result<i32> {
    if !config.profiles.contains_key(name) {
        return Err(unknown_profile(config, name));
    }
    // Read before opening: afterwards the data folder is always there.
    let first_time = desktop::state(env, config, name, &[])? == State::New;
    match desktop::open(env, config, name)? {
        Opened::Shown => {
            ui.success(&format!(
                "Claude Desktop of '{name}' was already open: brought to the front"
            ));
        }
        Opened::Started => {
            ui.success(&format!("Claude Desktop started for '{name}'"));
            if first_time {
                ui.detail("First time: sign in there with the account of this profile.");
                ui.detail(
                    "If you sign in through your browser, quit the other Claude Desktop windows",
                );
                ui.detail(
                    "first: with two open, which one the browser comes back to was not checked.",
                );
            }
        }
    }
    Ok(0)
}
