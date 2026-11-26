//! How things are shown to a person: short paths, where a profile lives, who
//! is logged in. Shared by the CLI and the menu bar app so both say the same.

use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::config::{self, Config, Rule};
use crate::paths::Env;

/// Shows a path the way people write it: `~/code` rather than `/Users/me/code`.
pub fn short_path(env: &Env, path: &Path) -> String {
    match path.strip_prefix(&env.home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Where a profile keeps its Claude Code config, for display.
pub fn location(env: &Env, config: &Config, name: &str) -> Result<String> {
    Ok(match config::profile_dir(env, config, name)? {
        Some(dir) => short_path(env, &dir),
        None => "~/.claude (Claude Code default)".to_string(),
    })
}

/// The account a profile is logged in to, or `None` when it has no login.
///
/// Read from the state file Claude Code keeps for each config folder
/// (`.claude.json`). That file is not a documented interface: we only read
/// it, and any surprise (missing file, other layout) means "no login" rather
/// than an error. The `?` after each step returns `None` as soon as one fails.
pub fn account(env: &Env, config: &Config, name: &str) -> Option<String> {
    let file = match config::profile_dir(env, config, name).ok()? {
        Some(dir) => dir.join(".claude.json"),
        // Without CLAUDE_CONFIG_DIR, Claude Code keeps it in the home folder.
        None => env.home.join(".claude.json"),
    };
    let text = fs::read_to_string(file).ok()?;
    let state: serde_json::Value = serde_json::from_str(&text).ok()?;
    let email = state.get("oauthAccount")?.get("emailAddress")?.as_str()?;
    Some(email.to_string())
}

/// A rule condition in plain words, for lists and messages.
pub fn when(rule: &Rule) -> String {
    let mut parts = Vec::new();
    if let Some(path) = &rule.path {
        parts.push(format!("folder {path}"));
    }
    if let Some(remote) = &rule.git_remote {
        parts.push(format!("git remote {remote}"));
    }
    if parts.is_empty() {
        return "no condition (never matches)".to_string();
    }
    parts.join(" and ")
}
