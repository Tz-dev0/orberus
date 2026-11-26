//! Creating and removing the user's rules. Shared by `cprofile rule` and the
//! menu bar app, so both apply the same checks.
//!
//! Team rules live in `team.toml`, which the company provides: nothing here
//! can change them.

use std::path::Path;

use anyhow::{bail, Result};

use crate::config::{self, Config, Rule};
use crate::git;
use crate::paths::Env;

/// One way to say where a rule applies, offered for a given folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// What the user reads: "Every repository of github.com/acme".
    pub label: String,
    pub path: Option<String>,
    pub git_remote: Option<String>,
}

/// Writes a folder the way rules store it: the home folder as `~`, so the
/// rule still reads well and survives a renamed user. `folder` must be
/// absolute.
pub fn portable_path(env: &Env, folder: &Path) -> String {
    // Both sides are made canonical: on macOS `/tmp` is a link to
    // `/private/tmp`, and the two spellings must compare equal.
    let folder = folder
        .canonicalize()
        .unwrap_or_else(|_| folder.to_path_buf());
    let home = env.home.canonicalize().unwrap_or_else(|_| env.home.clone());
    match folder.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => folder.display().to_string(),
    }
}

/// The rules that make sense for `folder`, widest first: every repository
/// of its owner and this repository alone when it has a git remote, then
/// the folder itself.
pub fn choices(env: &Env, folder: &Path) -> Vec<Choice> {
    let mut choices = Vec::new();
    if let Some(remote) = git::remotes(folder).into_iter().next() {
        // `github.com/acme/app` gives the owner pattern `github.com/acme/*`.
        if let Some((owner, _)) = remote.rsplit_once('/') {
            choices.push(Choice {
                label: format!("Every repository of {owner}"),
                path: None,
                git_remote: Some(format!("{owner}/*")),
            });
        }
        choices.push(Choice {
            label: format!("Only the repository {remote}"),
            path: None,
            git_remote: Some(remote),
        });
    }
    let path = portable_path(env, folder);
    choices.push(Choice {
        label: format!("The folder {path} and everything below"),
        path: Some(path),
        git_remote: None,
    });
    choices
}

/// Adds a rule at the end of the user's rules and saves the config.
///
/// Refused: a rule with no condition (it would never match), one for a
/// profile that does not exist (`claude` would then fail where it applies),
/// and an exact copy of a rule already there.
pub fn add(env: &Env, config: &mut Config, rule: Rule) -> Result<()> {
    // An empty text is no condition at all. `filter` keeps the value only
    // when the closure answers true.
    let clean = |text: Option<String>| {
        text.map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
    };
    let rule = Rule {
        path: clean(rule.path),
        git_remote: clean(rule.git_remote),
        ..rule
    };
    if rule.path.is_none() && rule.git_remote.is_none() {
        bail!("a rule needs a folder, a git remote, or both");
    }
    if !config.profiles.contains_key(&rule.profile) {
        bail!("unknown profile '{}'", rule.profile);
    }
    if config.rules.contains(&rule) {
        bail!("this rule already exists");
    }
    config.rules.push(rule);
    config::save(env, config)
}

/// Removes the user's rule at `index` (0 is their first one, team rules
/// are not counted), saves the config and returns the rule.
pub fn remove(env: &Env, config: &mut Config, index: usize) -> Result<Rule> {
    if index >= config.rules.len() {
        bail!("this rule no longer exists");
    }
    let rule = config.rules.remove(index);
    config::save(env, config)?;
    Ok(rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;
    use std::fs;

    fn world() -> (tempfile::TempDir, Env, Config) {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        let env = Env {
            root: home.join(".claude-profiles"),
            home,
        };
        let mut config = Config::default();
        config
            .profiles
            .insert("work".to_string(), Profile::default());
        (temp, env, config)
    }

    fn rule(path: Option<&str>, remote: Option<&str>, profile: &str) -> Rule {
        Rule {
            path: path.map(str::to_string),
            git_remote: remote.map(str::to_string),
            profile: profile.to_string(),
            locked: false,
        }
    }

    #[test]
    fn a_repository_offers_its_owner_itself_and_its_folder() {
        let (_temp, env, _) = world();
        let repo = env.home.join("code/api");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::write(
            repo.join(".git/config"),
            "[remote \"origin\"]\n\turl = git@github.com:acme/api.git\n",
        )
        .unwrap();
        let found = choices(&env, &repo);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].git_remote.as_deref(), Some("github.com/acme/*"));
        assert_eq!(found[1].git_remote.as_deref(), Some("github.com/acme/api"));
        assert_eq!(found[2].path.as_deref(), Some("~/code/api"));

        // Outside any repository, only the folder is left.
        let plain = env.home.join("notes");
        fs::create_dir_all(&plain).unwrap();
        let found = choices(&env, &plain);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path.as_deref(), Some("~/notes"));
    }

    #[test]
    fn the_home_folder_is_written_as_a_tilde() {
        let (_temp, env, _) = world();
        assert_eq!(portable_path(&env, &env.home), "~");
        assert_eq!(portable_path(&env, &env.home.join("a/b")), "~/a/b");
        assert_eq!(portable_path(&env, Path::new("/opt/x")), "/opt/x");
    }

    #[test]
    fn add_refuses_what_could_never_work() {
        let (_temp, env, mut config) = world();
        let no_condition = add(&env, &mut config, rule(None, Some("  "), "work"));
        assert!(no_condition.unwrap_err().to_string().contains("needs"));
        let no_profile = add(&env, &mut config, rule(Some("~/a"), None, "nope"));
        assert!(no_profile.unwrap_err().to_string().contains("unknown"));

        add(&env, &mut config, rule(Some(" ~/a "), None, "work")).unwrap();
        // Stored trimmed, and saved.
        assert_eq!(config.rules[0].path.as_deref(), Some("~/a"));
        assert_eq!(config::load(&env).unwrap().config.rules.len(), 1);
        let twice = add(&env, &mut config, rule(Some("~/a"), None, "work"));
        assert!(twice.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn remove_takes_one_rule_by_its_place() {
        let (_temp, env, mut config) = world();
        add(&env, &mut config, rule(Some("~/a"), None, "work")).unwrap();
        add(&env, &mut config, rule(Some("~/b"), None, "work")).unwrap();
        let gone = remove(&env, &mut config, 0).unwrap();
        assert_eq!(gone.path.as_deref(), Some("~/a"));
        assert_eq!(config::load(&env).unwrap().config.rules.len(), 1);
        assert!(remove(&env, &mut config, 5).is_err());
    }
}
