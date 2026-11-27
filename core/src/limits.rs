//! The 5-hour and weekly limits of each account, as Claude Code reports them.
//!
//! Nothing on disk holds these numbers, and asking Anthropic's service with
//! a subscription's token is not something a third-party tool may do. The
//! one documented way is the status line: Claude Code runs a command of the
//! user's choice and hands it a JSON document that contains, for
//! subscribers, `rate_limits.five_hour` and `rate_limits.seven_day`, each
//! with `used_percentage` (0 to 100) and `resets_at` (Unix seconds).
//! Read in the official status line page on 2026-11-27.
//!
//! So a profile whose limits are wanted gets `orberus statusline` as its
//! status line. Each time Claude Code runs it, the numbers are written to
//! `~/.orberus/limits/<profile>.json`. They are therefore the last numbers
//! Claude Code showed for that account, with their age, never a live
//! reading: a profile that was not used for a while has old numbers.
//!
//! A status line the user already had is kept aside and still shown.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::{self, Config};
use crate::paths::Env;
use crate::settings;
use crate::tool;

const KEY: &str = "statusLine";
/// The argument that makes `orberus` act as a status line.
pub const COMMAND: &str = "statusline";

/// One limit: how much of it is used, and when it starts over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    /// From 0 to 100.
    pub used: f64,
    /// Unix seconds.
    pub resets_at: u64,
}

/// The last limits Claude Code reported for one profile.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Limits {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
    /// When they were reported, in Unix seconds.
    pub seen_at: u64,
}

/// A window as it stands now.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub used: f64,
    /// Seconds until it starts over. `None` when that moment has passed
    /// since the numbers were reported: the window started over, so `used`
    /// is 0 until Claude Code says more.
    pub resets_in: Option<u64>,
}

impl Window {
    pub fn reading(&self, now: u64) -> Reading {
        if self.resets_at <= now {
            return Reading {
                used: 0.0,
                resets_in: None,
            };
        }
        Reading {
            used: self.used.clamp(0.0, 100.0),
            resets_in: Some(self.resets_at - now),
        }
    }
}

fn folder(env: &Env) -> PathBuf {
    env.root.join("limits")
}

fn limits_file(env: &Env, profile: &str) -> PathBuf {
    folder(env).join(format!("{profile}.json"))
}

/// Where the status line the user had before ours is kept.
fn previous_file(env: &Env, profile: &str) -> PathBuf {
    folder(env).join(format!("{profile}.statusline.json"))
}

/// One window out of the status line's input. Anything unexpected is `None`:
/// the document is Claude Code's, and it may change.
fn window(limits: &Value, key: &str) -> Option<Window> {
    let window = limits.get(key)?;
    let used = window.get("used_percentage")?.as_f64()?;
    // `as_f64` also reads a whole number; a date before 1970 makes no sense.
    let resets_at = window.get("resets_at")?.as_f64()?;
    if resets_at < 0.0 {
        return None;
    }
    Some(Window {
        used,
        resets_at: resets_at as u64,
    })
}

/// Notes the limits found in a status line input, for one profile.
///
/// Returns `false`, and writes nothing, when the input holds no limit: that
/// is the case before the first answer of a session, and for an account
/// without a subscription. A window missing from the input keeps the value
/// noted before; [`Window::reading`] knows when it has started over.
pub fn record(env: &Env, profile: &str, input: &str, now: u64) -> Result<bool> {
    let Ok(document) = serde_json::from_str::<Value>(input) else {
        return Ok(false);
    };
    let Some(reported) = document.get("rate_limits") else {
        return Ok(false);
    };
    let five_hour = window(reported, "five_hour");
    let seven_day = window(reported, "seven_day");
    if five_hour.is_none() && seven_day.is_none() {
        return Ok(false);
    }
    let before = read(env, profile).unwrap_or_default();
    let limits = Limits {
        // `or` keeps the earlier value when this one is missing.
        five_hour: five_hour.or(before.five_hour),
        seven_day: seven_day.or(before.seven_day),
        seen_at: now,
    };
    let file = limits_file(env, profile);
    fs::create_dir_all(folder(env))
        .with_context(|| format!("cannot create {}", folder(env).display()))?;
    // Written beside, then moved: a reader never sees half a file.
    let draft = file.with_extension("json.tmp");
    fs::write(&draft, serde_json::to_string(&limits)?)
        .with_context(|| format!("cannot write {}", draft.display()))?;
    fs::rename(&draft, &file).with_context(|| format!("cannot write {}", file.display()))?;
    Ok(true)
}

