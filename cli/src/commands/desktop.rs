//! `orberus desktop`: Claude Desktop, on one profile at a time.

use anyhow::Result;
use orberus_core::config::{self, Config};
use orberus_core::desktop::{self, State};
use orberus_core::paths::Env;
use orberus_core::view::short_path;

use crate::commands::{confirmed, unknown_profile};
use crate::ui::{Cell, Style, Ui};

/// Without a name, the profiles and which one Claude Desktop is on.
/// With one, Claude Desktop is switched to that profile.
pub(crate) fn desktop_command(ui: &Ui, env: &Env, name: Option<String>, yes: bool) -> Result<i32> {
    let config = config::load(env)?.config;
    match name {
        Some(name) => switch(ui, env, &config, &name, yes),
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
    // A Claude Desktop on a folder that is no profile's has no row above.
    let stray = copies
        .iter()
        .filter(|copy| desktop::owner(env, config, copy).is_none())
        .count();
    if stray > 0 {
        ui.warning("Claude Desktop is open on no profile: a switch closes it too.");
    }
    println!("Switch with: {}", ui.command("orberus desktop <profile>"));
    ui.detail("One Claude Desktop at a time: a switch closes it, then opens it again on");
    ui.detail("that profile's account. Each profile keeps its own sign-in.");
    Ok(0)
}

/// "on 'perso'", "on no profile", or both when two are open.
fn whose(closing: &[Option<String>]) -> String {
    let mut names: Vec<String> = closing
        .iter()
        .flatten()
        .map(|name| format!("'{name}'"))
        .collect();
    // `flatten` kept the named ones; one mention is enough for the rest.
    if closing.iter().any(Option::is_none) {
        names.push("no profile".to_string());
    }
    format!("on {}", names.join(" and "))
}

fn switch(ui: &Ui, env: &Env, config: &Config, name: &str, yes: bool) -> Result<i32> {
    if !config.profiles.contains_key(name) {
        return Err(unknown_profile(config, name));
    }
    // Read before switching: afterwards the data folder is always there.
    let first_time = desktop::state(env, config, name, &[])? == State::New;
    // What has to close is said before anything moves. Without the app,
    // nothing runs, and `switch` below says that it was not found.
    let closing = match desktop::app(env) {
        Some(app) => desktop::in_the_way(env, config, name, &desktop::running(&app)?)?,
        None => Vec::new(),
    };
    if !closing.is_empty() {
        let whose = whose(&closing);
        println!("Claude Desktop is open {whose}. It closes, and what runs in it stops:");
        println!("a conversation that is answering, a task.");
        let question = format!("Switch Claude Desktop to '{name}'?");
        if !confirmed(ui, &question, yes)? {
            println!("Nothing changed.");
            return Ok(0);
        }
    }

    let done = desktop::switch(env, config, name)?;
    match (done.started, done.closed) {
        (false, 0) => ui.success(&format!(
            "Claude Desktop was already on '{name}': brought to the front"
        )),
        (false, _) => ui.success(&format!(
            "Claude Desktop is on '{name}' alone: the other one was closed"
        )),
        (true, 0) => ui.success(&format!("Claude Desktop opened on '{name}'")),
        (true, _) => ui.success(&format!(
            "Claude Desktop switched to '{name}' (it was {})",
            whose(&closing)
        )),
    }
    if done.started && first_time {
        ui.detail("First time: sign in there with the account of this profile.");
    }
    Ok(0)
}
