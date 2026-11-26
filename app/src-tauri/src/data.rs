//! What the popover shows and what its buttons do, without any Tauri type.
//!
//! Everything takes an `Env`, like the core, so it can be tested against a
//! temporary folder instead of the real `~/.claude-profiles`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;

use claude_profiles::config::{self, RuleSource};
use claude_profiles::disk;
use claude_profiles::paths::Env;
use claude_profiles::purge;
use claude_profiles::retention::{self, Retention};
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
    /// How many of those rules come from the team file. They stay when the
    /// profile is removed.
    pub team_rules: usize,
    /// False when its folder must never be deleted, as for `~/.claude`.
    pub can_purge: bool,
    /// How long it keeps its conversations, in words: "30 days".
    pub keeps: String,
    /// The same in days. `None` when its settings file cannot be read,
    /// and then it cannot be changed from here either.
    pub keeps_days: Option<u64>,
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
    /// True while no profile reuses `~/.claude`: a new one may take it.
    pub can_reuse_login: bool,
}

/// The disk space one profile takes, for the "Storage" lines.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageView {
    pub name: String,
    /// In bytes, to draw the bars to scale.
    pub total: u64,
    /// The same, for people: "3.4 GB".
    pub size: String,
    /// What takes it, in words, biggest first.
    pub detail: String,
}

/// The "Storage" section: every profile, and the sum shown while the
/// section is folded.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageOverview {
    pub profiles: Vec<StorageView>,
    /// All profiles together, for people: "3.5 GB".
    pub total: String,
}

/// Measures every profile. This walks whole folders, so the page asks for
/// it apart from the profile list, which must show at once.
pub fn storage(env: &Env) -> Result<StorageOverview> {
    let config = config::load(env)?.config;
    let mut views = Vec::new();
    for name in config.profiles.keys() {
        let usage = disk::profile_usage(env, &config, name)?;
        let mut parts = [
            (usage.sessions, "conversations"),
            (usage.file_history, "file history"),
            (usage.extensions, "plugins and skills"),
            (usage.other, "other"),
        ];
        // Biggest first. `Reverse` flips the order of the comparison.
        parts.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
        let detail = parts
            .iter()
            .filter(|(size, _)| *size > 0)
            .map(|(size, what)| format!("{} {what}", disk::human(*size)))
            .collect::<Vec<_>>()
            .join(", ");
        views.push(StorageView {
            name: name.clone(),
            total: usage.total(),
            size: disk::human(usage.total()),
            detail,
        });
    }
    let total = disk::human(views.iter().map(|view| view.total).sum());
    Ok(StorageOverview {
        profiles: views,
        total,
    })
}

fn shim_path(env: &Env) -> PathBuf {
    env.bin_dir().join("claude")
}