/// The last limits noted for a profile, if any.
pub fn read(env: &Env, profile: &str) -> Option<Limits> {
    let text = fs::read_to_string(limits_file(env, profile)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Which profile a running status line belongs to.
///
/// `session` is the profile the shim started Claude Code with
/// (`ORBERUS_SESSION`), `config_dir` the value of `CLAUDE_CONFIG_DIR`.
/// Claude Code started without the shim has no session: the config folder
/// then says which profile it is, and no folder at all is the native one.
pub fn profile_of(
    env: &Env,
    config: &Config,
    session: Option<&str>,
    config_dir: Option<&Path>,
) -> Option<String> {
    if let Some(name) = session {
        if config.profiles.contains_key(name) {
            return Some(name.to_string());
        }
    }
    config.profiles.keys().find_map(|name| {
        let dir = config::profile_dir(env, config, name).ok()?;
        // Both `None`: the native profile, and no folder was given.
        (dir.as_deref() == config_dir).then(|| name.clone())
    })
}

/// The line shown under the prompt: "work · 5h 3% · week 11%".
pub fn line(profile: &str, limits: Option<&Limits>, now: u64) -> String {
    let mut parts = vec![profile.to_string()];
    if let Some(limits) = limits {
        if let Some(window) = &limits.five_hour {
            parts.push(format!("5h {}", percent(window.reading(now).used)));
        }
        if let Some(window) = &limits.seven_day {
            parts.push(format!("week {}", percent(window.reading(now).used)));
        }
    }
    parts.join(" · ")
}

/// "3%": limits are read as whole percents, like Claude shows them.
pub fn percent(used: f64) -> String {
    format!("{}%", used.round() as u64)
}

/// A delay in its biggest unit: "4d", "4h", "12m". For "resets in" and
/// "updated … ago".
pub fn delay_text(seconds: u64) -> String {
    match seconds {
        0..=59 => "under 1m".to_string(),
        60..=3_599 => format!("{}m", seconds / 60),
        3_600..=86_399 => format!("{}h", seconds / 3_600),
        _ => format!("{}d", seconds / 86_400),
    }
}

/// Whether a profile's limits are being noted.
#[derive(Debug, Clone, PartialEq)]
pub enum Capture {
    /// Its status line is ours.
    On,
    /// It has no status line, or the user's own.
    Off,
    /// Its settings cannot be read; the text says why.
    Unknown(String),
}

/// Our status line is recognised by how its command ends, so it is still
/// ours after the binary moved.
fn is_ours(status_line: &Value) -> bool {
    status_line
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.trim_end().ends_with(&format!(" {COMMAND}")))
}

pub fn capture(env: &Env, config: &Config, name: &str) -> Result<Capture> {
    let file = settings::file(env, config, name)?;
    Ok(match settings::read(&file) {
        Ok(Some(found)) if found.get(KEY).is_some_and(is_ours) => Capture::On,
        Ok(_) => Capture::Off,
        Err(error) => Capture::Unknown(format!("{error:#}")),
    })
}

/// The `orberus` program to name in the status line: what the `claude`
/// link made by `orberus setup` points to.
pub fn program(env: &Env) -> Result<PathBuf> {
    let shim = tool::CLAUDE.shim_path(env);
    // `canonicalize` follows the link to the real file.
    let program = fs::canonicalize(&shim)
        .with_context(|| format!("{} is missing: run orberus setup first", shim.display()))?;
    if program.file_stem() == shim.file_stem() {
        // Where links are not used (Windows), the shim is a copy, and a
        // copy named `claude` only ever acts as the shim.
        bail!("limits are not available on this system yet");
    }
    Ok(program)
}

/// Makes `orberus statusline` the status line of a profile. A status line
/// the user already had is kept aside: it is still what Claude Code shows,
/// and [`disable`] puts it back.
pub fn enable(env: &Env, config: &Config, name: &str, program: &Path) -> Result<()> {
    let file = settings::file(env, config, name)?;
    // A file we cannot understand is never overwritten: `?` stops here.
    let mut found = settings::read(&file)?.unwrap_or_default();
    // Single quotes, so a space in the path does not cut the command.
    let quoted = program.display().to_string().replace('\'', r"'\''");
    let command = format!("'{quoted}' {COMMAND}");

    let mut ours = Map::new();
    match found.get(KEY) {
        Some(current) if is_ours(current) => {
            // Already on: only the path is refreshed.
            if let Value::Object(current) = current {
                ours = current.clone();
            }
        }
        Some(current) => {
            fs::create_dir_all(folder(env))
                .with_context(|| format!("cannot create {}", folder(env).display()))?;
            let aside = previous_file(env, name);
            fs::write(&aside, serde_json::to_string_pretty(current)?)
                .with_context(|| format!("cannot write {}", aside.display()))?;
            // Its other choices (padding, refresh) stay as they were.
            if let Value::Object(current) = current {
                ours = current.clone();
            }
        }
        None => {}
    }
    ours.insert("type".to_string(), Value::from("command"));
    ours.insert("command".to_string(), Value::from(command));
    found.insert(KEY.to_string(), Value::Object(ours));
    settings::write(&file, found)
}

/// Removes our status line from a profile, and puts back the one the user
/// had. The numbers already noted stay.
pub fn disable(env: &Env, config: &Config, name: &str) -> Result<()> {
    let file = settings::file(env, config, name)?;
    let Some(mut found) = settings::read(&file)? else {
        return Ok(());
    };
    if !found.get(KEY).is_some_and(is_ours) {
        return Ok(());
    }
    let aside = previous_file(env, name);
    match previous(env, name) {
        Some(before) => found.insert(KEY.to_string(), before),
        // `shift_remove` keeps the order of the other settings.
        None => found.shift_remove(KEY),
    };
    settings::write(&file, found)?;
    // Only once the settings are written: a failure above loses nothing.
    let _ = fs::remove_file(aside);
    Ok(())
}

/// For a profile that is being removed: its folder stays, so our status
/// line is taken out of it, and the numbers noted for it are forgotten.
/// Nothing here is worth stopping a removal: failures are ignored.
pub fn forget(env: &Env, config: &Config, name: &str) {
    let _ = disable(env, config, name);
    let _ = fs::remove_file(limits_file(env, name));
    let _ = fs::remove_file(previous_file(env, name));
}

/// Takes our status line out of every profile that has it, and returns
/// their names. For uninstalling: a status line left behind would name a
/// program that is gone. A profile whose settings cannot be read is skipped.
pub fn disable_all(env: &Env, config: &Config) -> Vec<String> {
    let mut done = Vec::new();
    for name in config.profiles.keys() {
        if matches!(capture(env, config, name), Ok(Capture::On))
            && disable(env, config, name).is_ok()
        {
            done.push(name.clone());
        }
    }
    done
}

/// The status line the user had before ours, if any.
fn previous(env: &Env, name: &str) -> Option<Value> {
    let text = fs::read_to_string(previous_file(env, name)).ok()?;
    serde_json::from_str(&text).ok()
}

/// The command of the status line the user had before ours: `orberus
/// statusline` runs it and shows its output instead of its own line.
pub fn previous_command(env: &Env, name: &str) -> Option<String> {
    let before = previous(env, name)?;
    Some(before.get("command")?.as_str()?.to_string())
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

    const INPUT: &str = r#"{"model":{"id":"x"},"rate_limits":{
        "five_hour":{"used_percentage":23.5,"resets_at":2000},
        "seven_day":{"used_percentage":41.2,"resets_at":9000}}}"#;

    #[test]
    fn record_notes_both_windows() {
        let (_temp, env, _config) = world();
        assert!(record(&env, "work", INPUT, 1000).unwrap());
        let limits = read(&env, "work").unwrap();
        assert_eq!(limits.seen_at, 1000);
        assert_eq!(
            limits.five_hour,
            Some(Window {
                used: 23.5,
                resets_at: 2000
            })
        );
        assert_eq!(limits.seven_day.unwrap().used, 41.2);
        assert_eq!(read(&env, "perso"), None);
    }

    #[test]
    fn record_without_limits_keeps_what_was_noted() {
        let (_temp, env, _config) = world();
        record(&env, "work", INPUT, 1000).unwrap();
        // Before the first answer of a session, or anything unexpected.
        assert!(!record(&env, "work", r#"{"model":{"id":"x"}}"#, 1500).unwrap());
        assert!(!record(&env, "work", r#"{"rate_limits":{}}"#, 1500).unwrap());
        assert!(!record(&env, "work", "not json", 1500).unwrap());
        assert_eq!(read(&env, "work").unwrap().seen_at, 1000);
    }

    #[test]
    fn a_window_missing_from_the_input_keeps_its_last_value() {
        let (_temp, env, _config) = world();
        record(&env, "work", INPUT, 1000).unwrap();
        let only_week = r#"{"rate_limits":{"seven_day":{"used_percentage":50,"resets_at":9000}}}"#;
        assert!(record(&env, "work", only_week, 1200).unwrap());
        let limits = read(&env, "work").unwrap();
        assert_eq!(limits.five_hour.unwrap().used, 23.5);
        assert_eq!(limits.seven_day.unwrap().used, 50.0);
        assert_eq!(limits.seen_at, 1200);
    }

    #[test]
    fn a_window_past_its_reset_reads_as_empty() {
        let window = Window {
            used: 80.0,
            resets_at: 2000,
        };
        assert_eq!(
            window.reading(1400),
            Reading {
                used: 80.0,
                resets_in: Some(600)
            }
        );
        assert_eq!(
            window.reading(2000),
            Reading {
                used: 0.0,
                resets_in: None
            }
        );
    }

    #[test]
    fn profile_of_a_status_line() {
        let (_temp, env, config) = world();
        let work = env.profiles_dir().join("work");
        // Started by the shim: the session names the profile.
        assert_eq!(
            profile_of(&env, &config, Some("work"), None).as_deref(),
            Some("work")
        );
        // Started without it: the config folder does.
        assert_eq!(
            profile_of(&env, &config, None, Some(&work)).as_deref(),
            Some("work")
        );
        assert_eq!(
            profile_of(&env, &config, None, None).as_deref(),
            Some("perso")
        );
        assert_eq!(
            profile_of(&env, &config, Some("gone"), Some(Path::new("/elsewhere"))),
            None
        );
    }

    #[test]
    fn line_names_the_profile_and_its_limits() {
        let limits = Limits {
            five_hour: Some(Window {
                used: 2.6,
                resets_at: 5000,
            }),
            seven_day: Some(Window {
                used: 11.0,
                resets_at: 9000,
            }),
            seen_at: 1000,
        };
        assert_eq!(line("work", Some(&limits), 1000), "work · 5h 3% · week 11%");
        assert_eq!(line("work", Some(&limits), 6000), "work · 5h 0% · week 11%");
        assert_eq!(line("work", None, 1000), "work");
    }

    #[test]
    fn delay_text_uses_the_biggest_unit() {
        assert_eq!(delay_text(30), "under 1m");
        assert_eq!(delay_text(12 * 60 + 5), "12m");
        assert_eq!(delay_text(4 * 3600 + 59), "4h");
        assert_eq!(delay_text(4 * 86_400 + 3600), "4d");
    }

    #[test]
    fn enable_then_disable_leaves_the_settings_as_they_were() {
        let (_temp, env, config) = world();
        let file = settings::file(&env, &config, "work").unwrap();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        let before = "{\n  \"model\": \"opus\",\n  \"theme\": \"dark\"\n}\n";
        fs::write(&file, before).unwrap();
        assert_eq!(capture(&env, &config, "work").unwrap(), Capture::Off);

        enable(&env, &config, "work", Path::new("/opt/my tools/orberus")).unwrap();
        assert_eq!(capture(&env, &config, "work").unwrap(), Capture::On);
        let text = fs::read_to_string(&file).unwrap();
        assert!(
            text.contains("'/opt/my tools/orberus' statusline"),
            "{text}"
        );
        assert_eq!(previous_command(&env, "work"), None);

        disable(&env, &config, "work").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), before);
    }

    #[test]
    fn a_status_line_the_user_had_is_kept_and_put_back() {
        let (_temp, env, config) = world();
        let file = settings::file(&env, &config, "perso").unwrap();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        let before = "{\n  \"statusLine\": {\n    \"type\": \"command\",\n    \"command\": \"~/.claude/mine.sh\",\n    \"padding\": 2\n  }\n}\n";
        fs::write(&file, before).unwrap();

        enable(&env, &config, "perso", Path::new("/bin/orberus")).unwrap();
        assert_eq!(capture(&env, &config, "perso").unwrap(), Capture::On);
        assert_eq!(
            previous_command(&env, "perso").as_deref(),
            Some("~/.claude/mine.sh")
        );
        // Turning it on twice does not lose the user's own.
        enable(&env, &config, "perso", Path::new("/bin/orberus")).unwrap();
        assert_eq!(
            previous_command(&env, "perso").as_deref(),
            Some("~/.claude/mine.sh")
        );
        assert!(fs::read_to_string(&file)
            .unwrap()
            .contains("\"padding\": 2"));

        disable(&env, &config, "perso").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), before);
        assert_eq!(previous_command(&env, "perso"), None);
    }

    #[test]
    fn removing_a_profile_takes_our_status_line_out_of_its_folder() {
        let (_temp, env, mut config) = world();
        enable(&env, &config, "work", Path::new("/bin/orberus")).unwrap();
        record(&env, "work", INPUT, 1000).unwrap();
        let file = settings::file(&env, &config, "work").unwrap();

        config::remove_profile(&env, &mut config, "work").unwrap();
        assert!(!fs::read_to_string(&file).unwrap().contains("statusLine"));
        assert_eq!(read(&env, "work"), None);
    }

    #[test]
    fn broken_settings_are_never_overwritten() {
        let (_temp, env, config) = world();
        let file = settings::file(&env, &config, "work").unwrap();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "{ not json").unwrap();
        assert!(enable(&env, &config, "work", Path::new("/bin/orberus")).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "{ not json");
        assert!(matches!(
            capture(&env, &config, "work").unwrap(),
            Capture::Unknown(_)
        ));
    }
}
