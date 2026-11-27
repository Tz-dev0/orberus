//! `orberus retention`: how long Claude Code keeps a profile's conversations.

use anyhow::{bail, Result};
use orberus_core::config::{self, Config};
use orberus_core::paths::Env;
use orberus_core::retention::{self, Retention};

use crate::ui::{Cell, Style, Ui};

/// Asks how long a profile should keep its conversations. `None` means
/// "leave it as it is".
pub(crate) fn ask_retention(
    ui: &Ui,
    env: &Env,
    config: &Config,
    name: &str,
) -> Result<Option<u64>> {
    let current = retention::read(env, config, name)?;
    let options = vec![
        format!("Leave it: {}", current.describe()),
        "90 days".to_string(),
        "1 year".to_string(),
        "Another number of days".to_string(),
    ];
    let picked = ui.select(
        "Claude Code deletes old conversations. How long should this profile keep them?",
        &options,
        &[],
        Some(0),
    )?;
    Ok(match picked {
        0 => None,
        1 => Some(90),
        2 => Some(365),
        // Asked again until it is a whole number of days, 1 or more.
        _ => loop {
            match ui.input("Number of days:")?.parse::<u64>() {
                Ok(days) if days >= 1 => break Some(days),
                _ => println!("Type a whole number of days, 1 or more."),
            }
        },
    })
}

/// `orberus retention`: how long each profile keeps its conversations.
pub(crate) fn retention_command(
    ui: &Ui,
    env: &Env,
    name: Option<String>,
    days: Option<u64>,
) -> Result<i32> {
    let loaded = config::load(env)?;
    let config = &loaded.config;
    let Some(name) = name else {
        if config.profiles.is_empty() {
            println!(
                "No profile yet. Create one with: {}",
                ui.command("orberus add")
            );
            return Ok(0);
        }
        let mut rows = Vec::new();
        for name in config.profiles.keys() {
            let kept = retention::read(env, config, name)?;
            let style = match kept {
                Retention::Days(_) => Style::Plain,
                _ => Style::Yellow,
            };
            rows.push(vec![Cell::new(name), Cell::styled(kept.describe(), style)]);
        }
        ui.table(&["Profile", "Keeps conversations"], &rows);
        ui.detail("Older conversations are deleted by Claude Code, without a message.");
        ui.detail(&format!(
            "Change it with: {}   (a project or company settings file can still override it)",
            ui.command("orberus retention <profile> <days>")
        ));
        return Ok(0);
    };
    if !config.profiles.contains_key(&name) {
        bail!("unknown profile '{name}'");
    }
    let days = match days {
        Some(days) => days,
        None if ui.interactive() => match ask_retention(ui, env, config, &name)? {
            Some(days) => days,
            None => {
                println!("Nothing changed.");
                return Ok(0);
            }
        },
        None => bail!("missing number of days, for example: orberus retention {name} 365"),
    };
    retention::set(env, config, &name, days)?;
    ui.success(&format!(
        "Profile '{name}' keeps its conversations {}",
        retention::days_text(days)
    ));
    Ok(0)
}
