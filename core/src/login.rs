//! Opening the menu bar app when the user logs in.
//!
//! macOS starts, at login, every program described by a file in
//! `~/Library/LaunchAgents`. Ours is one small file that names the app's
//! program: writing it turns the option on, deleting it turns it off.
//! It is on by default: the app writes the file when it starts, unless the
//! user turned the option off.
//! Nothing is asked of `launchctl`, which would start a second copy of the
//! app on the spot; the file is read at the next login.
//!
//! The newer way (a login item registered by the app itself) asks for an
//! app signed by a known developer, which this one is not.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::paths::Env;
use crate::uninstall::APP_IDENTIFIER;

/// The file that tells macOS to open the app at login.
pub fn file(env: &Env) -> PathBuf {
    env.home
        .join("Library/LaunchAgents")
        .join(format!("{APP_IDENTIFIER}.plist"))
}

/// A path as text inside the file. The three characters XML reserves are
/// written the way XML wants them.
fn escaped(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn content(program: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{APP_IDENTIFIER}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
        escaped(program)
    )
}

/// True when the app opens at login, and it is this copy of it: a file
/// left by a copy that was since moved names a program that is gone.
pub fn enabled(env: &Env, program: &Path) -> bool {
    fs::read_to_string(file(env)).is_ok_and(|text| text == content(program))
}

/// Makes macOS open `program`, the app's own executable, at each login.
/// Only for an installed app: a program outside an `.app` is one built to
/// be tried, and would not be there for long.
pub fn enable(env: &Env, program: &Path) -> Result<()> {
    let in_app = program
        .ancestors()
        .any(|folder| folder.extension().is_some_and(|ext| ext == "app"));
    if !in_app || !program.is_file() {
        bail!(
            "{} is not the program of an installed app",
            program.display()
        );
    }
    let file = file(env);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    fs::write(&file, content(program)).with_context(|| format!("cannot write {}", file.display()))
}

/// Stops opening the app at login. Returns false when it was not set.
pub fn disable(env: &Env) -> Result<bool> {
    let file = file(env);
    match fs::remove_file(&file) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("cannot delete {}", file.display())),
    }
}

/// Where "the user turned it off" is remembered, among the app's own
/// notes. Without it, the next start of the app would turn it on again.
fn declined_note(env: &Env) -> PathBuf {
    env.root.join("run").join(".login-off")
}

/// The user's choice, from the switch in the app: on or off, and
/// remembered either way.
pub fn set(env: &Env, program: &Path, on: bool) -> Result<()> {
    let note = declined_note(env);
    if on {
        enable(env, program)?;
        // The note may not be there: nothing to say about that.
        let _ = fs::remove_file(&note);
    } else {
        disable(env)?;
        if let Some(dir) = note.parent() {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        fs::write(&note, "").with_context(|| format!("cannot write {}", note.display()))?;
    }
    Ok(())
}

/// Called when the app starts: opening at login is on unless the user
/// turned it off. Returns true when it was just turned on. Also puts the
/// file right after the app was moved to another folder.
pub fn by_default(env: &Env, program: &Path) -> Result<bool> {
    if declined_note(env).exists() || enabled(env, program) {
        return Ok(false);
    }
    enable(env, program)?;
    Ok(true)
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
    fn the_app_is_opened_at_login_until_told_otherwise() {
        let (_temp, env) = world();
        let program = env
            .home
            .join("Applications/Orberus.app/Contents/MacOS/orberus-app");
        fs::create_dir_all(program.parent().unwrap()).unwrap();
        fs::write(&program, "").unwrap();

        assert!(!enabled(&env, &program));
        assert!(!disable(&env).unwrap());

        enable(&env, &program).unwrap();
        assert!(enabled(&env, &program));
        let text = fs::read_to_string(file(&env)).unwrap();
        assert!(text.contains(&format!("<string>{}</string>", program.display())));
        assert!(text.contains("<key>RunAtLoad</key>"));

        // Another copy of the app does not take this file for its own.
        assert!(!enabled(
            &env,
            &env.home.join("Other.app/Contents/MacOS/orberus-app")
        ));

        assert!(disable(&env).unwrap());
        assert!(!file(&env).exists());
    }

    #[test]
    fn it_is_on_by_default_until_the_user_turns_it_off() {
        let (_temp, env) = world();
        let program = env
            .home
            .join("Applications/Orberus.app/Contents/MacOS/orberus-app");
        fs::create_dir_all(program.parent().unwrap()).unwrap();
        fs::write(&program, "").unwrap();

        // First start of the app: on, without being asked.
        assert!(by_default(&env, &program).unwrap());
        assert!(enabled(&env, &program));
        // Nothing more to do at the next start.
        assert!(!by_default(&env, &program).unwrap());

        // Turned off by the user: it stays off, start after start.
        set(&env, &program, false).unwrap();
        assert!(!enabled(&env, &program));
        assert!(!by_default(&env, &program).unwrap());
        assert!(!file(&env).exists());

        // Turned on again, then the app is moved: the file follows it.
        set(&env, &program, true).unwrap();
        let moved = env
            .home
            .join("Tools/Orberus.app/Contents/MacOS/orberus-app");
        fs::create_dir_all(moved.parent().unwrap()).unwrap();
        fs::write(&moved, "").unwrap();
        assert!(!enabled(&env, &moved));
        assert!(by_default(&env, &moved).unwrap());
        assert!(enabled(&env, &moved));
    }

    #[test]
    fn only_an_installed_app_can_be_opened_at_login() {
        let (_temp, env) = world();
        // A program built to be tried, outside any `.app`.
        let loose = env.home.join("target/debug/orberus-app");
        fs::create_dir_all(loose.parent().unwrap()).unwrap();
        fs::write(&loose, "").unwrap();
        assert!(enable(&env, &loose).is_err());
        // An app that is not there.
        assert!(enable(&env, &env.home.join("Gone.app/Contents/MacOS/orberus-app")).is_err());
        assert!(!file(&env).exists());
    }

    #[test]
    fn a_path_is_written_the_way_xml_wants_it() {
        let text = content(Path::new(
            "/Apps/R&D <x>/Orberus.app/Contents/MacOS/orberus-app",
        ));
        assert!(text.contains(
            "<string>/Apps/R&amp;D &lt;x&gt;/Orberus.app/Contents/MacOS/orberus-app</string>"
        ));
    }
}
