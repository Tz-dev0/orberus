//! Is there a newer release? Asked of GitHub, at most once a day.
//!
//! There is no server behind Orberus. GitHub answers the address of "the
//! latest release" of a repository with a redirection to that release, whose
//! address ends with its tag (`…/releases/tag/v0.1.6`). Reading where the
//! redirection leads is enough: no page is downloaded, and nothing about
//! the user is sent beyond the request itself.
//!
//! Only the menu bar app asks. The shim never does: it starts no process
//! and makes no network request.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::paths::Env;

/// The address GitHub redirects to the latest release.
const LATEST: &str = "https://github.com/Tz-dev0/orberus/releases/latest";

/// Where the update is explained: the page to open from the notice.
pub const HOW_TO: &str = "https://github.com/Tz-dev0/orberus/blob/main/docs/installation.md#update";

/// How long an answer is trusted before GitHub is asked again.
const DAY: u64 = 86_400;

/// The version a release address ends with: `0.1.6` for
/// `…/releases/tag/v0.1.6`. Anything that is not digits and dots is
/// refused: this text comes from the network and ends up on screen.
fn version_of(address: &str) -> Option<String> {
    let (_, tag) = address.trim().rsplit_once("/releases/tag/v")?;
    numbers(tag)?;
    Some(tag.to_string())
}

/// `0.1.6` as `[0, 1, 6]`. `None` for anything else, a pre-release such
/// as `0.2.0-beta` included: it is never offered as an update.
fn numbers(version: &str) -> Option<Vec<u64>> {
    if version.is_empty() || version.len() > 20 {
        return None;
    }
    // `collect` into an `Option` stops at the first part that is not a
    // number and gives `None` for the whole.
    version.split('.').map(|part| part.parse().ok()).collect()
}

/// True when `latest` is a later version than `current`. Vectors compare
/// number by number, so 0.1.10 comes after 0.1.9.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (numbers(latest), numbers(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

/// Where the last answer is kept, among the app's own notes: when GitHub
/// was asked, then the version it gave.
fn note(env: &Env) -> PathBuf {
    env.root.join("run").join(".update")
}

fn read_note(env: &Env) -> Option<(u64, Option<String>)> {
    let text = fs::read_to_string(note(env)).ok()?;
    let mut lines = text.lines();
    let asked = lines.next()?.parse().ok()?;
    // A version we would not show is forgotten.
    let version = lines
        .next()
        .filter(|version| numbers(version).is_some())
        .map(str::to_string);
    Some((asked, version))
}

fn write_note(env: &Env, asked: u64, version: Option<&str>) {
    let note = note(env);
    if let Some(dir) = note.parent() {
        let _ = fs::create_dir_all(dir);
    }
    // A note that cannot be written only means GitHub is asked again.
    let _ = fs::write(note, format!("{asked}\n{}\n", version.unwrap_or("")));
}

/// Asks GitHub for the latest release. `None` when there is no answer:
/// no network, no `curl`, or an address we do not understand.
fn ask() -> Option<String> {
    // The system's own `curl` on macOS, so that no other program found
    // first in the `PATH` is started in its place.
    let curl = if cfg!(target_os = "macos") {
        "/usr/bin/curl"
    } else {
        "curl"
    };
    let output = Command::new(curl)
        .args(["--silent", "--max-time", "5", "--output"])
        .arg(if cfg!(windows) { "NUL" } else { "/dev/null" })
        // Prints where the redirection leads, without following it.
        .args(["--write-out", "%{redirect_url}", LATEST])
        .output()
        .ok()?;
    version_of(&String::from_utf8_lossy(&output.stdout))
}

/// The version to update to, when a release later than `current` exists.
/// GitHub is asked at most once a day; in between, and without network,
/// the last answer is used.
pub fn available(env: &Env, current: &str, now: u64) -> Option<String> {
    available_with(env, current, now, ask)
}

