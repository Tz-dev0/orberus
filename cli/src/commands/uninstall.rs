//! `orberus uninstall`.

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use orberus_core::config;
use orberus_core::desktop;
use orberus_core::ide::{self, State};
use orberus_core::limits;
use orberus_core::login;
use orberus_core::paths::Env;
use orberus_core::pause;
use orberus_core::purge;
use orberus_core::tool;
use orberus_core::uninstall::{self, Binary};
use orberus_core::view::short_path;

use crate::commands::confirmed;
use crate::ui::Ui;

/// Undoes `setup` and the install commands. Profiles stay unless `purge`.
pub(crate) fn uninstall_command(ui: &Ui, env: &Env, purge: bool, yes: bool) -> Result<i32> {
    // A config that cannot be read must not block a removal: we then go on
    // without the list of profiles.
    let loaded = config::load(env).ok();
    // Looked for now: one of the places that tell where the app is, the
    // file that opens it at login, is deleted further down.
    let apps = uninstall::installed_apps(env);
    let binary = uninstall::binary(env)?;

    // The editors whose extension was sent through our wrapper.
    let editors: Vec<ide::Editor> = ide::editors(env)
        .into_iter()
        .filter(|editor| ide::state(env, &tool::CLAUDE, editor) == State::On)
        .collect();

    // Checked before anything is printed, so a refusal comes first.
    if purge {
        uninstall::data_folder(env)?;
        // A Claude Desktop that is open would keep writing in a folder
        // that is about to go.
        let open = desktop::open_copies(env)?;
        if !open.is_empty() {
            bail!(
                "Claude Desktop is open on profile {}: quit it, then try again",
                open.join(", ")
            );
        }
    }

    // Say what will happen before asking.
    ui.title("This removes");
    if uninstall::shim(env).is_some() {
        ui.detail("the claude shim: `claude` is the real Claude Code again");
    }
    if uninstall::shell_files_have_our_line(env) {
        ui.detail("the PATH line that setup added to your shell config");
    }
    for editor in &editors {
        ui.detail(&format!(
            "the setting that sends {}'s Claude Code extension through Orberus",
            editor.name
        ));
    }
    if let Binary::Ours(path) = &binary {
        ui.detail(&format!("orberus itself: {}", short_path(env, path)));
    }
    if login::file(env).exists() {
        ui.detail("the setting that opens the menu bar app when you log in");
    }
    for app in &apps {
        let how = if app.running.is_empty() {
            "moved to the Trash"
        } else {
            "quit, then moved to the Trash"
        };
        ui.detail(&format!(
            "the menu bar app, {how}: {}",
            short_path(env, &app.bundle)
        ));
    }

    // The profiles whose folder and login a purge deletes. The native one is
    // left out: it is ~/.claude, which is never ours to delete.
    let mut doomed: Vec<(String, PathBuf)> = Vec::new();
    if purge {
        let root = uninstall::data_folder(env)?;
        if let Some(loaded) = &loaded {
            for name in loaded.config.profiles.keys() {
                if let Some(dir) = config::profile_dir(env, &loaded.config, name)? {
                    doomed.push((name.clone(), dir));
                }
            }
        }
        ui.warning(&format!(
            "{} will be deleted: your profiles, rules and team rules",
            short_path(env, &root)
        ));
        for (name, dir) in &doomed {
            ui.warning(&format!(
                "profile '{name}': {} deleted with its history, settings and login",
                short_path(env, dir)
            ));
        }
        // The first folder is Claude Desktop's own, which is not ours.
        if desktop::data_dirs(env).len() > 1 {
            ui.warning("the data and sign-in of each profile's own Claude Desktop go with it");
        }
        ui.detail("~/.claude and Claude Desktop's own data are not touched. This cannot be undone");
    } else {
        ui.detail(&format!(
            "kept: your profiles and rules, in {} (to delete them too: --purge)",
            short_path(env, &env.root)
        ));
    }

    let question = if purge {
        "Uninstall Orberus and delete every profile?"
    } else {
        "Uninstall Orberus?"
    };
    if !confirmed(ui, question, yes)? {
        println!("Nothing changed.");
        return Ok(0);
    }

    if let Some(loaded) = &loaded {
        // Before the program goes: a status line left behind would name it.
        for name in limits::disable_all(env, &loaded.config) {
            ui.success(&format!("Status line removed from profile '{name}'"));
        }
    }
    if uninstall::remove_shim(env)? {
        ui.success("Shim removed");
    }
    // The editors first, the wrapper after: an editor must never be left
    // pointing to a file that is gone.
    let mut editors_cleared = true;
    for editor in &editors {
        match ide::turn_off(env, &tool::CLAUDE, editor) {
            Ok(_) => ui.success(&format!(
                "{}: its Claude Code extension starts Claude Code itself again",
                editor.name
            )),
            Err(error) => {
                editors_cleared = false;
                ui.warning(&format!("{error:#}"));
            }
        }
    }
    if editors_cleared {
        uninstall::remove_wrapper(env)?;
    } else {
        ui.warning(&format!(
            "{} is kept: an editor still starts it",
            short_path(env, &tool::CLAUDE.wrapper_path(env))
        ));
    }
    let shell = uninstall::clean_shell_files(env)?;
    for file in &shell.cleaned {
        ui.success(&format!("PATH line removed from {}", short_path(env, file)));
    }
    for file in &shell.still_mention {
        ui.warning(&format!(
            "{} still names the shim folder in a line setup did not write: remove it by hand",
            short_path(env, file)
        ));
    }
    for app in &apps {
        // Quit first: left running, it would stay in the menu bar with its
        // files in the Trash.
        let quit = uninstall::quit_app(app);
        if quit && !app.running.is_empty() {
            ui.success("Menu bar app quit");
        }
        // A copy that cannot be moved (another disk, a folder we may not
        // write in) is said and left: the rest of the removal goes on.
        match uninstall::trash_app(env, &app.bundle) {
            Ok(_) => ui.success(&format!(
                "{} moved to the Trash",
                short_path(env, &app.bundle)
            )),
            Err(error) => {
                ui.warning(&format!("{error:#}"));
                ui.detail("it stays where it is: move it to the Trash yourself");
            }
        }
        if !quit {
            ui.warning("the menu bar app is still running: quit it from its menu bar icon");
        }
    }
    // The app is gone: macOS must not look for it at the next login.
    if login::disable(env)? {
        ui.success("The app no longer opens at login");
    }
    uninstall::remove_launchers(env)?;
    // A pause does not outlive the tool: installed again later, Orberus
    // must not start out silently paused.
    pause::resume(env)?;

    if purge {
        for (name, dir) in &doomed {
            // The login first: its name in the keychain comes from the
            // folder's path, which is still known here.
            if purge::forget_login(dir)? {
                ui.success(&format!("Login of '{name}' removed from the keychain"));
            }
            // Folders under the data folder go with it below. One set by
            // hand elsewhere is deleted only if the usual checks allow it.
            if !dir.starts_with(&env.root) && dir.exists() {
                if let Some(loaded) = &loaded {
                    match purge::folder(env, &loaded.config, name) {
                        Ok(dir) => {
                            fs::remove_dir_all(&dir)
                                .with_context(|| format!("cannot delete {}", dir.display()))?;
                            ui.success(&format!("{} deleted", short_path(env, &dir)));
                        }
                        Err(error) => ui.warning(&format!("{error:#}")),
                    }
                }
            }
        }
    }

    // Last, because the program removes itself here.
    match uninstall::remove_binary(env)? {
        Binary::Ours(_) => ui.success("orberus removed"),
        Binary::Elsewhere(path) => {
            ui.warning(&format!(
                "orberus itself is still at {}",
                short_path(env, &path)
            ));
            ui.detail("remove it the way it was installed, for example: cargo uninstall orberus");
        }
    }
    if purge && uninstall::delete_data(env)? {
        ui.success(&format!("{} deleted", short_path(env, &env.root)));
    }

    println!();
    println!("Orberus is uninstalled. Open a new terminal.");
    if !purge {
        println!("Your profiles are kept in {}.", short_path(env, &env.root));
    }
    Ok(0)
}
