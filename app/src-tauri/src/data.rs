//! What the popover shows and what its buttons do, without any Tauri type.
//!
//! Everything takes an `Env`, like the core, so it can be tested against a
//! temporary folder instead of the real `~/.claude-profiles`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use claude_profiles::config::{self, RuleSource};
use claude_profiles::disk;
use claude_profiles::paths::Env;
use claude_profiles::purge;
use claude_profiles::retention::{self, Retention};
use claude_profiles::rules;
use claude_profiles::sessions;
use claude_profiles::usage::{self, Verdict};
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
    /// Its place among the user's own rules, to remove it. `None` for a
    /// team rule, which cannot be removed from here.
    pub index: Option<usize>,
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

/// One conversation of a profile, for the list under its storage line.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationView {
    pub id: String,
    /// Its title, or the start of its id when it has none.
    pub title: String,
    /// Where it worked and when it was last used: "~/app, 2 days ago".
    pub detail: String,
    /// "815 MB".
    pub size: String,
    /// Changed in the last hour: it may still be open, so it cannot go.
    pub in_use: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversations {
    /// The biggest ones, biggest first.
    pub items: Vec<ConversationView>,
    /// How many smaller ones are not listed.
    pub more: usize,
}

/// How many conversations the window lists for a profile.
const LISTED: usize = 8;

/// The biggest conversations of a profile, like `cprofile clean`.
pub fn conversations(env: &Env, name: &str) -> Result<Conversations> {
    let config = config::load(env)?.config;
    let found = sessions::profile_sessions(env, &config, name)?;
    let now = SystemTime::now();
    let items = found
        .iter()
        .take(LISTED)
        .map(|session| {
            let used = sessions::ago(session.last_used, now);
            ConversationView {
                id: session.id.clone(),
                title: match &session.title {
                    Some(title) => title.clone(),
                    None => format!("Untitled ({})", &session.id[..8]),
                },
                detail: match &session.folder {
                    Some(folder) => format!("{}, {used}", view::short_path(env, Path::new(folder))),
                    None => used,
                },
                size: disk::human(session.size),
                in_use: session.in_use(now),
            }
        })
        .collect();
    Ok(Conversations {
        items,
        more: found.len().saturating_sub(LISTED),
    })
}

/// Moves one conversation of a profile to the Trash, by its full id.
pub fn trash_conversation(env: &Env, name: &str, id: &str) -> Result<()> {
    let config = config::load(env)?.config;
    let found = sessions::profile_sessions(env, &config, name)?;
    // The id comes from the page: it must be one this profile really has.
    let Some(session) = found.iter().find(|session| session.id == id) else {
        bail!("this conversation is no longer in profile '{name}'");
    };
    let trash = sessions::trash_dir(env)?;
    sessions::move_to_trash(session, &trash, SystemTime::now())?;
    Ok(())
}

/// One project a profile was used in, for the "Usage" section.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageProject {
    /// "~/code/acme-api".
    pub folder: String,
    /// Its part of the profile's usage: "63%".
    pub share: String,
    /// What today's rules say, in a word: "covered", "default",
    /// "elsewhere", "gone" or "unknown".
    pub state: &'static str,
    /// The same for people, short: "no rule", "would use perso".
    pub note: String,
    /// The full explanation, shown on hover.
    pub why: String,
    /// True when the line deserves a look.
    pub flagged: bool,
}

/// One conversation of a profile, by what it sends with each answer.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTalk {
    pub id: String,
    pub title: String,
    /// "~/code/acme-api, 61% of this profile".
    pub detail: String,
    /// Tokens sent with each answer, on average: "417k".
    pub context: String,
    /// Above the size from which a conversation is flagged.
    pub heavy: bool,
}

/// What one profile consumed over the period.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageView {
    pub name: String,
    /// Its estimated weight, to draw the bars to scale. No unit.
    pub weight: f64,
    /// Its part of all profiles together: "72%". Empty with no activity.
    pub share: String,
    /// The heaviest projects, heaviest first.
    pub projects: Vec<UsageProject>,
    /// How many lighter projects are not listed.
    pub more_projects: usize,
    /// The heaviest conversations, heaviest first.
    pub talks: Vec<UsageTalk>,
    /// Projects that deserve a look, among all of them.
    pub flagged: usize,
    /// "About 88% went to reading context again". Empty with no activity.
    pub reread: String,
}

/// The "Usage" section.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageOverview {
    pub profiles: Vec<UsageView>,
    /// The period, in days.
    pub days: u64,
    /// Projects that deserve a look, all profiles together.
    pub flagged: usize,
}

