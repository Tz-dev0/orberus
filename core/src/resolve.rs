//! Decides which profile applies to a folder.
//!
//! Priority, highest first:
//! 1. a locked rule (it also refuses an explicit choice of another profile)
//! 2. an explicit choice (`--profile` or the ORBERUS_PROFILE variable)
//! 3. a `.orberus-profile` file in the folder or one of its parents
//! 4. the first matching rule, team rules before user rules
//! 5. the default profile

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::config::{Loaded, Rule, RuleSource};
use crate::git;
use crate::paths::Env;

pub const REPO_FILE: &str = ".orberus-profile";

#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    Locked { rule: String, source: RuleSource },
    Explicit,
    RepoFile(PathBuf),
    Rule { rule: String, source: RuleSource },
    Default,
}

impl Reason {
    pub fn describe(&self) -> String {
        match self {
            Reason::Locked { rule, source } => {
                format!("locked rule {rule} in {}", source.label())
            }
            Reason::Explicit => "explicit choice".to_string(),
            Reason::RepoFile(path) => format!("file {}", path.display()),
            Reason::Rule { rule, source } => format!("rule {rule} in {}", source.label()),
            Reason::Default => "default profile".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub profile: String,
    pub reason: Reason,
}

/// Returns `None` when nothing decides: no rule matches and there is no
/// default. The caller then runs `claude` untouched.
pub fn resolve(
    env: &Env,
    loaded: &Loaded,
    cwd: &Path,
    explicit: Option<&str>,
) -> Result<Option<Resolution>> {
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let needs_git = loaded.rules().any(|(rule, _)| rule.git_remote.is_some());
    let remotes = if needs_git {
        git::remotes(&cwd)
    } else {
        Vec::new()
    };
    let applies = |rule: &Rule| rule_matches(env, rule, &cwd, &remotes);

    let locked = loaded
        .rules()
        .find(|(rule, _)| rule.locked && applies(rule));

    let resolution = if let Some((rule, source)) = locked {
        if let Some(wanted) = explicit {
            if wanted != rule.profile {
                bail!(
                    "refused: this folder is locked to profile '{}' by rule {} in {}, \
                     so profile '{wanted}' cannot be used here",
                    rule.profile,
                    rule.describe(),
                    source.label()
                );
            }
        }
        Some(Resolution {
            profile: rule.profile.clone(),
            reason: Reason::Locked {
                rule: rule.describe(),
                source,
            },
        })
    } else if let Some(name) = explicit {
        Some(Resolution {
            profile: name.to_string(),
            reason: Reason::Explicit,
        })
    } else if let Some((file, name)) = find_repo_file(&cwd) {
        Some(Resolution {
            profile: name,
            reason: Reason::RepoFile(file),
        })
    } else if let Some((rule, source)) = loaded.rules().find(|(rule, _)| applies(rule)) {
        Some(Resolution {
            profile: rule.profile.clone(),
            reason: Reason::Rule {
                rule: rule.describe(),
                source,
            },
        })
    } else {
        loaded.config.default.clone().map(|profile| Resolution {
            profile,
            reason: Reason::Default,
        })
    };

    if let Some(found) = &resolution {
        if !loaded.config.profiles.contains_key(&found.profile) {
            bail!(
                "profile '{}' ({}) does not exist; create it with: orberus add {}",
                found.profile,
                found.reason.describe(),
                found.profile
            );
        }
    }
    Ok(resolution)
}

fn rule_matches(env: &Env, rule: &Rule, cwd: &Path, remotes: &[String]) -> bool {
    // A rule with no condition would match everywhere: treat it as invalid.
    if rule.path.is_none() && rule.git_remote.is_none() {
        return false;
    }
    if let Some(pattern) = &rule.path {
        if !path_matches(env, pattern, cwd) {
            return false;
        }
    }
    if let Some(pattern) = &rule.git_remote {
        let pattern = git::normalize_remote(pattern);
        if !remotes
            .iter()
            .any(|remote| git::glob_match(&pattern, remote))
        {
            return false;
        }
    }
    true
}

/// A path rule covers its folder and everything below it. A trailing `/**`
/// or `/*` is accepted and means the same thing.
fn path_matches(env: &Env, pattern: &str, cwd: &Path) -> bool {
    let trimmed = pattern
        .trim_end_matches("/**")
        .trim_end_matches("/*")
        .trim_end_matches('/');
    let base = env.expand_tilde(trimmed);
    let base = base.canonicalize().unwrap_or(base);
    cwd.starts_with(&base)
}

fn find_repo_file(cwd: &Path) -> Option<(PathBuf, String)> {
    for dir in cwd.ancestors() {
        let file = dir.join(REPO_FILE);
        if let Ok(text) = fs::read_to_string(&file) {
            let name = text.lines().map(str::trim).find(|line| !line.is_empty())?;
            return Some((file, name.to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Profile};

    struct Fixture {
        _temp: tempfile::TempDir,
        env: Env,
        loaded: Loaded,
    }

    fn rule(path: Option<&str>, remote: Option<&str>, profile: &str, locked: bool) -> Rule {
        Rule {
            path: path.map(str::to_string),
            git_remote: remote.map(str::to_string),
            profile: profile.to_string(),
            locked,
        }
    }

    fn fixture(rules: Vec<Rule>, team_rules: Vec<Rule>) -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        let env = Env {
            root: home.join(".orberus"),
            home,
        };
        let mut config = Config {
            default: Some("perso".into()),
            rules,
            ..Config::default()
        };
        config.profiles.insert(
            "perso".into(),
            Profile {
                native: true,
                dir: None,
            },
        );
        config.profiles.insert("work".into(), Profile::default());
        Fixture {
            _temp: temp,
            env,
            loaded: Loaded {
                config,
                team_rules,
                exists: true,
            },
        }
    }

    fn make_repo(fx: &Fixture, relative: &str, remote: &str) -> PathBuf {
        let repo = fx.env.home.join(relative);
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("lib/src")).unwrap();
        fs::write(
            repo.join(".git/config"),
            format!("[remote \"origin\"]\n\turl = {remote}\n"),
        )
        .unwrap();
        repo
    }

    fn profile_of(fx: &Fixture, cwd: &Path, explicit: Option<&str>) -> Resolution {
        resolve(&fx.env, &fx.loaded, cwd, explicit)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn falls_back_to_default() {
        let fx = fixture(vec![], vec![]);
        let found = profile_of(&fx, &fx.env.home, None);
        assert_eq!(found.profile, "perso");
        assert_eq!(found.reason, Reason::Default);
    }

    #[test]
    fn path_rule_covers_subfolders_only() {
        let fx = fixture(vec![rule(Some("~/work/**"), None, "work", false)], vec![]);
        let inside = fx.env.home.join("work/app/lib");
        let sibling = fx.env.home.join("workshop");
        fs::create_dir_all(&inside).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        assert_eq!(profile_of(&fx, &inside, None).profile, "work");
        assert_eq!(profile_of(&fx, &sibling, None).profile, "perso");
    }

    #[test]
    fn git_remote_rule_matches_from_a_subfolder() {
        let fx = fixture(
            vec![rule(None, Some("github.com/acme/*"), "work", false)],
            vec![],
        );
        let repo = make_repo(&fx, "anywhere/app", "git@github.com:Acme/app.git");
        let other = make_repo(&fx, "anywhere/mine", "https://github.com/me/mine.git");
        assert_eq!(profile_of(&fx, &repo.join("lib/src"), None).profile, "work");
        assert_eq!(profile_of(&fx, &other, None).profile, "perso");
    }

    #[test]
    fn explicit_choice_beats_a_normal_rule() {
        let fx = fixture(vec![rule(Some("~/work"), None, "work", false)], vec![]);
        let inside = fx.env.home.join("work");
        fs::create_dir_all(&inside).unwrap();
        let found = profile_of(&fx, &inside, Some("perso"));
        assert_eq!(found.profile, "perso");
        assert_eq!(found.reason, Reason::Explicit);
    }

    #[test]
    fn locked_rule_refuses_another_profile() {
        let fx = fixture(
            vec![],
            vec![rule(None, Some("github.com/acme/*"), "work", true)],
        );
        let repo = make_repo(&fx, "code/app", "https://github.com/acme/app.git");
        let error = resolve(&fx.env, &fx.loaded, &repo, Some("perso")).unwrap_err();
        assert!(
            error.to_string().contains("locked to profile 'work'"),
            "{error}"
        );
        // The same profile asked explicitly is fine.
        assert_eq!(profile_of(&fx, &repo, Some("work")).profile, "work");
    }

    #[test]
    fn locked_rule_beats_the_repo_file() {
        let fx = fixture(
            vec![],
            vec![rule(None, Some("github.com/acme/*"), "work", true)],
        );
        let repo = make_repo(&fx, "code/app", "https://github.com/acme/app.git");
        fs::write(repo.join(REPO_FILE), "perso\n").unwrap();
        let found = profile_of(&fx, &repo, None);
        assert_eq!(found.profile, "work");
        assert!(matches!(
            found.reason,
            Reason::Locked {
                source: RuleSource::Team,
                ..
            }
        ));
    }

    #[test]
    fn repo_file_beats_a_normal_rule() {
        let fx = fixture(vec![rule(Some("~/code"), None, "work", false)], vec![]);
        let project = fx.env.home.join("code/side-project/src");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            fx.env.home.join("code/side-project").join(REPO_FILE),
            "perso\n",
        )
        .unwrap();
        let found = profile_of(&fx, &project, None);
        assert_eq!(found.profile, "perso");
        assert!(matches!(found.reason, Reason::RepoFile(_)));
    }

    #[test]
    fn team_rules_are_checked_before_user_rules() {
        let fx = fixture(
            vec![rule(Some("~/code"), None, "perso", false)],
            vec![rule(Some("~/code"), None, "work", false)],
        );
        let dir = fx.env.home.join("code");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(profile_of(&fx, &dir, None).profile, "work");
    }

    #[test]
    fn unknown_profile_is_an_error() {
        let fx = fixture(vec![], vec![]);
        let error = resolve(&fx.env, &fx.loaded, &fx.env.home, Some("ghost")).unwrap_err();
        assert!(error.to_string().contains("does not exist"), "{error}");
    }

    #[test]
    fn nothing_decides_without_default() {
        let mut fx = fixture(vec![], vec![]);
        fx.loaded.config.default = None;
        assert_eq!(
            resolve(&fx.env, &fx.loaded, &fx.env.home, None).unwrap(),
            None
        );
    }
}
