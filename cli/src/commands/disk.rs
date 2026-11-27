//! `orberus disk`: the space each profile takes.

use anyhow::Result;
use orberus_core::config;
use orberus_core::disk;
use orberus_core::paths::Env;
use orberus_core::view::location;

use crate::ui::{Cell, Style, Ui};

/// `orberus disk`: the space each profile takes, and what takes it.
pub(crate) fn disk_usage(ui: &Ui, env: &Env) -> Result<i32> {
    let loaded = config::load(env)?;
    if loaded.config.profiles.is_empty() {
        println!(
            "No profile yet. Create one with: {}",
            ui.command("orberus add")
        );
        return Ok(0);
    }
    let mut rows = Vec::new();
    let mut all = 0;
    for name in loaded.config.profiles.keys() {
        let usage = disk::profile_usage(env, &loaded.config, name)?;
        all += usage.total();
        rows.push(vec![
            Cell::new(name),
            Cell::styled(disk::human(usage.total()), Style::Bold),
            Cell::new(disk::human(usage.sessions)),
            Cell::new(disk::human(usage.file_history)),
            Cell::new(disk::human(usage.extensions)),
            Cell::new(disk::human(usage.other)),
            Cell::styled(location(env, &loaded.config, name)?, Style::Dim),
        ]);
    }
    ui.table(
        &[
            "Profile",
            "Total",
            "Sessions",
            "File history",
            "Plugins, skills",
            "Other",
            "Folder",
        ],
        &rows,
    );
    if rows.len() > 1 {
        ui.detail(&format!("All profiles: {}", disk::human(all)));
    }
    ui.detail("Sessions: your conversations, in projects/. `claude --resume` needs them.");
    ui.detail("Nothing is deleted by this command.");
    Ok(0)
}
