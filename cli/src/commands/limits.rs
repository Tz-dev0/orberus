//! `orberus limits`: the 5-hour and weekly limits of each account, and
//! `orberus statusline`, the status line that Claude Code runs to tell us.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use orberus_core::config::{self, Config};
use orberus_core::launch;
use orberus_core::limits::{self, Capture, Window};
use orberus_core::paths::Env;
use orberus_core::tool;

use crate::commands::pick_profile;
use crate::ui::{Cell, Style, Ui};

/// `on` or `off`, as typed after `orberus limits`.
#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum Switch {
    On,
    Off,
}

fn now() -> u64 {
    // A clock set before 1970 gives 0 rather than a crash.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

/// What Claude Code runs as the status line of a profile. It notes the
/// limits it is handed, then prints one line.
///
/// It never fails loudly: an error here would show under the prompt of
/// every session. Whatever goes wrong, the line is simply shorter.
pub(crate) fn statusline() -> Result<i32> {
    let mut input = String::new();
    // Claude Code writes one JSON document, then closes its end.
    let _ = std::io::stdin().read_to_string(&mut input);

    let Ok(env) = Env::from_process() else {
        return Ok(0);
    };
    let config = config::load(&env)
        .map(|loaded| loaded.config)
        .unwrap_or_default();
    let session = std::env::var(launch::SESSION_ENV).ok();
    let config_dir = std::env::var_os(tool::CLAUDE.config_env).map(PathBuf::from);
    let profile = limits::profile_of(&env, &config, session.as_deref(), config_dir.as_deref());

    let now = now();
    if let Some(profile) = &profile {
        let _ = limits::record(&env, profile, &input, now);
        // The status line the user had before ours keeps its place.
        if let Some(command) = limits::previous_command(&env, profile) {
            if run_previous(&command, &input) {
                return Ok(0);
            }
        }
        let noted = limits::read(&env, profile);
        println!("{}", limits::line(profile, noted.as_ref(), now));
    }
    Ok(0)
}

/// Runs the user's own status line with the same input, its output going
/// straight to Claude Code. False when it could not be started.
fn run_previous(command: &str, input: &str) -> bool {
    // The settings say the command "runs in a shell": so does this.
    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::piped())
        .spawn();
    let Ok(mut child) = child else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
        // `stdin` is dropped here, which closes it: the script sees the end.
    }
    child.wait().is_ok()
}

fn window_cells(window: Option<&Window>, now: u64) -> [Cell; 2] {
    let Some(window) = window else {
        return [Cell::new(""), Cell::new("")];
    };
    let reading = window.reading(now);
    let style = match reading.used {
        used if used >= 90.0 => Style::Red,
        used if used >= 70.0 => Style::Yellow,
        _ => Style::Plain,
    };
    [
        Cell::styled(limits::percent(reading.used), style),
        Cell::new(match reading.resets_in {
            Some(seconds) => format!("in {}", limits::delay_text(seconds)),
            None => "started over".to_string(),
        }),
    ]
}

fn table(ui: &Ui, env: &Env, config: &Config) -> Result<()> {
    let now = now();
    let mut rows = Vec::new();
    let mut any_off = false;
    for name in config.profiles.keys() {
        let capture = limits::capture(env, config, name)?;
        let noted = limits::read(env, name);
        let [five, five_reset] =
            window_cells(noted.as_ref().and_then(|l| l.five_hour.as_ref()), now);
        let [week, week_reset] =
            window_cells(noted.as_ref().and_then(|l| l.seven_day.as_ref()), now);
        let updated = match (&capture, &noted) {
            (Capture::Unknown(why), _) => Cell::styled(why.clone(), Style::Yellow),
            (Capture::Off, None) => {
                any_off = true;
                Cell::styled("off", Style::Dim)
            }
            (Capture::Off, Some(noted)) => {
                any_off = true;
                Cell::styled(
                    format!(
                        "{} ago, now off",
                        limits::delay_text(now.saturating_sub(noted.seen_at))
                    ),
                    Style::Dim,
                )
            }
            (Capture::On, None) => Cell::styled("waiting for Claude Code", Style::Dim),
            (Capture::On, Some(noted)) => Cell::new(format!(
                "{} ago",
                limits::delay_text(now.saturating_sub(noted.seen_at))
            )),
        };
        rows.push(vec![
            Cell::new(name),
            five,
            five_reset,
            week,
            week_reset,
            updated,
        ]);
    }
    ui.table(
        &["Profile", "5-hour", "Resets", "Weekly", "Resets", "Updated"],
        &rows,
    );
    ui.detail("The last numbers Claude Code showed for each account, not a live reading.");
    ui.detail("Rounded to a whole percent: approximate to about 1% when fresh.");
    if any_off {
        ui.detail(&format!(
            "Turn it on for a profile with: {}",
            ui.command("orberus limits on <profile>")
        ));
    }
    Ok(())
}

/// `orberus limits`, `orberus limits on|off [profile]`.
pub(crate) fn limits_command(
    ui: &Ui,
    env: &Env,
    switch: Option<Switch>,
    profile: Option<String>,
) -> Result<i32> {
    let loaded = config::load(env)?;
    let config = &loaded.config;
    let Some(switch) = switch else {
        if config.profiles.is_empty() {
            println!(
                "No profile yet. Create one with: {}",
                ui.command("orberus add")
            );
            return Ok(0);
        }
        table(ui, env, config)?;
        return Ok(0);
    };
    let name = pick_profile(ui, config, profile, "Which profile?")?;
    match switch {
        Switch::On => {
            let program = limits::program(env)?;
            limits::enable(env, config, &name, &program)?;
            ui.success(&format!("Limits of '{name}' are now noted"));
            ui.detail("Claude Code shows them under its prompt, and reports them here after the next answer in that profile.");
            if limits::previous_command(env, &name).is_some() {
                ui.detail("Your own status line is kept: it is still the one shown.");
            }
        }
        Switch::Off => {
            limits::disable(env, config, &name)?;
            ui.success(&format!("Limits of '{name}' are no longer noted"));
        }
    }
    Ok(0)
}
