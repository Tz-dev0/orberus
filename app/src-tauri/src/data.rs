//! What the popover shows and what its buttons do, without any Tauri type.
//!
//! Everything takes an `Env`, like the core, so it can be tested against a
//! temporary folder instead of the real `~/.claude-profiles`.

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use claude_profiles::config;
use claude_profiles::paths::Env;
use claude_profiles::view;

// `Serialize` lets Tauri turn these structs into JSON for the TypeScript
// side. `camelCase` gives the field names JavaScript expects (`isDefault`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    pub name: String,
    /// `None` when nobody is logged in to this profile.
    pub account: Option<String>,
    pub location: String,
    pub is_default: bool,
    pub rules: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleView {
    /// Folder the rule covers, when it has one.
    pub path: Option<String>,
    /// Git remote pattern, when it has one.
    pub git_remote: Option<String>,
    pub profile: String,
    pub locked: bool,
    /// `team.toml` or `config.toml`.
    pub source: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub profiles: Vec<ProfileView>,
    pub rules: Vec<RuleView>,
    pub shim_installed: bool,
}

fn shim_path(env: &Env) -> PathBuf {
    env.bin_dir().join("claude")
}

pub fn overview(env: &Env) -> Result<Overview> {
    let loaded = config::load(env)?;
    let config = &loaded.config;

    let mut profiles = Vec::new();
    for name in config.profiles.keys() {
        profiles.push(ProfileView {
            name: name.clone(),
            account: view::account(env, config, name),
            location: view::location(env, config, name)?,
            is_default: config.default.as_deref() == Some(name),
            rules: loaded
                .rules()
                .filter(|(rule, _)| rule.profile == *name)
                .count(),
        });
    }
    let rules = loaded
        .rules()
        .map(|(rule, source)| RuleView {
            path: rule.path.clone(),
            git_remote: rule.git_remote.clone(),
            profile: rule.profile.clone(),
            locked: rule.locked,
            source: source.label(),
        })
        .collect();

    Ok(Overview {
        profiles,
        rules,
        shim_installed: shim_path(env).exists(),
    })
}

pub fn set_default(env: &Env, name: &str) -> Result<()> {
    let mut config = config::load(env)?.config;
    if !config.profiles.contains_key(name) {
        bail!("unknown profile '{name}'");
    }
    config.default = Some(name.to_string());
    config::save(env, &config)
}

/// Wraps a text in single quotes for a shell script. A single quote inside
/// is closed, escaped and reopened: `it's` becomes `'it'\''s'`.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Writes the small script that starts Claude Code in a profile, and returns
/// its path. macOS opens a `.command` file in a new terminal window.
///
/// The script goes through the shim with an explicit choice, so a locked rule
/// still has the last word, exactly as when the user types the command.
pub fn write_launcher(env: &Env, name: &str) -> Result<PathBuf> {
    // The name ends up in a script: only accept what `cprofile add` accepts.
    config::validate_name(name)?;
    let config = config::load(env)?.config;
    if !config.profiles.contains_key(name) {
        bail!("unknown profile '{name}'");
    }
    let shim = shim_path(env);
    if !shim.exists() {
        bail!("the claude shim is not installed yet: run `cprofile setup` in a terminal");
    }

    let dir = env.root.join("run");
    fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let file = dir.join(format!("{name}.command"));
    let script = format!(
        "#!/bin/sh\n\
         # Written by Claude Profiles. Starts Claude Code with the '{name}' profile.\n\
         cd \"$HOME\" || exit 1\n\
         CLAUDE_PROFILE={name} exec {shim}\n",
        shim = shell_quote(&shim.display().to_string())
    );
    fs::write(&file, script).with_context(|| format!("cannot write {}", file.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755))
            .with_context(|| format!("cannot make {} executable", file.display()))?;
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_profiles::config::{Config, Profile, Rule};

    /// A throwaway home with two profiles and one locked rule.
    fn world() -> (tempfile::TempDir, Env) {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let env = Env {
            root: home.join(".claude-profiles"),
            home,
        };
        let mut config = Config {
            default: Some("perso".to_string()),
            ..Config::default()
        };
        config.profiles.insert(
            "perso".to_string(),
            Profile {
                native: true,
                dir: None,
            },
        );
        config
            .profiles
            .insert("work".to_string(), Profile::default());
        config.rules.push(Rule {
            path: None,
            git_remote: Some("github.com/acme/*".to_string()),
            profile: "work".to_string(),
            locked: true,
        });
        config::save(&env, &config).unwrap();
        (temp, env)
    }

    #[test]
    fn overview_lists_profiles_accounts_and_rules() {
        let (_temp, env) = world();
        let work_dir = env.profiles_dir().join("work");
        fs::create_dir_all(&work_dir).unwrap();
        fs::write(
            work_dir.join(".claude.json"),
            r#"{"oauthAccount": {"emailAddress": "me@acme.test"}}"#,
        )
        .unwrap();

        let overview = overview(&env).unwrap();
        assert!(!overview.shim_installed);
        assert_eq!(overview.profiles.len(), 2);

        let perso = &overview.profiles[0];
        assert_eq!(perso.name, "perso");
        assert!(perso.is_default);
        assert_eq!(perso.account, None);
        assert_eq!(perso.rules, 0);

        let work = &overview.profiles[1];
        assert_eq!(work.account.as_deref(), Some("me@acme.test"));
        assert_eq!(work.location, "~/.claude-profiles/profiles/work");
        assert_eq!(work.rules, 1);

        assert_eq!(overview.rules.len(), 1);
        assert_eq!(
            overview.rules[0].git_remote.as_deref(),
            Some("github.com/acme/*")
        );
        assert_eq!(overview.rules[0].path, None);
        assert!(overview.rules[0].locked);
    }

    #[test]
    fn set_default_only_accepts_an_existing_profile() {
        let (_temp, env) = world();
        set_default(&env, "work").unwrap();
        assert_eq!(
            config::load(&env).unwrap().config.default.as_deref(),
            Some("work")
        );
        assert!(set_default(&env, "nobody").is_err());
    }

    #[test]
    fn launcher_needs_the_shim_and_goes_through_it() {
        let (_temp, env) = world();
        // No shim yet: the error says what to do.
        let error = write_launcher(&env, "work").unwrap_err().to_string();
        assert!(error.contains("cprofile setup"), "{error}");

        fs::create_dir_all(env.bin_dir()).unwrap();
        fs::write(shim_path(&env), "").unwrap();
        let file = write_launcher(&env, "work").unwrap();
        let script = fs::read_to_string(&file).unwrap();
        assert!(script.starts_with("#!/bin/sh\n"), "{script}");
        assert!(script.contains("CLAUDE_PROFILE=work exec '"), "{script}");
        assert!(script.contains("/.claude-profiles/bin/claude'"), "{script}");

        // A name that could break out of the script is refused.
        assert!(write_launcher(&env, "work; rm -rf ~").is_err());
        assert!(write_launcher(&env, "nobody").is_err());
    }

    #[test]
    fn shell_quote_survives_a_single_quote() {
        assert_eq!(shell_quote("/a b/claude"), "'/a b/claude'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
