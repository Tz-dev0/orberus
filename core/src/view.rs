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
        None => {
            let tool = &crate::tool::CLAUDE;
            format!(
                "{} ({} default)",
                short_path(env, &tool.native_dir(env)),
                tool.name
            )
        }
    })
}

/// The account a profile is logged in to, or `None` when it has no login.
///
/// Read from the state file Claude Code keeps for each config folder
/// (`.claude.json`). That file is not a documented interface: we only read
/// it, and any surprise (missing file, other layout) means "no login" rather
/// than an error. The `?` after each step returns `None` as soon as one fails.
pub fn account(env: &Env, config: &Config, name: &str) -> Option<String> {
    let login = login(env, config, name)?;
    let email = login.get("emailAddress")?.as_str()?;
    Some(email.to_string())
}

/// The subscription of the account a profile is logged in to, as a short
/// name: "Max 5x", "Pro", "Team". `None` when there is no login, or when the
/// file does not say it the way we know.
///
/// Same source and same caution as [`account`]. Seen on Claude Code 2.1.323
/// with a Max subscription: `organizationType` is `claude_max` and
/// `organizationRateLimitTier` is `default_claude_max_5x`.
pub fn plan(env: &Env, config: &Config, name: &str) -> Option<String> {
    let login = login(env, config, name)?;
    let kind = login.get("organizationType")?.as_str()?;
    let tier = login
        .get("organizationRateLimitTier")
        .and_then(|tier| tier.as_str());
    plan_name(kind, tier)
}

/// What Claude Code wrote about the login of a profile: the `oauthAccount`
/// object of its state file.
fn login(env: &Env, config: &Config, name: &str) -> Option<serde_json::Value> {
    let file = match config::profile_dir(env, config, name).ok()? {
        Some(dir) => dir.join(".claude.json"),
        // Without CLAUDE_CONFIG_DIR, Claude Code keeps it in the home folder.
        None => env.home.join(".claude.json"),
    };
    let text = fs::read_to_string(file).ok()?;
    let mut state: serde_json::Value = serde_json::from_str(&text).ok()?;
    // `take` moves the object out of the document instead of copying it.
    Some(state.get_mut("oauthAccount")?.take())
}

/// `claude_max` and `default_claude_max_5x` give "Max 5x".
///
/// The name is derived, not looked up in a list, so a subscription that
/// appears later still gets a name. A kind that does not start with
/// `claude_` is not one we understand: no name rather than a wrong one.
fn plan_name(kind: &str, tier: Option<&str>) -> Option<String> {
    let kind = kind.strip_prefix("claude_")?;
    let words: Vec<String> = kind
        .split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            // Upper-case the first letter, keep the rest.
            let mut letters = word.chars();
            match letters.next() {
                Some(first) => first.to_uppercase().chain(letters).collect(),
                None => String::new(),
            }
        })
        .collect();
    if words.is_empty() {
        return None;
    }
    let mut name = words.join(" ");
    // The multiplier is the last piece of the tier when it reads like "5x".
    let times = tier
        .and_then(|tier| tier.rsplit('_').next())
        .filter(|last| {
            last.strip_suffix('x')
                .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        });
    if let Some(times) = times {
        name.push(' ');
        name.push_str(times);
    }
    Some(name)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_name_reads_kind_and_multiplier() {
        assert_eq!(
            plan_name("claude_max", Some("default_claude_max_5x")).as_deref(),
            Some("Max 5x")
        );
        assert_eq!(
            plan_name("claude_max", Some("default_claude_max_20x")).as_deref(),
            Some("Max 20x")
        );
    }

    #[test]
    fn plan_name_without_multiplier() {
        assert_eq!(plan_name("claude_pro", None).as_deref(), Some("Pro"));
        assert_eq!(
            plan_name("claude_team", Some("default_claude_team")).as_deref(),
            Some("Team")
        );
        // A tier that only looks like a multiplier is left out.
        assert_eq!(
            plan_name("claude_pro", Some("default_x")).as_deref(),
            Some("Pro")
        );
    }

    #[test]
    fn plan_name_of_an_unknown_shape_is_none() {
        assert_eq!(plan_name("something_else", None), None);
        assert_eq!(plan_name("claude_", None), None);
    }

    #[test]
    fn plan_and_account_come_from_the_profile_state_file() {
        let home = tempfile::tempdir().unwrap();
        let env = Env {
            home: home.path().to_path_buf(),
            root: home.path().join(".orberus"),
        };
        let mut config = Config::default();
        config.profiles.insert("work".into(), Default::default());
        assert_eq!(plan(&env, &config, "work"), None);

        let dir = config::profile_dir(&env, &config, "work").unwrap().unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"me@acme.test","organizationType":"claude_max","organizationRateLimitTier":"default_claude_max_5x"}}"#,
        )
        .unwrap();
        assert_eq!(
            account(&env, &config, "work").as_deref(),
            Some("me@acme.test")
        );
        assert_eq!(plan(&env, &config, "work").as_deref(), Some("Max 5x"));
    }
}
