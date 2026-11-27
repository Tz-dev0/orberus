//! Profiles: `add`, `list`, `remove`, `default`, and `use` for one folder.

use std::fs;

use anyhow::{bail, Context, Result};
use orberus_core::config::{self, Loaded};
use orberus_core::paths::Env;
use orberus_core::purge;
use orberus_core::resolve::{self, REPO_FILE};
use orberus_core::retention;
use orberus_core::tool;
use orberus_core::view::{location, short_path};

use crate::commands::retention::ask_retention;
use crate::commands::{account_cell, confirmed, pick_profile};
use crate::shim::run_tool;
use crate::ui::{Cell, Style, Ui};

pub(crate) fn add(
    ui: &Ui,
    env: &Env,
    name: Option<String>,
    mut native: bool,
    dir: Option<String>,
    keep_days: Option<u64>,
) -> Result<i32> {
    let mut config = config::load(env)?.config;
    // `map(str::to_string)` copies the name, so `config` is free to change
    // further down while we still hold it.
    let native_owner = config::native_profile(&config).map(str::to_string);

    // No name given: guided mode. The questions only cover what the flags
    // did not say, so `orberus add work --native` never asks anything.
    let guided = name.is_none();
    let name = match name {
        Some(name) => name,
        None if ui.interactive() => ui.input("Profile name (for example: work, perso):")?,
        None => bail!("missing profile name, for example: orberus add work"),
    };
    config::validate_name(&name)?;
    if config.profiles.contains_key(&name) {
        bail!(
            "profile '{name}' already exists; start Claude Code in it with: orberus run -p {name}"
        );
    }

    if guided && !native && dir.is_none() && native_owner.is_none() {
        // Only offered while no profile owns ~/.claude yet.
        native = ui.confirm(
            "Reuse your current Claude Code login (~/.claude) for this profile?",
            config.profiles.is_empty(),
        )?;
    }
    if native {
        if let Some(other) = native_owner {
            bail!(
                "profile '{other}' is already the native one; only one profile can reuse ~/.claude"
            );
        }
    }

    let first = config::add_profile(env, &mut config, &name, native, dir)?;

    ui.success(&format!(
        "Profile '{name}' created{}",
        if first { " and set as default" } else { "" }
    ));
    ui.detail(&location(env, &config, &name)?);

    // Claude Code deletes conversations after 30 days unless told otherwise,
    // and each profile has its own setting: say so now, not after a loss.
    let keep_days = match keep_days {
        Some(days) => Some(days),
        None if guided && ui.interactive() => ask_retention(ui, env, &config, &name)?,
        None => None,
    };
    match keep_days {
        Some(days) => {
            retention::set(env, &config, &name, days)?;
            ui.detail(&format!(
                "keeps its conversations {}",
                retention::days_text(days)
            ));
        }
        None => {
            let current = retention::read(env, &config, &name)?;
            ui.detail(&format!(
                "keeps its conversations {}; change it with: orberus retention {name}",
                current.describe()
            ));
        }
    }
    if native {
        return Ok(0);
    }

    let login = format!("orberus run --profile {name}");
    if ui.interactive() && ui.confirm("Start Claude Code in this profile now, to log in?", true)? {
        ui.detail("type /login in Claude Code if it does not ask by itself");
        // On macOS and Linux this call does not return: the process becomes
        // Claude Code. The config is read again to include the new profile.
        let loaded = config::load(env)?;
        return run_tool(&tool::CLAUDE, env, &loaded, Some(name), Vec::new());
    }
    println!(
        "Log in to its account with: {}   then /login",
        ui.command(&login)
    );
    Ok(0)
}

fn profiles_table(ui: &Ui, env: &Env, loaded: &Loaded) -> Result<()> {
    let config = &loaded.config;
    // Which profile applies in this folder. A broken rule must not hide the
    // list, so an error here just leaves the column empty.
    let here = std::env::current_dir()
        .ok()
        .and_then(|cwd| resolve::resolve(env, loaded, &cwd, None).ok().flatten())
        .map(|found| found.profile);

    let mut rows = Vec::new();
    for name in config.profiles.keys() {
        let is_default = config.default.as_deref() == Some(name);
        let is_here = here.as_deref() == Some(name);
        let rules = loaded
            .rules()
            .filter(|(rule, _)| rule.profile == *name)
            .count();
        rows.push(vec![
            Cell::styled(name, if is_here { Style::Green } else { Style::Plain }),
            account_cell(env, config, name),
            Cell::new(location(env, config, name)?),
            Cell::new(if rules == 0 {
                String::new()
            } else {
                rules.to_string()
            }),
            Cell::new(if is_default { "●" } else { "" }),
            Cell::styled(if is_here { "✓" } else { "" }, Style::Green),
        ]);
    }
    ui.table(
        &[
            "Profile",
            "Account",
            "Config folder",
            "Rules",
            "Default",
            "Here",
        ],
        &rows,
    );
    Ok(())
}

