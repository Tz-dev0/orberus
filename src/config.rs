use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths::Env;

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Profile used when nothing else decides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Path of the real `claude` binary. Found in PATH when not set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_bin: Option<String>,
    /// Hides the one-line notice the shim prints in a terminal.
    #[serde(default, skip_serializing_if = "is_false")]
    pub quiet: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, Profile>,
    #[serde(default, rename = "rule", skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Uses Claude Code's own default folder (`~/.claude`) by leaving
    /// CLAUDE_CONFIG_DIR unset. This keeps an existing login untouched.
    #[serde(default, skip_serializing_if = "is_false")]
    pub native: bool,
    /// Custom config folder. Defaults to `<root>/profiles/<name>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Folder the rule applies to, including everything below it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Git remote pattern, `*` matches anything: `github.com/acme/*`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_remote: Option<String>,
    pub profile: String,
    /// A locked rule wins over everything and refuses another profile.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

impl Rule {
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(path) = &self.path {
            parts.push(format!("path = \"{path}\""));
        }
        if let Some(remote) = &self.git_remote {
            parts.push(format!("git_remote = \"{remote}\""));
        }
        parts.join(", ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSource {
    Team,
    User,
}

impl RuleSource {
    pub fn label(self) -> &'static str {
        match self {
            RuleSource::Team => "team.toml",
            RuleSource::User => "config.toml",
        }
    }
}

/// Team file: only rules are read from it.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct TeamFile {
    #[serde(default, rename = "rule")]
    rules: Vec<Rule>,
}

#[derive(Debug, Default, Clone)]
pub struct Loaded {
    pub config: Config,
    pub team_rules: Vec<Rule>,
    /// False when the user has no config file yet.
    pub exists: bool,
}

impl Loaded {
    /// All rules in the order they are checked: team first, then user.
    pub fn rules(&self) -> impl Iterator<Item = (&Rule, RuleSource)> {
        self.team_rules
            .iter()
            .map(|rule| (rule, RuleSource::Team))
            .chain(
                self.config
                    .rules
                    .iter()
                    .map(|rule| (rule, RuleSource::User)),
            )
    }

    /// True when there is nothing to decide with, so `claude` should run
    /// exactly as if this tool was not installed.
    pub fn is_empty(&self) -> bool {
        !self.exists && self.team_rules.is_empty()
    }
}

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    let value = toml::from_str(&text).with_context(|| format!("invalid {}", path.display()))?;
    Ok(Some(value))
}

pub fn load(env: &Env) -> Result<Loaded> {
    let config: Option<Config> = read_toml(&env.config_file())?;
    let team: Option<TeamFile> = read_toml(&env.team_file())?;
    Ok(Loaded {
        exists: config.is_some(),
        config: config.unwrap_or_default(),
        team_rules: team.unwrap_or_default().rules,
    })
}

pub fn save(env: &Env, config: &Config) -> Result<()> {
    fs::create_dir_all(&env.root)
        .with_context(|| format!("cannot create {}", env.root.display()))?;
    let text = toml::to_string_pretty(config).context("cannot serialize the config")?;
    let target = env.config_file();
    let temp = target.with_extension("toml.tmp");
    fs::write(&temp, text).with_context(|| format!("cannot write {}", temp.display()))?;
    fs::rename(&temp, &target).with_context(|| format!("cannot write {}", target.display()))?;
    Ok(())
}

pub fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok {
        bail!("invalid profile name '{name}': use letters, digits, '-' and '_'");
    }
    Ok(())
}

/// The folder given to Claude Code as CLAUDE_CONFIG_DIR for this profile.
/// `None` means the profile is native: the variable must stay unset.
pub fn profile_dir(env: &Env, config: &Config, name: &str) -> Result<Option<PathBuf>> {
    let Some(profile) = config.profiles.get(name) else {
        bail!("unknown profile '{name}' (create it with: cprofile add {name})");
    };
    if profile.native {
        return Ok(None);
    }
    Ok(Some(match &profile.dir {
        Some(dir) => env.expand_tilde(dir),
        None => env.profiles_dir().join(name),
    }))
}
