//! `orberus clean`: move chosen conversations to the Trash.

use std::path::Path;

use anyhow::{bail, Result};
use orberus_core::config;
use orberus_core::disk;
use orberus_core::paths::Env;
use orberus_core::sessions::{self, Session};
use orberus_core::view::short_path;

use crate::commands::{confirmed, pick_profile, shorten};
use crate::ui::{Cell, Style, Ui};

/// `orberus clean`: the biggest conversations of a profile, and a way to
/// move some to the Trash. Without a choice it only lists.
pub(crate) fn clean(
    ui: &Ui,
    env: &Env,
    profile: Option<String>,
    wanted: Vec<String>,
    all: bool,
    yes: bool,
) -> Result<i32> {
    let loaded = config::load(env)?;
    let name = pick_profile(
        ui,
        &loaded.config,
        profile,
        "Which profile do you want to clean?",
    )?;
    let found = sessions::profile_sessions(env, &loaded.config, &name)?;
    if found.is_empty() {
        println!("Profile '{name}' has no conversation on disk.");
        return Ok(0);
    }
    let now = std::time::SystemTime::now();
    let total: u64 = found.iter().map(|session| session.size).sum();
    let shown = if all {
        found.len()
    } else {
        found.len().min(15)
    };

    let rows: Vec<Vec<Cell>> = found[..shown]
        .iter()
        .enumerate()
        .map(|(index, session)| {
            let folder = match &session.folder {
                Some(folder) => short_path(env, Path::new(folder)),
                None => "?".to_string(),
            };
            let used = if session.in_use(now) {
                Cell::styled("in use?", Style::Yellow)
            } else {
                Cell::new(sessions::ago(session.last_used, now))
            };
            vec![
                Cell::styled((index + 1).to_string(), Style::Cyan),
                Cell::styled(disk::human(session.size), Style::Bold),
                used,
                Cell::new(folder),
                Cell::new(shorten(
                    session.title.as_deref().unwrap_or("(no title)"),
                    36,
                )),
                Cell::styled(&session.id[..8], Style::Dim),
            ]
        })
        .collect();
    ui.table(&["#", "Size", "Last used", "Folder", "Title", "Id"], &rows);
    ui.detail(&format!(
        "{} conversation(s), {} in all{}",
        found.len(),
        disk::human(total),
        if shown < found.len() {
            "; the biggest are shown, --all lists every one"
        } else {
            ""
        }
    ));

    // Which ones: named with --session, or asked for in a terminal.
    let mut chosen: Vec<&Session> = Vec::new();
    if !wanted.is_empty() {
        for prefix in &wanted {
            let mut matches = found.iter().filter(|s| s.id.starts_with(prefix.as_str()));
            match (matches.next(), matches.next()) {
                (Some(session), None) => chosen.push(session),
                (None, _) => {
                    bail!("no conversation of '{name}' has an id starting with '{prefix}'")
                }
                (Some(_), Some(_)) => {
                    bail!("several conversations start with '{prefix}': give more of the id")
                }
            }
        }
    } else if ui.interactive() {
        let answer =
            ui.answer("Numbers to move to the Trash (for example: 1 3), or Enter for none:")?;
        for word in answer.split(|c: char| c == ',' || c.is_whitespace()) {
            if word.is_empty() {
                continue;
            }
            match word.parse::<usize>() {
                Ok(number) if (1..=shown).contains(&number) => chosen.push(&found[number - 1]),
                _ => bail!("'{word}' is not a number between 1 and {shown}"),
            }
        }
    }
    // The same conversation named twice counts once.
    chosen.sort_by(|a, b| a.id.cmp(&b.id));
    chosen.dedup_by(|a, b| a.id == b.id);
    if chosen.is_empty() {
        ui.detail("Nothing was changed.");
        if !ui.interactive() {
            ui.detail(&format!(
                "To move one to the Trash: orberus clean {name} --session <id> --yes"
            ));
        }
        return Ok(0);
    }

    let trash = sessions::trash_dir(env)?;
    let freed: u64 = chosen.iter().map(|session| session.size).sum();
    ui.warning(&format!(
        "{} conversation(s), {}, will move to the Trash. `claude --resume` will no longer find them",
        chosen.len(),
        disk::human(freed)
    ));
    if !confirmed(ui, "Move them to the Trash?", yes)? {
        println!("Nothing changed.");
        return Ok(0);
    }
    let mut failures = 0;
    for session in chosen {
        match sessions::move_to_trash(session, &trash, now) {
            Ok(_) => ui.success(&format!(
                "{}  {} moved to the Trash",
                &session.id[..8],
                disk::human(session.size)
            )),
            Err(error) => {
                failures += 1;
                ui.failure(&format!("{error:#}"));
            }
        }
    }
    ui.detail("The space is freed when you empty the Trash. Each one holds a note to put it back.");
    Ok(if failures == 0 { 0 } else { 1 })
}