pub fn overview(env: &Env) -> Result<Overview> {
    let loaded = config::load(env)?;
    let config = &loaded.config;

    let mut profiles = Vec::new();
    for name in config.profiles.keys() {
        let kept = retention::read(env, config, name)?;
        let (keeps, keeps_days) = match &kept {
            // Claude Code's default is shown as the number it stands for.
            Retention::Default => (
                retention::days_text(retention::DEFAULT_DAYS),
                Some(retention::DEFAULT_DAYS),
            ),
            Retention::Days(days) => (retention::days_text(*days), Some(*days)),
            Retention::Unknown(_) => (kept.describe(), None),
        };
        profiles.push(ProfileView {
            name: name.clone(),
            account: view::account(env, config, name),
            location: view::location(env, config, name)?,
            is_default: config.default.as_deref() == Some(name),
            rules: loaded
                .rules()
                .filter(|(rule, _)| rule.profile == *name)
                .count(),
            team_rules: loaded
                .rules()
                .filter(|(rule, source)| rule.profile == *name && *source == RuleSource::Team)
                .count(),
            can_purge: purge::folder(env, config, name).is_ok(),
            keeps,
            keeps_days,
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
        can_reuse_login: config::native_profile(config).is_none(),
    })
}

/// Creates a profile, with the same checks as `cprofile add`. With
/// `reuse_login` it takes over the existing `~/.claude` login.
pub fn add_profile(env: &Env, name: &str, reuse_login: bool) -> Result<()> {
    let mut config = config::load(env)?.config;
    // Spaces around a typed name are never meant.
    config::add_profile(env, &mut config, name.trim(), reuse_login, None)?;
    Ok(())
}

/// Removes a profile and the user's rules that point to it, like
/// `cprofile remove`. Its Claude Code folder, and so its login, are kept,
/// unless `purge` asks to delete them too (`cprofile remove --purge`).
pub fn remove_profile(env: &Env, name: &str, purge: bool) -> Result<()> {
    let mut config = config::load(env)?.config;
    if purge {
        purge::purge_profile(env, &mut config, name)?;
    } else {
        config::remove_profile(env, &mut config, name)?;
    }
    // What the app itself wrote for this profile. A profile created later
    // under the same name must not start in the old one's folder. These
    // files may not exist, hence the ignored results.
    let _ = fs::remove_file(folder_note(env, name));
    let _ = fs::remove_file(run_dir(env).join(format!("{name}.command")));
    Ok(())
}

/// Sets how long a profile keeps its conversations, like
/// `cprofile retention <profile> <days>`.
pub fn set_retention(env: &Env, name: &str, days: u64) -> Result<()> {
    let config = config::load(env)?.config;
    if !config.profiles.contains_key(name) {
        bail!("unknown profile '{name}'");
    }
    retention::set(env, &config, name, days)
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

/// Where the app keeps the scripts it writes and the folders it remembers.
fn run_dir(env: &Env) -> PathBuf {
    env.root.join("run")
}

fn folder_note(env: &Env, name: &str) -> PathBuf {
    run_dir(env).join(format!("{name}.folder"))
}

/// The folder Claude Code was last opened in for this profile, if it still
/// exists. The folder picker starts there, so reopening a project is quick.
pub fn last_folder(env: &Env, name: &str) -> Option<PathBuf> {
    let text = fs::read_to_string(folder_note(env, name)).ok()?;
    let folder = PathBuf::from(text.trim_end_matches('\n'));
    folder.is_dir().then_some(folder)
}

/// Writes the small script that starts Claude Code in a profile, from the
/// given project folder, and returns its path. macOS opens a `.command` file
/// in a new terminal window.
///
/// The script goes through the shim with an explicit choice, so a locked rule
/// still has the last word, exactly as when the user types the command.
pub fn write_launcher(env: &Env, name: &str, folder: &Path) -> Result<PathBuf> {
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

    if !folder.is_dir() {
        bail!("{} is not a folder", folder.display());
    }

    let dir = run_dir(env);
    fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    // Remembered for next time. Not being able to write it is not worth
    // failing the launch, hence the ignored result.
    let _ = fs::write(folder_note(env, name), folder.display().to_string());
    let file = dir.join(format!("{name}.command"));
    let script = format!(
        "#!/bin/sh\n\
         # Written by Claude Profiles. Starts Claude Code with the '{name}' profile.\n\
         cd {folder} || exit 1\n\
         CLAUDE_PROFILE={name} exec {shim}\n",
        folder = shell_quote(&folder.display().to_string()),
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
        assert_eq!(work.team_rules, 0);

        assert_eq!(overview.rules.len(), 1);
        assert_eq!(
            overview.rules[0].git_remote.as_deref(),
            Some("github.com/acme/*")
        );
        assert_eq!(overview.rules[0].path, None);
        assert!(overview.rules[0].locked);
    }

    #[test]
    fn add_profile_applies_the_same_checks_as_the_cli() {
        let (_temp, env) = world();
        // "perso" already reuses ~/.claude, so the offer is gone.
        assert!(!overview(&env).unwrap().can_reuse_login);

        add_profile(&env, "  client ", false).unwrap();
        let config = config::load(&env).unwrap().config;
        assert!(config.profiles.contains_key("client"));
        assert!(env.profiles_dir().join("client").is_dir());
        // The first profile stays the default.
        assert_eq!(config.default.as_deref(), Some("perso"));

        let error =
            |name: &str, reuse: bool| add_profile(&env, name, reuse).unwrap_err().to_string();
        assert!(error("client", false).contains("already exists"));
        assert!(error("my profile", false).contains("invalid profile name"));
        assert!(error("", false).contains("invalid profile name"));
        assert!(error("other", true).contains("already the native one"));
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
    fn storage_measures_each_profile_and_names_the_biggest_part_first() {
        let (_temp, env) = world();
        let sessions = env.profiles_dir().join("work/projects/app");
        fs::create_dir_all(&sessions).unwrap();
        fs::write(sessions.join("session.jsonl"), vec![b'x'; 300_000]).unwrap();
        fs::write(env.profiles_dir().join("work/settings.json"), "{}").unwrap();

        let overview = storage(&env).unwrap();
        // Only "work" holds anything, so the sum is its size.
        assert_eq!(overview.total, overview.profiles[1].size);
        let storage = overview.profiles;
        assert_eq!(storage.len(), 2);
        // "perso" is native and its ~/.claude does not exist in this world.
        assert_eq!(storage[0].total, 0);
        assert_eq!(storage[0].detail, "");
        let work = &storage[1];
        assert!(work.total >= 300_000, "{work:?}");
        assert!(work.size.ends_with("KB"), "{work:?}");
        assert!(work.detail.contains("conversations, "), "{work:?}");
        assert!(work.detail.ends_with(" other"), "{work:?}");
    }

    #[test]
    fn retention_is_shown_and_set_per_profile() {
        let (_temp, env) = world();
        let before = overview(&env).unwrap();
        assert_eq!(before.profiles[1].keeps, "30 days");
        assert_eq!(before.profiles[1].keeps_days, Some(30));

        set_retention(&env, "work", 120).unwrap();
        let after = overview(&env).unwrap();
        assert_eq!(after.profiles[1].keeps, "120 days");
        assert_eq!(after.profiles[1].keeps_days, Some(120));
        // The other profile keeps its own value.
        assert_eq!(after.profiles[0].keeps_days, Some(30));

        assert!(set_retention(&env, "work", 0).is_err());
        assert!(set_retention(&env, "nobody", 365).is_err());

        // A settings file that cannot be read: shown, and not changeable.
        fs::write(env.profiles_dir().join("work/settings.json"), "{ nope").unwrap();
        let broken = overview(&env).unwrap();
        assert_eq!(broken.profiles[1].keeps_days, None);
        assert!(broken.profiles[1].keeps.starts_with("unknown"));
    }

    #[test]
    fn remove_profile_takes_its_rules_but_keeps_its_folder() {
        let (_temp, env) = world();
        let work_dir = env.profiles_dir().join("work");
        fs::create_dir_all(&work_dir).unwrap();
        let mut config = config::load(&env).unwrap().config;
        config.rules.push(Rule {
            path: Some("~/perso".to_string()),
            git_remote: None,
            profile: "perso".to_string(),
            locked: false,
        });
        config::save(&env, &config).unwrap();
        fs::create_dir_all(run_dir(&env)).unwrap();
        fs::write(folder_note(&env, "work"), "/somewhere").unwrap();

        // The native profile's folder is ~/.claude: no purge offered.
        let before = overview(&env).unwrap();
        assert!(!before.profiles[0].can_purge);
        assert!(before.profiles[1].can_purge);
        assert!(remove_profile(&env, "perso", true).is_err());

        remove_profile(&env, "work", false).unwrap();
        let config = config::load(&env).unwrap().config;
        assert!(!config.profiles.contains_key("work"));
        // Only the rule of the other profile is left.
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].profile, "perso");
        assert_eq!(config.default.as_deref(), Some("perso"));
        // The account's folder stays, the app's own note goes.
        assert!(work_dir.is_dir());
        assert!(!folder_note(&env, "work").exists());

        // Removing the default leaves no default, as in the CLI.
        remove_profile(&env, "perso", false).unwrap();
        let config = config::load(&env).unwrap().config;
        assert_eq!(config.default, None);
        assert!(config.rules.is_empty());

        assert!(remove_profile(&env, "nobody", false).is_err());

        // With `purge`, the folder goes too.
        add_profile(&env, "client", false).unwrap();
        let client_dir = env.profiles_dir().join("client");
        fs::write(client_dir.join("history.jsonl"), "{}").unwrap();
        remove_profile(&env, "client", true).unwrap();
        assert!(!client_dir.exists());
    }

    #[test]
    fn launcher_needs_the_shim_and_starts_in_the_chosen_folder() {
        let (_temp, env) = world();
        // A project folder with a space and a quote in its name.
        let project = env.home.join("it's my app");
        fs::create_dir_all(&project).unwrap();

        // No shim yet: the error says what to do.
        let error = write_launcher(&env, "work", &project)
            .unwrap_err()
            .to_string();
        assert!(error.contains("cprofile setup"), "{error}");
        assert_eq!(last_folder(&env, "work"), None);

        fs::create_dir_all(env.bin_dir()).unwrap();
        fs::write(shim_path(&env), "").unwrap();
        let file = write_launcher(&env, "work", &project).unwrap();
        let script = fs::read_to_string(&file).unwrap();
        assert!(script.starts_with("#!/bin/sh\n"), "{script}");
        assert!(script.contains("/it'\\''s my app' || exit 1\n"), "{script}");
        assert!(script.contains("CLAUDE_PROFILE=work exec '"), "{script}");
        assert!(script.contains("/.claude-profiles/bin/claude'"), "{script}");

        // The folder is remembered for this profile only, while it exists.
        assert_eq!(last_folder(&env, "work"), Some(project.clone()));
        assert_eq!(last_folder(&env, "perso"), None);
        fs::remove_dir(&project).unwrap();
        assert_eq!(last_folder(&env, "work"), None);

        // A missing folder, an unknown profile, and a name that could break
        // out of the script are all refused.
        assert!(write_launcher(&env, "work", &project).is_err());
        assert!(write_launcher(&env, "nobody", &env.home).is_err());
        assert!(write_launcher(&env, "work; rm -rf ~", &env.home).is_err());
    }

    #[test]
    fn shell_quote_survives_a_single_quote() {
        assert_eq!(shell_quote("/a b/claude"), "'/a b/claude'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
