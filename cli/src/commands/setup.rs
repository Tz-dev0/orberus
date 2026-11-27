//! `orberus setup`: the `claude` link and the `PATH` line.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use orberus_core::paths::Env;
use orberus_core::tool;
use orberus_core::uninstall;
use orberus_core::view::short_path;

use crate::ui::Ui;

/// The config file of the user's shell, when we know how to write to it.
fn shell_config(env: &Env) -> Option<PathBuf> {
    let shell = std::env::var("SHELL").ok()?;
    let name = Path::new(&shell).file_name()?.to_str()?.to_string();
    match name.as_str() {
        "zsh" => Some(env.home.join(".zshrc")),
        // Terminal on macOS starts bash as a login shell, which reads
        // ~/.bash_profile and not ~/.bashrc.
        "bash" if cfg!(target_os = "macos") => Some(env.home.join(".bash_profile")),
        "bash" => Some(env.home.join(".bashrc")),
        _ => None,
    }
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
    if in_path {
        ui.success("The shim folder is in your PATH");
        return Ok(());
    }

    // Written with $HOME when possible, so the line still works if the home
    // folder moves (a restored backup, a renamed account).
    let absolute = bin.display().to_string();
    let with_home = bin
        .strip_prefix(&env.home)
        .ok()
        .map(|rest| format!("$HOME/{}", rest.display()));
    let line = format!(
        "export PATH=\"{}:$PATH\"",
        with_home.as_deref().unwrap_or(&absolute)
    );
    let rc = shell_config(env);

    // Already written by an earlier setup: only a new terminal is missing.
    // We look for both spellings of the folder, absolute and with $HOME.
    if let Some(rc) = &rc {
        let text = fs::read_to_string(rc).unwrap_or_default();
        let mentioned = text.contains(&absolute)
            || with_home
                .as_deref()
                .is_some_and(|spelling| text.contains(spelling));
        if mentioned {
            ui.success(&format!(
                "{} already puts the shim in your PATH",
                short_path(env, rc)
            ));
            ui.detail("open a new terminal for it to apply");
            return Ok(());
        }
    }

    if let Some(rc) = &rc {
        let question = format!("Add the shim to your PATH in {}?", short_path(env, rc));
        if ui.interactive() && ui.confirm(&question, true)? {
            // `append(true)` adds at the end and never rewrites what is there.
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(rc)
                .with_context(|| format!("cannot open {}", rc.display()))?;
            writeln!(file, "\n{}\n{line}", uninstall::PATH_COMMENT)
                .with_context(|| format!("cannot write {}", rc.display()))?;
            ui.success(&format!("Added to {}", short_path(env, rc)));
            ui.detail("open a new terminal for it to apply");
            return Ok(());
        }
    }

    ui.warning("The shim folder is not in your PATH yet");
    println!();
    println!("Add this line to your shell config, then open a new terminal:");
    println!();
    println!("  {}", ui.command(&line));
    Ok(())
}
