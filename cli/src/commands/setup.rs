//! `orberus setup`: the `claude` link and the `PATH` line.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use orberus_core::paths::Env;
use orberus_core::shell;
use orberus_core::tool;
use orberus_core::view::short_path;

use crate::ui::Ui;

/// The name of the user's shell: `zsh`, `bash`.
fn shell_name() -> Option<String> {
    let shell = std::env::var("SHELL").ok()?;
    Some(Path::new(&shell).file_name()?.to_str()?.to_string())
}

/// The shell config files that should put the shim in `PATH` and do not.
///
/// The first file of a shell is the one a terminal reads: when the shim
/// folder is already in this terminal's `PATH`, it got there somehow, and
/// that file is left alone. The others (apps, scripts) are always checked.
pub(crate) fn path_missing_in(env: &Env) -> Vec<PathBuf> {
    let bin = env.bin_dir();
    let in_path = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir == bin));
    let files = shell_name()
        .map(|name| shell::configs(env, &name))
        .unwrap_or_default();
    files
        .into_iter()
        .enumerate()
        .filter(|(index, file)| !shell::mentions(env, file) && !(*index == 0 && in_path))
        .map(|(_, file)| file)
        .collect()
}

pub(crate) fn setup(ui: &Ui, env: &Env) -> Result<i32> {
    let bin = env.bin_dir();
    fs::create_dir_all(&bin).with_context(|| format!("cannot create {}", bin.display()))?;
    fs::create_dir_all(env.profiles_dir())?;
    let me = std::env::current_exe()?.canonicalize()?;

    // One shim today. With a second tool, this block runs once per entry
    // of `tool::ALL`.
    let tool = &tool::CLAUDE;
    #[cfg(unix)]
    let link = {
        let link = tool.shim_path(env);
        match fs::remove_file(&link) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| format!("cannot replace {}", link.display()))
            }
        }
        std::os::unix::fs::symlink(&me, &link)
            .with_context(|| format!("cannot create {}", link.display()))?;
        link
    };
    #[cfg(not(unix))]
    let link = {
        let link = tool.shim_path(env);
        fs::copy(&me, &link).with_context(|| format!("cannot create {}", link.display()))?;
        link
    };

    ui.success(&format!("Shim installed: {}", short_path(env, &link)));
    setup_path(ui, env, &bin)?;

    println!();
    ui.title("Next steps");
    ui.commands(&[
        ("orberus add", "create a profile, step by step"),
        (
            "orberus rule add",
            "send a repository or a folder to a profile",
        ),
        ("orberus doctor", "check the installation"),
    ]);
    Ok(0)
}

/// Makes sure the shim folder is in PATH, or says how to put it there.
fn setup_path(ui: &Ui, env: &Env, bin: &Path) -> Result<()> {
    let in_path = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir == bin));
    let known_shell = shell_name().is_some_and(|name| !shell::configs(env, &name).is_empty());
    let missing = path_missing_in(env);

    if missing.is_empty() && (in_path || known_shell) {
        if in_path {
            ui.success("The shim folder is in your PATH");
        } else {
            // Written by an earlier setup: only a new terminal is missing.
            ui.success("Your shell config already puts the shim in your PATH");
            ui.detail("open a new terminal for it to apply");
        }
        return Ok(());
    }

    if !missing.is_empty() {
        let names: Vec<String> = missing.iter().map(|file| short_path(env, file)).collect();
        let question = format!("Add the shim to your PATH in {}?", names.join(" and "));
        if ui.interactive() && ui.confirm(&question, true)? {
            for file in &missing {
                shell::add_line(env, file)?;
                ui.success(&format!("Added to {}", short_path(env, file)));
            }
            ui.detail("a terminal reads the first file; apps and scripts read the other");
            ui.detail("open a new terminal for it to apply");
            return Ok(());
        }
    }

    if in_path {
        // This terminal is fine; what is missing is for apps and scripts.
        ui.success("The shim folder is in your PATH");
        ui.warning("Apps and scripts that start their own shell would skip the shim");
    } else {
        ui.warning("The shim folder is not in your PATH yet");
    }
    println!();
    if missing.is_empty() {
        println!("Add this line to your shell config, then open a new terminal:");
    } else {
        let names: Vec<String> = missing.iter().map(|file| short_path(env, file)).collect();
        println!(
            "Add this line to {}, then open a new terminal:",
            names.join(" and ")
        );
    }
    println!();
    println!("  {}", ui.command(&shell::path_line(env)));
    Ok(())
}