/// The period the window looks at, like `cprofile usage` by default.
const USAGE_DAYS: u64 = 7;
/// How many projects and conversations the window lists for a profile.
const USAGE_PROJECTS: usize = 5;
const USAGE_TALKS: usize = 3;

/// Which projects and conversations used each profile, like
/// `cprofile usage`. This reads the conversation files of the period, so
/// the page asks for it apart from the profile list.
pub fn usage(env: &Env) -> Result<UsageOverview> {
    let loaded = config::load(env)?;
    // With no rule at all, "no rule names it" is true of every project and
    // is not worth a flag.
    let has_rules = loaded.rules().next().is_some();
    let mut read = Vec::new();
    for name in loaded.config.profiles.keys() {
        read.push((name, usage::profile_usage(env, &loaded, name, USAGE_DAYS)?));
    }
    let all: f64 = read.iter().map(|(_, used)| used.total.weight()).sum();

    let mut views = Vec::new();
    for (name, used) in read {
        let whole = used.total.weight();
        let mut flagged = 0;
        let mut projects = Vec::new();
        for folder in &used.folders {
            let (state, note, why, flag) = match usage::verdict(env, &loaded, name, &folder.folder)
            {
                Verdict::Covered(reason) => (
                    "covered",
                    String::new(),
                    format!("Sent to {name} by {}", usage::rule_text(&reason)),
                    false,
                ),
                Verdict::NoRule => (
                    "default",
                    "no rule".to_string(),
                    format!("No rule names this project: it uses {name} only as the default"),
                    has_rules,
                ),
                Verdict::Elsewhere { profile, reason } => (
                    "elsewhere",
                    format!("would use {profile}"),
                    format!(
                        "Started there today, claude would use {profile} ({}). \
                         It ran with {name} by an explicit choice, or the rules changed since",
                        usage::rule_text(&reason)
                    ),
                    true,
                ),
                Verdict::Gone => (
                    "gone",
                    "folder gone".to_string(),
                    "The folder was deleted, so its rules cannot be checked".to_string(),
                    false,
                ),
                Verdict::Unknown(why) => ("unknown", "not checked".to_string(), why, false),
            };
            if flag {
                flagged += 1;
            }
            // Every project is judged, for the count; only the heaviest
            // are listed.
            if projects.len() < USAGE_PROJECTS {
                projects.push(UsageProject {
                    folder: view::short_path(env, &folder.folder),
                    share: usage::percent(folder.tokens.weight(), whole),
                    state,
                    note,
                    why,
                    flagged: flag,
                });
            }
        }
        let talks = used
            .conversations
            .iter()
            .take(USAGE_TALKS)
            .map(|talk| UsageTalk {
                id: talk.id.clone(),
                title: match sessions::title(&talk.transcript) {
                    Some(title) => title,
                    // `chars`: an id shorter than 8 bytes must not panic.
                    None => format!("Untitled ({})", talk.id.chars().take(8).collect::<String>()),
                },
                detail: format!(
                    "{}, {} of this profile",
                    view::short_path(env, &talk.folder),
                    usage::percent(talk.tokens.weight(), whole)
                ),
                context: usage::tokens_text(talk.context),
                heavy: talk.context >= usage::HEAVY_CONTEXT,
            })
            .collect();
        let active = !used.folders.is_empty();
        views.push(UsageView {
            name: name.clone(),
            weight: whole,
            share: if active {
                usage::percent(whole, all)
            } else {
                String::new()
            },
            more_projects: used.folders.len().saturating_sub(USAGE_PROJECTS),
            projects,
            talks,
            flagged,
            reread: if active {
                format!(
                    "About {:.0}% went to reading context again",
                    used.total.reread_share() * 100.0
                )
            } else {
                String::new()
            },
        });
    }
    let flagged = views.iter().map(|view| view.flagged).sum();
    Ok(UsageOverview {
        profiles: views,
        days: USAGE_DAYS,
        flagged,
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
    // Team rules come first in the list; the user's own are numbered from
    // 0 after them.
    let team = loaded.team_rules.len();
    let rules = loaded
        .rules()
        .enumerate()
        .map(|(place, (rule, source))| RuleView {
            path: rule.path.clone(),
            git_remote: rule.git_remote.clone(),
            profile: rule.profile.clone(),
            locked: rule.locked,
            source: source.label(),
            index: (source == RuleSource::User).then(|| place - team),
        })
        .collect();

    Ok(Overview {
        profiles,
        rules,
        shim_installed: shim_path(env).exists(),
        can_reuse_login: config::native_profile(config).is_none(),
    })
}

/// One way to say where a new rule applies, for the form.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleChoice {
    pub label: String,
    pub path: Option<String>,
    pub git_remote: Option<String>,
}

/// What the "new rule" form offers once a folder is chosen.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDraft {
    /// The chosen folder, for people: "~/code/acme-api".
    pub folder: String,
    /// Where the rule can apply, widest first. Never empty.
    pub choices: Vec<RuleChoice>,
}

