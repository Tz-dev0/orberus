//! `orberus usage`: where a profile's usage went, by project or by conversation.

use std::path::Path;

use anyhow::{bail, Result};
use orberus_core::config;
use orberus_core::paths::Env;
use orberus_core::sessions;
use orberus_core::usage::{self, Verdict};
use orberus_core::view::{account, short_path};

use crate::commands::{shorten, unknown_profile};
use crate::ui::{Cell, Style, Ui};

/// `orberus usage`: which folders used each account, checked against the
/// rules as they are today.
pub(crate) fn usage_command(
    ui: &Ui,
    env: &Env,
    profile: Option<String>,
    days: u64,
    conversations: bool,
    all: bool,
) -> Result<i32> {
    const SHOWN: usize = 10;
    let loaded = config::load(env)?;
    let config = &loaded.config;
    if config.profiles.is_empty() {
        println!(
            "No profile yet. Create one with: {}",
            ui.command("orberus add")
        );
        return Ok(0);
    }
    if days == 0 {
        bail!("--days must be 1 or more");
    }
    let names: Vec<String> = match profile {
        Some(name) if config.profiles.contains_key(&name) => vec![name],
        Some(name) => return Err(unknown_profile(config, &name)),
        None => config.profiles.keys().cloned().collect(),
    };
    let period = if days == 1 {
        "last 24 hours".to_string()
    } else {
        format!("last {days} days")
    };

    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            println!();
        }
        let who = account(env, config, name).unwrap_or_else(|| "not logged in".to_string());
        println!(
            "{}  {}",
            ui.paint(name, Style::Bold),
            ui.paint(&format!("{who}, {period}"), Style::Dim)
        );
        let used = usage::profile_usage(env, &loaded, name, days)?;
        if used.folders.is_empty() {
            ui.detail("No conversation in this period.");
            continue;
        }
        let whole = used.total.weight();
        if conversations {
            let shown = if all { usize::MAX } else { SHOWN };
            conversations_table(ui, env, &used, shown);
            continue;
        }

        let mut rows = Vec::new();
        // Projects no rule names, and projects that would use another
        // profile today: (weight, how many, the heaviest one, kept for the
        // advice). Projects come heaviest first, so the first seen stays.
        let mut uncovered: (f64, usize, Option<&Path>) = (0.0, 0, None);
        let mut elsewhere: (f64, usize, Option<&Path>) = (0.0, 0, None);
        let mut hidden = (0.0, 0usize);
        for (rank, folder) in used.folders.iter().enumerate() {
            let weight = folder.tokens.weight();
            let cell = match usage::verdict(env, &loaded, name, &folder.folder) {
                Verdict::Covered(reason) => Cell::styled(
                    format!("✓ {}", shorten(&usage::rule_text(&reason), 46)),
                    Style::Green,
                ),
                Verdict::NoRule => {
                    uncovered = (
                        uncovered.0 + weight,
                        uncovered.1 + 1,
                        uncovered.2.or(Some(&folder.folder)),
                    );
                    Cell::styled("no rule, default profile", Style::Yellow)
                }
                Verdict::Elsewhere { profile, reason } => {
                    elsewhere = (
                        elsewhere.0 + weight,
                        elsewhere.1 + 1,
                        elsewhere.2.or(Some(&folder.folder)),
                    );
                    Cell::styled(
                        format!(
                            "would use {profile} today ({})",
                            shorten(&usage::rule_text(&reason), 30)
                        ),
                        Style::Yellow,
                    )
                }
                Verdict::Gone => Cell::styled("folder gone, not checked", Style::Dim),
                Verdict::Unknown(why) => Cell::styled(shorten(&why, 48), Style::Red),
            };
            if !all && rank >= SHOWN {
                hidden = (hidden.0 + weight, hidden.1 + 1);
                continue;
            }
            rows.push(vec![
                Cell::styled(usage::percent(weight, whole), Style::Bold),
                Cell::new(shorten(&short_path(env, &folder.folder), 44)),
                Cell::new(folder.answers.to_string()),
                cell,
            ]);
        }
        if hidden.1 > 0 {
            rows.push(vec![
                Cell::styled(usage::percent(hidden.0, whole), Style::Dim),
                Cell::styled(format!("{} more, see --all", hidden.1), Style::Dim),
                Cell::new(""),
                Cell::new(""),
            ]);
        }
        ui.table(&["Share", "Project", "Answers", "Today's rules"], &rows);

        // With no rule at all, "no rule names it" is true of every project
        // and teaches nothing: say the one useful thing instead.
        let no_rule_yet = loaded.rules().next().is_none();
        if uncovered.1 > 0 && no_rule_yet {
            ui.detail(&format!(
                "You have no rule yet, so every folder uses the default profile. Add one with: {}",
                ui.command("orberus rule add")
            ));
        } else if uncovered.1 > 0 {
            ui.warning(&format!(
                "{} came from {} project(s) no rule names: '{name}' is used there only as the default.",
                usage::percent(uncovered.0, whole),
                uncovered.1
            ));
            if let Some(folder) = uncovered.2 {
                ui.detail(&format!(
                    "If one belongs to another account, add a rule: {}",
                    ui.command(&format!(
                        "cd {} && orberus rule add",
                        short_path(env, folder)
                    ))
                ));
            }
        }
        if elsewhere.1 > 0 {
            ui.warning(&format!(
                "{} came from {} project(s) where `claude` would use another profile today.",
                usage::percent(elsewhere.0, whole),
                elsewhere.1
            ));
            ui.detail(&format!(
                "They ran with '{name}' by an explicit choice, or the rules changed since."
            ));
            if let Some(folder) = elsewhere.2 {
                ui.detail(&format!(
                    "To keep one on '{name}': {}",
                    ui.command(&format!(
                        "cd {} && orberus use {name}",
                        short_path(env, folder)
                    ))
                ));
            }
        }
        if uncovered.1 == 0 && elsewhere.1 == 0 {
            ui.success("Every project used with this profile is sent here by a rule or a pin.");
        }
    }
    println!();
    ui.detail(
        "Share: an estimate from the conversations kept on this Mac, not your plan's limits.",
    );
    if conversations {
        ui.detail("Context: what is sent to Claude with each answer, on average. It grows with the conversation.");
    } else {
        ui.detail("Today's rules: where `claude` would go if you started it in that folder now.");
    }
    Ok(0)
}

