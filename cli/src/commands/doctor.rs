//! `orberus doctor`: check the installation.

use anyhow::Result;
use orberus_core::config;
use orberus_core::launch;
use orberus_core::paths::Env;
use orberus_core::retention::{self, Retention};
use orberus_core::tool;
use orberus_core::view::short_path;

use crate::commands::setup::path_missing_in;
use crate::ui::Ui;

pub(crate) fn doctor(ui: &Ui, env: &Env) -> Result<i32> {
    let mut failures = 0;
    let mut check = |ok: bool, label: &str, hint: &str| {
        if ok {
            ui.success(label);
        } else {
            failures += 1;
            ui.failure(label);
            ui.detail(hint);
        }
    };

    let loaded = config::load(env);
    check(
        loaded.is_ok(),
        "config files are valid",
        "fix the file named in the error below",
    );
    if let Err(error) = &loaded {
        ui.detail(&format!("{error:#}"));
    }
    let loaded = loaded.unwrap_or_default();

    let tool = &tool::CLAUDE;
    let shim = tool.shim_path(env);
    check(shim.exists(), "shim is installed", "run: orberus setup");

    let found = launch::in_path(tool);
    let first_is_shim = found.first().is_some_and(|(_, is_shim)| *is_shim);
    check(
        first_is_shim,
        "`claude` in your PATH is the shim",
        &format!(
            "put {} first in your PATH (orberus setup can do it), then open a new terminal",
            short_path(env, &env.bin_dir())
        ),
    );

    // A terminal can be fine while apps and scripts skip the shim: they
    // start a shell that reads other files.
    let missing = path_missing_in(env);
    let names: Vec<String> = missing.iter().map(|file| short_path(env, file)).collect();
    check(
        missing.is_empty(),
        "apps and scripts get the shim too",
        &format!(
            "no PATH line for the shim in {}: run orberus setup",
            names.join(" and ")
        ),
    );

    let real = launch::find_real(env, &loaded.config, tool);
    check(
        real.is_ok(),
        "real Claude Code is found",
        "install Claude Code or set claude_bin in config.toml",
    );
    if let Ok(real) = &real {
        ui.detail(&short_path(env, real));
    }

    check(
        !loaded.config.profiles.is_empty(),
        "at least one profile exists",
        "run: orberus add",
    );
    let default_ok = match &loaded.config.default {
        Some(name) => loaded.config.profiles.contains_key(name),
        None => true,
    };
    check(default_ok, "default profile exists", "run: orberus default");
    let rules_ok = loaded
        .rules()
        .all(|(rule, _)| loaded.config.profiles.contains_key(&rule.profile));
    check(
        rules_ok,
        "every rule points to an existing profile",
        "see: orberus rule list",
    );

    let outside = std::env::var_os(tool.config_env).is_some()
        && std::env::var_os(launch::SESSION_ENV).is_none();
    check(
        !outside,
        &format!("{} is not set by something else", tool.config_env),
        "remove it from your shell config: orberus sets it for each launch",
    );

    // Not a failure: a warning about what Claude Code will delete by itself.
    for name in loaded.config.profiles.keys() {
        match retention::read(env, &loaded.config, name) {
            Ok(Retention::Days(_)) | Err(_) => {}
            Ok(kept) => {
                ui.warning(&format!(
                    "'{name}' keeps its conversations {}",
                    kept.describe()
                ));
                ui.detail(&format!(
                    "older ones are deleted by Claude Code; to keep them: orberus retention {name} {}",
                    retention::SUGGESTED_DAYS
                ));
            }
        }
    }

    println!();
    if failures == 0 {
        ui.success("Everything is in place");
        Ok(0)
    } else {
        ui.failure(&format!("{failures} problem(s) to fix"));
        Ok(1)
    }
}
