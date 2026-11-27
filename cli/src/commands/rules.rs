//! `orberus rule add`, `rule list`, `rule remove`.

use anyhow::{bail, Context, Result};
use orberus_core::config::{self, Rule};
use orberus_core::paths::Env;
use orberus_core::rules;
use orberus_core::view::when;

use crate::commands::{confirmed, pick_profile};
use crate::ui::{Cell, Style, Ui};

/// Builds a rule condition from the current folder, by asking.
/// Returns `(path, git_remote)`.
fn ask_condition(ui: &Ui, env: &Env) -> Result<(Option<String>, Option<String>)> {
    let cwd = std::env::current_dir().context("cannot read the current folder")?;
    let mut choices = rules::choices(env, &cwd);
    // Each line also shows the condition as it will be stored.
    let options: Vec<String> = choices
        .iter()
        .map(|choice| match (&choice.git_remote, &choice.path) {
            (Some(remote), _) => format!("{}  (git remote {remote})", choice.label),
            _ => choice.label.clone(),
        })
        .collect();
    let index = ui.select("Where should the rule apply?", &options, &[], Some(0))?;
    let choice = choices.swap_remove(index);
    Ok((choice.path, choice.git_remote))
}

pub(crate) fn rule_add(
    ui: &Ui,
    env: &Env,
    path: Option<String>,
    git_remote: Option<String>,
    profile: Option<String>,
    mut locked: bool,
) -> Result<i32> {
    let mut config = config::load(env)?.config;

    // No condition given: guided mode, built from the current folder.
    let guided = path.is_none() && git_remote.is_none();
    let (path, git_remote) = if !guided {
        let path = path.map(|path| portable_path(env, &path)).transpose()?;
        (path, git_remote)
    } else if ui.interactive() {
        if config.profiles.is_empty() {
            bail!("no profile yet, create one with: orberus add");
        }
        ask_condition(ui, env)?
    } else {
        bail!(
            "a rule needs --path, --git-remote, or both, for example: \
             orberus rule add --git-remote \"github.com/acme/*\" --profile work"
        );
    };

    let profile = pick_profile(ui, &config, profile, "Which profile should it use?")?;
    if guided && !locked {
        locked = ui.confirm(
            "Lock it? A locked rule refuses any other profile there.",
            false,
        )?;
    }

    let rule = Rule {
        path,
        git_remote,
        profile,
        locked,
    };
    let summary = format!(
        "{} → {}{}",
        when(&rule),
        rule.profile,
        if locked { " (locked)" } else { "" }
    );
    rules::add(env, &mut config, rule)?;
    ui.success(&format!("Rule added: {summary}"));
    Ok(0)
}

/// Stores a path rule in a form that survives a different working folder:
/// relative paths become absolute, and the home folder is written as `~`.
fn portable_path(env: &Env, path: &str) -> Result<String> {
    if path.starts_with('~') {
        return Ok(path.to_string());
    }
    // `join` keeps an absolute path as it is and completes a relative one.
    let absolute = std::env::current_dir()?.join(path);
    Ok(rules::portable_path(env, &absolute))
}

pub(crate) fn rule_list(ui: &Ui, env: &Env) -> Result<i32> {
    let loaded = config::load(env)?;
    let rows: Vec<Vec<Cell>> = loaded
        .rules()
        .enumerate()
        .map(|(index, (rule, source))| {
            vec![
                Cell::new((index + 1).to_string()),
                Cell::new(when(rule)),
                Cell::new(&rule.profile),
                Cell::styled(if rule.locked { "locked" } else { "" }, Style::Yellow),
                Cell::styled(source.label(), Style::Dim),
            ]
        })
        .collect();
    if rows.is_empty() {
        println!(
            "No rule yet. Create one from this folder with: {}",
            ui.command("orberus rule add")
        );
        return Ok(0);
    }
    ui.table(&["#", "When", "Profile", "Lock", "From"], &rows);
    ui.detail("Checked from top to bottom. A locked rule wins over everything.");
    Ok(0)
}

pub(crate) fn rule_remove(ui: &Ui, env: &Env, number: Option<usize>, yes: bool) -> Result<i32> {
    let loaded = config::load(env)?;
    // Numbers follow `rule list`: team rules come first, then the user's.
    let team = loaded.team_rules.len();
    let mut config = loaded.config;
    if config.rules.is_empty() {
        bail!("you have no rule of your own to remove (see: orberus rule list)");
    }

    let index = match number {
        Some(number) if number == 0 || number > team + config.rules.len() => {
            bail!("there is no rule {number} (see: orberus rule list)")
        }
        Some(number) if number <= team => bail!(
            "rule {number} comes from team.toml, which your company provides: it cannot be removed here"
        ),
        Some(number) => number - team - 1,
        None if ui.interactive() => {
            let options: Vec<String> = config
                .rules
                .iter()
                .map(|rule| format!("{} → {}", when(rule), rule.profile))
                .collect();
            ui.select("Which rule do you want to remove?", &options, &[], None)?
        }
        None => bail!("missing rule number (see: orberus rule list)"),
    };

    let summary = format!(
        "{} → {}",
        when(&config.rules[index]),
        config.rules[index].profile
    );
    if !confirmed(ui, &format!("Remove the rule {summary}?"), yes)? {
        println!("Nothing changed.");
        return Ok(0);
    }
    rules::remove(env, &mut config, index)?;
    ui.success(&format!("Rule removed: {summary}"));
    Ok(0)
}