/// The rules that make sense for a folder, like the guided
/// `cprofile rule add`.
pub fn rule_draft(env: &Env, folder: &Path) -> RuleDraft {
    RuleDraft {
        folder: view::short_path(env, folder),
        choices: rules::choices(env, folder)
            .into_iter()
            .map(|choice| RuleChoice {
                label: choice.label,
                path: choice.path,
                git_remote: choice.git_remote,
            })
            .collect(),
    }
}

/// Adds a rule, with the same checks as `cprofile rule add`.
pub fn add_rule(
    env: &Env,
    path: Option<String>,
    git_remote: Option<String>,
    profile: &str,
    locked: bool,
) -> Result<()> {
    let mut config = config::load(env)?.config;
    rules::add(
        env,
        &mut config,
        config::Rule {
            path,
            git_remote,
            profile: profile.to_string(),
            locked,
        },
    )
}

/// Removes one of the user's rules. `shown` is the rule as the page showed
/// it: when the config changed meanwhile and another rule sits at that
/// place, nothing is removed.
pub fn remove_rule(env: &Env, index: usize, shown: &config::Rule) -> Result<()> {
    let mut config = config::load(env)?.config;
    if config.rules.get(index) != Some(shown) {
        bail!("the rules changed since they were listed: look at them again");
    }
    rules::remove(env, &mut config, index)?;
    Ok(())
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
    fn rules_are_added_and_removed_but_never_a_team_one() {
        let (_temp, env) = world();
        fs::write(
            env.root.join("team.toml"),
            "[[rule]]\ngit_remote = \"github.com/corp/*\"\nprofile = \"work\"\n",
        )
        .unwrap();
        let folder = env.home.join("notes");
        fs::create_dir_all(&folder).unwrap();
        let draft = rule_draft(&env, &folder);
        assert_eq!(draft.choices.len(), 1);
        let path = draft.choices[0].path.clone();
        assert!(path.as_deref().is_some_and(|path| path.ends_with("notes")));

        add_rule(&env, path.clone(), None, "perso", false).unwrap();
        assert!(add_rule(&env, path.clone(), None, "perso", false).is_err());
        assert!(add_rule(&env, None, None, "perso", false).is_err());
        assert!(add_rule(&env, path.clone(), None, "nope", false).is_err());

        let rules = overview(&env).unwrap().rules;
        // The team rule first, then the one of the world, then the new one.
        assert_eq!(rules.len(), 3);
        assert_eq!((rules[0].source, rules[0].index), ("team.toml", None));
        assert_eq!(rules[1].index, Some(0));
        assert_eq!(rules[2].index, Some(1));

        let shown = config::Rule {
            path,
            git_remote: None,
            profile: "perso".to_string(),
            locked: false,
        };
        // Another rule sits at that place: nothing goes.
        assert!(remove_rule(&env, 0, &shown).is_err());
        assert_eq!(overview(&env).unwrap().rules.len(), 3);
        remove_rule(&env, 1, &shown).unwrap();
        let rules = overview(&env).unwrap().rules;
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[1].git_remote.as_deref(), Some("github.com/acme/*"));
    }

    #[test]
    fn usage_judges_each_project_and_flags_the_heavy_conversation() {
        let (_temp, env) = world();
        // A company repository, covered by the rule, and a side project
        // that only the default profile would take.
        let repo = |name: &str, remote: &str| {
            let folder = env.home.join(name);
            fs::create_dir_all(folder.join(".git")).unwrap();
            fs::write(
                folder.join(".git/config"),
                format!("[remote \"origin\"]\n\turl = {remote}\n"),
            )
            .unwrap();
            folder
        };
        let api = repo("api", "git@github.com:acme/api.git");
        let blog = repo("blog", "git@github.com:me/blog.git");
        // A date inside the period, whenever the test runs: Claude Code
        // writes them as text, and "9999" sorts after any real date.
        let answer = |id: &str, folder: &Path, read: u64| {
            format!(
                "{{\"type\":\"assistant\",\"cwd\":\"{}\",\"timestamp\":\"9999-01-01T00:00:00.000Z\",\"requestId\":\"r-{id}\",\"message\":{{\"id\":\"{id}\",\"usage\":{{\"input_tokens\":0,\"output_tokens\":1000,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":{read}}}}}}}\n",
                folder.display()
            )
        };
        let project = env.profiles_dir().join("work/projects/-p");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("aaaaaaaa-1111.jsonl"),
            [
                "{\"type\":\"custom-title\",\"customTitle\":\"Payment retries\"}\n".to_string(),
                answer("a", &api, 400_000),
                answer("b", &api, 500_000),
            ]
            .concat(),
        )
        .unwrap();
        fs::write(
            project.join("bbbbbbbb-2222.jsonl"),
            answer("c", &blog, 50_000),
        )
        .unwrap();

        let overview = usage(&env).unwrap();
        assert_eq!(overview.days, 7);
        assert_eq!(overview.flagged, 1);
        let perso = &overview.profiles[0];
        assert_eq!((perso.share.as_str(), perso.weight), ("", 0.0));
        assert!(perso.projects.is_empty() && perso.reread.is_empty());

        let work = &overview.profiles[1];
        assert_eq!(work.share, "100%");
        assert_eq!(work.flagged, 1);
        assert_eq!(work.projects.len(), 2);
        assert_eq!(work.projects[0].state, "covered");
        assert!(work.projects[0].why.contains("locked rule"), "{work:?}");
        assert!(!work.projects[0].flagged);
        assert_eq!(work.projects[1].state, "elsewhere");
        assert_eq!(work.projects[1].note, "would use perso");
        assert!(work.projects[1].flagged);

        assert_eq!(work.talks[0].title, "Payment retries");
        assert_eq!(work.talks[0].context, "450k");
        assert!(work.talks[0].heavy);
        assert_eq!(work.talks[1].title, "Untitled (bbbbbbbb)");
        assert!(!work.talks[1].heavy);
        assert!(work.reread.starts_with("About "), "{work:?}");
    }

    // The Trash is only known on macOS for now.
    #[cfg(target_os = "macos")]
    #[test]
    fn conversations_are_listed_and_one_can_go_to_the_trash() {
        use std::time::Duration;

        let (_temp, env) = world();
        let project = env.profiles_dir().join("work/projects/-Users-me-app");
        fs::create_dir_all(project.join("memory")).unwrap();
        let old = "11111111-2222-3333-4444-555555555555";
        let fresh = "99999999-2222-3333-4444-555555555555";
        fs::write(
            project.join(format!("{old}.jsonl")),
            format!(
                "{{\"customTitle\":\"Fix the login\"}}\n{{\"cwd\":\"{}\"}}\n{}",
                env.home.join("app").display(),
                "x".repeat(200_000)
            ),
        )
        .unwrap();
        fs::write(project.join(format!("{fresh}.jsonl")), "{}\n").unwrap();
        fs::File::options()
            .write(true)
            .open(project.join(format!("{old}.jsonl")))
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(3 * 86_400))
            .unwrap();

        let listed = conversations(&env, "work").unwrap();
        assert_eq!(listed.more, 0);
        assert_eq!(listed.items.len(), 2);
        let big = &listed.items[0];
        assert_eq!(big.title, "Fix the login");
        assert_eq!(big.detail, "~/app, 3 days ago");
        assert!(big.size.ends_with("KB") && !big.in_use, "{big:?}");
        let small = &listed.items[1];
        assert_eq!(small.title, "Untitled (99999999)");
        assert!(small.in_use);

        fs::create_dir_all(env.home.join(".Trash")).unwrap();
        // In use, unknown, and from another profile: all refused.
        assert!(trash_conversation(&env, "work", fresh).is_err());
        assert!(trash_conversation(&env, "work", "nope").is_err());
        assert!(trash_conversation(&env, "perso", old).is_err());
        assert!(project.join(format!("{old}.jsonl")).exists());

        trash_conversation(&env, "work", old).unwrap();
        assert!(!project.join(format!("{old}.jsonl")).exists());
        assert!(env
            .home
            .join(format!(".Trash/Claude conversation {old}/{old}.jsonl"))
            .is_file());
        assert!(project.join("memory").is_dir());
        assert_eq!(conversations(&env, "work").unwrap().items.len(), 1);
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
