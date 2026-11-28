//! `orberus pause` and `orberus resume`.

use anyhow::Result;
use orberus_core::paths::Env;
use orberus_core::pause;

use crate::ui::Ui;

pub(crate) fn pause_command(ui: &Ui, env: &Env) -> Result<i32> {
    if pause::pause(env)? {
        ui.success("Orberus is paused");
    } else {
        ui.success("Orberus was already paused");
    }
    ui.detail("claude now starts as if Orberus was not installed: no account is picked,");
    ui.detail("and no rule applies, locked ones included. In your editors too.");
    println!(
        "To pick the account by folder again: {}",
        ui.command("orberus resume")
    );
    Ok(0)
}

pub(crate) fn resume_command(ui: &Ui, env: &Env) -> Result<i32> {
    if pause::resume(env)? {
        ui.success("Orberus picks the account by folder again");
    } else {
        ui.success("Orberus was not paused");
    }
    Ok(0)
}