/// The same, with the question to GitHub given by the caller: the tests
/// answer in its place.
fn available_with(
    env: &Env,
    current: &str,
    now: u64,
    ask: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let noted = read_note(env);
    // A date in the future (the clock was changed) does not count.
    let fresh = noted
        .as_ref()
        .is_some_and(|(asked, _)| *asked <= now && now - asked < DAY);
    let known = noted.and_then(|(_, version)| version);
    let latest = if fresh {
        known
    } else {
        // Without an answer, the last one stays, and the next question
        // waits a day all the same: no retry at each opening of the window.
        let latest = ask().or(known);
        write_note(env, now, latest.as_deref());
        latest
    };
    latest.filter(|latest| is_newer(latest, current))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> (tempfile::TempDir, Env) {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let env = Env {
            root: home.join(".orberus"),
            home,
        };
        (temp, env)
    }

    #[test]
    fn the_version_is_read_at_the_end_of_the_release_address() {
        let address = "https://github.com/Tz-dev0/orberus/releases/tag/v0.1.6";
        assert_eq!(version_of(address).as_deref(), Some("0.1.6"));
        assert_eq!(
            version_of(&format!("{address}\n")).as_deref(),
            Some("0.1.6")
        );
        // No release yet, a pre-release, or text that is not a version.
        assert_eq!(version_of(""), None);
        assert_eq!(
            version_of("https://github.com/Tz-dev0/orberus/releases"),
            None
        );
        assert_eq!(
            version_of("https://example.com/releases/tag/v0.2.0-beta"),
            None
        );
        assert_eq!(
            version_of("https://example.com/releases/tag/v<b>1</b>"),
            None
        );
    }

    /// The real question, over the network: not run with the others.
    /// `cargo test -p orberus-core -- --ignored asks_github`
    #[test]
    #[ignore = "needs the network"]
    fn asks_github_for_the_latest_release() {
        let latest = ask().expect("no answer from GitHub");
        assert!(numbers(&latest).is_some(), "{latest}");
        // An old version is offered the update, the latest one is not.
        assert!(is_newer(&latest, "0.0.1"));
        assert!(!is_newer(&latest, &latest));
    }

    #[test]
    fn versions_compare_number_by_number() {
        assert!(is_newer("0.1.7", "0.1.6"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("0.2", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.6", "0.1.6"));
        assert!(!is_newer("0.1.5", "0.1.6"));
        assert!(!is_newer("nonsense", "0.1.6"));
    }

    #[test]
    fn github_is_asked_once_a_day_and_the_answer_is_kept() {
        let (_temp, env) = world();
        let answer = |version: &'static str| move || Some(version.to_string());
        let never = || -> Option<String> { panic!("asked again within the day") };

        // First opening: asked, and a later release is offered.
        let found = available_with(&env, "0.1.6", 1_000, answer("0.1.7"));
        assert_eq!(found.as_deref(), Some("0.1.7"));
        // Within the day the note answers, also after the update is done.
        assert_eq!(
            available_with(&env, "0.1.6", 1_000 + DAY - 1, never).as_deref(),
            Some("0.1.7")
        );
        assert_eq!(available_with(&env, "0.1.7", 2_000, never), None);

        // A day later it is asked again.
        let found = available_with(&env, "0.1.7", 1_000 + DAY, answer("0.1.8"));
        assert_eq!(found.as_deref(), Some("0.1.8"));
    }

    #[test]
    fn without_an_answer_the_last_one_stays_and_the_next_try_waits() {
        let (_temp, env) = world();
        let silent = || None;
        // Never an answer: nothing to offer, and no retry before a day.
        assert_eq!(available_with(&env, "0.1.6", 1_000, silent), None);
        let never = || -> Option<String> { panic!("asked again within the day") };
        assert_eq!(available_with(&env, "0.1.6", 2_000, never), None);

        // An answer, then the network goes: what was known is still shown.
        let later = 1_000 + DAY;
        available_with(&env, "0.1.6", later, || Some("0.1.7".to_string()));
        let found = available_with(&env, "0.1.6", later + DAY, silent);
        assert_eq!(found.as_deref(), Some("0.1.7"));

        // A note from a clock set in the future does not silence the check.
        write_note(&env, u64::MAX, Some("0.1.7"));
        let found = available_with(&env, "0.1.6", later, || Some("0.1.9".to_string()));
        assert_eq!(found.as_deref(), Some("0.1.9"));
    }
}