pub(crate) fn list(ui: &Ui, env: &Env) -> Result<i32> {
    let loaded = config::load(env)?;
    if loaded.config.profiles.is_empty() {
        println!(
            "No profile yet. Create one with: {}",
            ui.command("orberus add")
        );
        return Ok(0);
    }
    profiles_table(ui, env, &loaded)?;
    ui.detail("Default: used when no rule applies.   Here: used in this folder.");
    Ok(0)
}

pub(crate) fn remove(
    ui: &Ui,
    env: &Env,
    name: Option<String>,
    purge: bool,
    yes: bool,
) -> Result<i32> {
    let loaded = config::load(env)?;
    let name = pick_profile(
        ui,
        &loaded.config,
        name,
        "Which profile do you want to remove?",
    )?;
    let kept = location(env, &loaded.config, &name)?;
    let own_rules = loaded
        .config
        .rules
        .iter()
        .filter(|rule| rule.profile == name)
        .count();
    let team_rules = loaded
        .team_rules
        .iter()
        .filter(|rule| rule.profile == name)
        .count();

    if own_rules > 0 {
        ui.warning(&format!(
            "{own_rules} of your rules point to '{name}' and will be removed too"
        ));
    }
    if team_rules > 0 {
        ui.warning(&format!(
            "{team_rules} team rule(s) need '{name}': claude will refuse to start where they match"
        ));
    }
    // Checked before the question, so a refusal comes first.
    if purge {
        purge::folder(env, &loaded.config, &name)?;
        ui.warning(&format!(
            "{kept} will be deleted with its history, settings and login. This cannot be undone"
        ));
    }
    let question = if purge {
        format!("Remove profile '{name}' and delete its folder?")
    } else {
        format!("Remove profile '{name}'?")
    };
    if !confirmed(ui, &question, yes)? {
        println!("Nothing changed.");
        return Ok(0);
    }

    // `loaded` is no longer needed as a whole, so we take its config out
    // (a move) instead of copying it.
    let mut config = loaded.config;
    let was_default = config.default.as_deref() == Some(name.as_str());
    if purge {
        let purged = purge::purge_profile(env, &mut config, &name)?;
        ui.success(&format!("Profile '{name}' removed"));
        ui.detail(&format!("deleted its Claude Code folder: {kept}"));
        if purged.login {
            ui.detail("removed its login from the keychain");
        }
    } else {
        config::remove_profile(env, &mut config, &name)?;
        ui.success(&format!("Profile '{name}' removed"));
        // The folder holds the history and settings of that account.
        // Deleting it cannot be undone, so it only happens on request.
        ui.detail(&format!("its Claude Code folder is kept: {kept}"));
        ui.detail("to delete it too next time: orberus remove --purge");
    }
    if was_default {
        ui.warning(&format!(
            "It was the default profile. Choose another with: {}",
            ui.command("orberus default")
        ));
    }
    Ok(0)
}

pub(crate) fn set_default(ui: &Ui, env: &Env, name: Option<String>) -> Result<i32> {
    let mut config = config::load(env)?.config;
    let name = pick_profile(ui, &config, name, "Which profile when no rule applies?")?;
    config.default = Some(name.clone());
    config::save(env, &config)?;
    ui.success(&format!("Default profile is now '{name}'"));
    Ok(0)
}

pub(crate) fn pin(ui: &Ui, env: &Env, name: Option<String>) -> Result<i32> {
    let loaded = config::load(env)?;
    let name = pick_profile(ui, &loaded.config, name, "Which profile for this folder?")?;
    let cwd = std::env::current_dir().context("cannot read the current folder")?;
    let file = cwd.join(REPO_FILE);
    fs::write(&file, format!("{name}\n"))
        .with_context(|| format!("cannot write {}", file.display()))?;
    ui.success(&format!("This folder now uses '{name}'"));
    ui.detail(&format!("written to {}", short_path(env, &file)));

    // A locked rule beats the file we just wrote. Say so now, rather than
    // letting the user find out at the next launch.
    if let Ok(Some(found)) = resolve::resolve(env, &loaded, &cwd, None) {
        if found.profile != name {
            ui.warning(&format!(
                "claude will still use '{}' here: {}",
                found.profile,
                found.reason.describe()
            ));
        }
    }
    Ok(0)
}
