//! How long each profile keeps its conversations.
//!
//! Claude Code deletes session transcripts older than `cleanupPeriodDays`,
//! 30 days unless set, without a message. Each profile has its own
//! `settings.json`, so each has its own value, and a new profile starts on
//! the 30-day default.
//!
//! From the official settings reference (read on 2026-11-26): the default is
//! 30, the minimum is 1, and 0 fails validation. Any whole number of days
//! from 1 up is accepted here. Only the user-level file of the profile is
//! read here; a project or managed settings file can still override it.

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};

use crate::config::{self, Config};
use crate::paths::Env;

const KEY: &str = "cleanupPeriodDays";
/// What Claude Code uses when the setting is absent.
pub const DEFAULT_DAYS: u64 = 30;
/// What a hint suggests to someone who wants to keep more than the default.
pub const SUGGESTED_DAYS: u64 = 365;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retention {
    /// Not set: Claude Code's own default applies.
    Default,
    Days(u64),
    /// The file or the value cannot be understood; the text says why.
    Unknown(String),
}

impl Retention {
    /// For a table or a sentence: "30 days (Claude Code default)".
    pub fn describe(&self) -> String {
        match self {
            Retention::Default => format!("{} (Claude Code default)", days_text(DEFAULT_DAYS)),
            Retention::Days(days) => days_text(*days),
            Retention::Unknown(why) => format!("unknown ({why})"),
        }
    }
}

/// "30 days", "1 year", "10 years": whole years read better than 3650 days.
pub fn days_text(days: u64) -> String {
    match days {
        1 => "1 day".to_string(),
        365 => "1 year".to_string(),
        days if days % 365 == 0 => format!("{} years", days / 365),
        days => format!("{days} days"),
    }
}

/// The profile's own settings file: `settings.json` in its config folder.
fn settings_file(env: &Env, config: &Config, name: &str) -> Result<PathBuf> {
    let dir = match config::profile_dir(env, config, name)? {
        Some(dir) => dir,
        None => crate::tool::CLAUDE.native_dir(env),
    };
    Ok(dir.join("settings.json"))
}

/// Reads the file as a JSON object. `None` when the file does not exist.
fn read_settings(file: &PathBuf) -> Result<Option<Map<String, Value>>> {
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        // No file yet is normal for a profile that was never started.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("cannot read {}", file.display())),
    };
    match serde_json::from_str(&text) {
        Ok(Value::Object(settings)) => Ok(Some(settings)),
        Ok(_) => bail!("{} is not a JSON object", file.display()),
        Err(error) => bail!("{} is not valid JSON: {error}", file.display()),
    }
}

pub fn read(env: &Env, config: &Config, name: &str) -> Result<Retention> {
    let file = settings_file(env, config, name)?;
    let settings = match read_settings(&file) {
        Ok(Some(settings)) => settings,
        Ok(None) => return Ok(Retention::Default),
        // A broken file is shown, not fatal: the other profiles still list.
        Err(error) => return Ok(Retention::Unknown(format!("{error:#}"))),
    };
    Ok(match settings.get(KEY) {
        None => Retention::Default,
        // `as_u64` is `None` for a negative number, a fraction or a text.
        Some(value) => match value.as_u64() {
            Some(days) if days >= 1 => Retention::Days(days),
            _ => Retention::Unknown(format!("{KEY} is {value}, not a number of days")),
        },
    })
}

/// Sets how long the profile keeps its conversations, leaving every other
/// setting as it is.
pub fn set(env: &Env, config: &Config, name: &str, days: u64) -> Result<()> {
    if days < 1 {
        bail!("Claude Code refuses 0: the shortest it accepts is 1 day");
    }
    let file = settings_file(env, config, name)?;
    // A file we cannot understand is never overwritten: `?` stops here.
    let mut settings = read_settings(&file)?.unwrap_or_default();
    settings.insert(KEY.to_string(), Value::from(days));

    let mut text = serde_json::to_string_pretty(&Value::Object(settings))?;
    text.push('\n');
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    // Written next to the file, then moved over it: a crash in the middle
    // leaves the old settings whole instead of half a file.
    let draft = file.with_extension("json.orberus-tmp");
    fs::write(&draft, text).with_context(|| format!("cannot write {}", draft.display()))?;
    fs::rename(&draft, &file).with_context(|| format!("cannot write {}", file.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;

    fn world() -> (tempfile::TempDir, Env, Config) {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let env = Env {
            root: home.join(".orberus"),
            home,
        };
        let mut config = Config::default();
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
        (temp, env, config)
    }

    #[test]
    fn days_read_like_a_person_would_say_them() {
        assert_eq!(days_text(1), "1 day");
        assert_eq!(days_text(30), "30 days");
        assert_eq!(days_text(365), "1 year");
        assert_eq!(days_text(3650), "10 years");
        assert_eq!(days_text(400), "400 days");
    }

    #[test]
    fn a_profile_without_the_setting_is_on_the_default() {
        let (_temp, env, config) = world();
        // No settings file at all, then one that does not mention it.
        assert_eq!(read(&env, &config, "work").unwrap(), Retention::Default);
        let dir = env.profiles_dir().join("work");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings.json"), r#"{"theme": "dark"}"#).unwrap();
        assert_eq!(read(&env, &config, "work").unwrap(), Retention::Default);
    }

    #[test]
    fn set_keeps_the_other_settings_and_their_order() {
        let (_temp, env, config) = world();
        let file = env.home.join(".claude/settings.json");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, r#"{"theme": "dark", "model": "opus"}"#).unwrap();

        // The native profile's settings are in ~/.claude.
        set(&env, &config, "perso", 3650).unwrap();
        assert_eq!(read(&env, &config, "perso").unwrap(), Retention::Days(3650));
        let text = fs::read_to_string(&file).unwrap();
        let (theme, model) = (text.find("theme").unwrap(), text.find("model").unwrap());
        assert!(
            theme < model,
            "the order of the keys must not change: {text}"
        );
        assert!(text.contains(r#""model": "opus""#), "{text}");

        // A profile never started has no file yet: it is created.
        set(&env, &config, "work", 365).unwrap();
        assert_eq!(read(&env, &config, "work").unwrap(), Retention::Days(365));
        // The other profile is untouched.
        assert_eq!(read(&env, &config, "perso").unwrap(), Retention::Days(3650));
    }

    #[test]
    fn zero_and_broken_files_are_refused() {
        let (_temp, env, config) = world();
        let error = set(&env, &config, "work", 0).unwrap_err().to_string();
        assert!(error.contains("1 day"), "{error}");

        let dir = env.profiles_dir().join("work");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        fs::write(&file, "{ not json").unwrap();
        assert!(matches!(
            read(&env, &config, "work").unwrap(),
            Retention::Unknown(_)
        ));
        // Never overwrite a file we cannot read.
        assert!(set(&env, &config, "work", 365).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "{ not json");

        // A value Claude Code itself would refuse is reported as such.
        fs::write(&file, r#"{"cleanupPeriodDays": 0}"#).unwrap();
        assert!(matches!(
            read(&env, &config, "work").unwrap(),
            Retention::Unknown(_)
        ));
    }
}
