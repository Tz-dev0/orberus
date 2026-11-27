//! What Claude Desktop notes about the plan's usage.
//!
//! While it runs, the Claude Desktop app writes the 5-hour and weekly usage
//! of the account it is logged in to, every few minutes, in
//! `~/Library/Application Support/Claude/plan-usage-history.json`. It is what
//! its menu bar window draws. That usage is the account's: work done in
//! Claude Desktop, on claude.ai and in Claude Code all count in it.
//!
//! Seen on 2026-11-27 (Claude Desktop 7.37095.0), 185 samples over 28 days,
//! one every 5 minutes while the app is in use:
//!
//! ```json
//! {"version": 2, "samples": [
//!   {"t": 1795808191691, "org": "<organization id>", "u": {"fh": 16, "sd": 13}}
//! ]}
//! ```
//!
//! `t` is in milliseconds, `fh` and `sd` are percents of the 5-hour and the
//! 7-day limit, `org` is the same id as `oauthAccount.organizationUuid` in
//! Claude Code's state file. There is no reset time.
//!
//! This file is Claude Desktop's own, not a documented interface. We only
//! read it, and anything unexpected means "no sample", never an error.
//! Nothing here talks to a server or reads a login.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::paths::Env;

/// One reading of an account's usage by Claude Desktop.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// When it was taken, in Unix seconds.
    pub at: u64,
    /// Percent of the 5-hour limit, from 0 to 100.
    pub five_hour: Option<f64>,
    /// Percent of the weekly limit.
    pub seven_day: Option<f64>,
}

/// Where Claude Desktop keeps its samples.
pub fn history_file(env: &Env) -> PathBuf {
    env.home
        .join("Library")
        .join("Application Support")
        .join("Claude")
        .join("plan-usage-history.json")
}

/// The most recent sample of one organization in the file's text.
fn latest_in(text: &str, organization: &str) -> Option<Sample> {
    let document: Value = serde_json::from_str(text).ok()?;
    let samples = document.get("samples")?.as_array()?;
    samples
        .iter()
        .filter_map(|sample| {
            if sample.get("org")?.as_str()? != organization {
                return None;
            }
            let millis = sample.get("t")?.as_f64()?;
            if millis < 0.0 {
                return None;
            }
            let usage = sample.get("u")?;
            let five_hour = usage.get("fh").and_then(Value::as_f64);
            let seven_day = usage.get("sd").and_then(Value::as_f64);
            if five_hour.is_none() && seven_day.is_none() {
                return None;
            }
            Some(Sample {
                at: (millis / 1000.0) as u64,
                five_hour,
                seven_day,
            })
        })
        // The file is in order of time today; this does not rely on it.
        .max_by_key(|sample| sample.at)
}

/// The last usage Claude Desktop noted for an organization, if any.
pub fn latest(env: &Env, organization: &str) -> Option<Sample> {
    let text = fs::read_to_string(history_file(env)).ok()?;
    latest_in(&text, organization)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HISTORY: &str = r#"{"version":2,"samples":[
        {"t":1000000,"org":"acme","u":{"fh":12,"sd":39}},
        {"t":3000000,"org":"acme","u":{"fh":16,"sd":13}},
        {"t":2000000,"org":"acme","u":{"fh":50,"sd":50}},
        {"t":9000000,"org":"other","u":{"fh":99,"sd":99}}
    ]}"#;

    #[test]
    fn latest_sample_of_the_organization() {
        assert_eq!(
            latest_in(HISTORY, "acme"),
            Some(Sample {
                at: 3000,
                five_hour: Some(16.0),
                seven_day: Some(13.0)
            })
        );
        assert_eq!(latest_in(HISTORY, "other").unwrap().at, 9000);
        assert_eq!(latest_in(HISTORY, "nobody"), None);
    }

    #[test]
    fn anything_unexpected_is_no_sample() {
        assert_eq!(latest_in("not json", "acme"), None);
        assert_eq!(latest_in(r#"{"samples":"soon"}"#, "acme"), None);
        // A sample without numbers, and one of another shape, are skipped.
        let odd = r#"{"samples":[
            {"t":5000000,"org":"acme","u":{}},
            {"when":"now"},
            {"t":4000000,"org":"acme","u":{"sd":7}}
        ]}"#;
        assert_eq!(
            latest_in(odd, "acme"),
            Some(Sample {
                at: 4000,
                five_hour: None,
                seven_day: Some(7.0)
            })
        );
    }

    #[test]
    fn latest_reads_the_file_in_the_home_folder() {
        let temp = tempfile::tempdir().unwrap();
        let env = Env {
            home: temp.path().to_path_buf(),
            root: temp.path().join(".orberus"),
        };
        assert_eq!(latest(&env, "acme"), None);
        let file = history_file(&env);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, HISTORY).unwrap();
        assert_eq!(latest(&env, "acme").unwrap().five_hour, Some(16.0));
    }
}