/// The conversations of a profile, heaviest first, with what each one sends
/// again at every answer.
fn conversations_table(ui: &Ui, env: &Env, used: &usage::ProfileUsage, shown: usize) {
    const HEAVY: u64 = usage::HEAVY_CONTEXT;
    let whole = used.total.weight();
    let mut rows = Vec::new();
    let mut heavy = (0.0, 0usize);
    let mut hidden = (0.0, 0usize);
    for (rank, conversation) in used.conversations.iter().enumerate() {
        let weight = conversation.tokens.weight();
        let is_heavy = conversation.context >= HEAVY;
        if is_heavy {
            heavy = (heavy.0 + weight, heavy.1 + 1);
        }
        if rank >= shown {
            hidden = (hidden.0 + weight, hidden.1 + 1);
            continue;
        }
        // Read only for the lines shown: it opens the file.
        let title = match sessions::title(&conversation.transcript) {
            Some(title) => Cell::new(shorten(&title, 32)),
            None => Cell::styled("(untitled)", Style::Dim),
        };
        let context_style = if is_heavy {
            Style::Yellow
        } else {
            Style::Plain
        };
        rows.push(vec![
            Cell::styled(usage::percent(weight, whole), Style::Bold),
            Cell::styled(usage::tokens_text(conversation.context), context_style),
            Cell::new(conversation.answers.to_string()),
            Cell::new(shorten(&short_path(env, &conversation.folder), 30)),
            title,
            // `chars` and not a byte slice: an id shorter than 8 bytes must
            // not panic.
            Cell::styled(
                conversation.id.chars().take(8).collect::<String>(),
                Style::Dim,
            ),
        ]);
    }
    if hidden.1 > 0 {
        rows.push(vec![
            Cell::styled(usage::percent(hidden.0, whole), Style::Dim),
            Cell::new(""),
            Cell::new(""),
            Cell::styled(format!("{} more, see --all", hidden.1), Style::Dim),
            Cell::new(""),
            Cell::new(""),
        ]);
    }
    ui.table(
        &["Share", "Context", "Answers", "Project", "Title", "Id"],
        &rows,
    );
    ui.detail(&format!(
        "About {:.0}% of this usage went to reading context again, the rest to writing answers.",
        used.total.reread_share() * 100.0
    ));
    if heavy.1 > 0 {
        ui.warning(&format!(
            "{} conversation(s) send more than {} tokens with each answer, for {} of this usage.",
            heavy.1,
            usage::tokens_text(HEAVY),
            usage::percent(heavy.0, whole)
        ));
        ui.detail(&format!(
            "To lighten one: {} inside it, or a new conversation when the subject changes.",
            ui.command("/compact")
        ));
    }
}
