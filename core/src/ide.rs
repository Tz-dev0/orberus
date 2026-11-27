//! Editors, and the one setting that sends an editor's Claude Code extension
//! through Orberus.
//!
//! Editors start Claude Code in one of two ways.
//!
//! Some run the `claude` command, in a terminal of their own: the JetBrains
//! IDEs do (their plugin has no copy of Claude Code), and so does anyone
//! typing `claude` in an editor's terminal. The shim is first in `PATH`, so
//! nothing more is needed for them.
//!
//! The editors of the VS Code family (VS Code, Cursor and the others built
//! on it) install an extension that carries its own copy of Claude Code and
//! starts that copy directly: `PATH` is not looked at, and the shim never
//! runs. The extension has a setting for this, `claudeCode.claudeProcessWrapper`:
//! a program to start in place of its copy, which gets the copy's path as
//! its first argument. Orberus' wrapper (the same program under a second
//! name, see [`crate::tool::for_wrapper`]) is made to be that program. This
//! module finds those editors and writes or removes the setting in their
//! `settings.json`.
//!
//! The extension declares the setting with the scope "machine": the editor
//! reads it from the user's own settings only, never from a project, and a
//! window opened on another machine (SSH, a container) uses that machine's
//! settings, so it never looks for our wrapper where it does not exist.
//!
//! Checked against the extension itself (2.1.325) and Claude Code's
//! documentation of it. Which folders an editor keeps its settings in is
//! what is seen on a Mac with VS Code and Cursor; the paths for Linux and
//! Windows are the ones VS Code documents and were never tried.
//!
//! Whether the extension is installed in one editor is read in two steps,
//! because the folder of an editor's settings does not say where its
//! extensions are. Each editor's app carries a `product.json` that names
//! both: `nameShort` is its settings folder (`Code`, `Cursor`) and
//! `dataFolderName` the folder, under the home folder, that holds its
//! extensions (`.vscode`, `.cursor`). An editor whose app is not found in
//! the Applications folders is simply not told apart: Orberus then only
//! knows whether the extension is installed somewhere.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::jsonc;
use crate::paths::Env;
use crate::tool::Tool;
use crate::uninstall::app_folders;

/// One settings file of an editor of the VS Code family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    /// The name people know it by: `VS Code`, `Cursor`. An editor profile
    /// that has its own settings is listed apart, with its folder's name.
    pub name: String,
    pub settings: PathBuf,
    /// Where the editor keeps its extensions, when its app was found.
    pub extensions: Option<PathBuf>,
}

/// What an editor's settings say about the wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// The extension starts its own copy of the tool: Orberus is skipped.
    Off,
    /// The extension starts our wrapper.
    On,
    /// The setting names another program. It is not ours to replace.
    Other(String),
    /// The settings file cannot be read or understood; why.
    Unreadable(String),
}

/// The folder where apps keep their settings, under the home folder.
fn settings_base(env: &Env) -> PathBuf {
    if cfg!(target_os = "macos") {
        env.home.join("Library/Application Support")
    } else if cfg!(windows) {
        env.home.join("AppData/Roaming")
    } else {
        env.home.join(".config")
    }
}

/// The name people use for an editor, from the name of its settings folder.
fn display_name(folder: &str) -> String {
    // VS Code is the one whose folder does not carry its name.
    match folder {
        "Code" => "VS Code".to_string(),
        "Code - Insiders" => "VS Code Insiders".to_string(),
        other => other.to_string(),
    }
}

/// The editors of the VS Code family that have been used on this machine,
/// in alphabetical order.
///
/// No list of names: an editor is recognized by the folders every editor of
/// this family keeps (`User/globalStorage` next to `User/settings.json`),
/// so one that appears tomorrow is found too.
pub fn editors(env: &Env) -> Vec<Editor> {
    editors_with(env, &app_folders(env))
}

/// Where each editor keeps its extensions, by the name of its settings
/// folder, read in the `product.json` of the apps of these folders.
fn extension_folders(env: &Env, app_folders: &[PathBuf]) -> HashMap<String, PathBuf> {
    let mut found = HashMap::new();
    for folder in app_folders {
        let Ok(apps) = fs::read_dir(folder) else {
            continue;
        };
        for app in apps.flatten() {
            let product = app.path().join("Contents/Resources/app/product.json");
            // Most apps have no such file: they are not editors.
            let Ok(text) = fs::read_to_string(product) else {
                continue;
            };
            let Ok(product) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            // `as_str` gives `None` for a missing or non-text value.
            let (Some(settings), Some(data)) = (
                product["nameShort"].as_str(),
                product["dataFolderName"].as_str(),
            ) else {
                continue;
            };
            found.insert(settings.to_string(), env.home.join(data).join("extensions"));
        }
    }
    found
}

/// [`editors`], with the folders to look for the editors' apps in.
fn editors_with(env: &Env, app_folders: &[PathBuf]) -> Vec<Editor> {
    let Ok(entries) = fs::read_dir(settings_base(env)) else {
        return Vec::new();
    };
    let extension_folders = extension_folders(env, app_folders);
    let mut found = Vec::new();
    // `flatten` skips the entries that cannot be read.
    for entry in entries.flatten() {
        let user = entry.path().join("User");
        if !user.join("globalStorage").is_dir() {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().to_string();
        let name = display_name(&folder);
        let extensions = extension_folders.get(&folder).cloned();
        // An editor profile with settings of its own does not read the
        // main file for this setting.
        if let Ok(profiles) = fs::read_dir(user.join("profiles")) {
            for profile in profiles.flatten() {
                let settings = profile.path().join("settings.json");
                if settings.is_file() {
                    found.push(Editor {
                        name: format!("{name} (profile {})", profile.file_name().to_string_lossy()),
                        settings,
                        extensions: extensions.clone(),
                    });
                }
            }
        }
        found.push(Editor {
            name,
            settings: user.join("settings.json"),
            extensions,
        });
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// The name of the setting, or an error for a tool with no extension.
fn setting(tool: &Tool) -> Result<&'static str> {
    match &tool.extension {
        Some(extension) => Ok(extension.wrapper_setting),
        None => bail!("{} has no editor extension", tool.name),
    }
}

/// The file's text. A file that does not exist yet reads as empty: an
/// editor whose settings were never changed has none.
fn read(file: &Path) -> Result<String> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("cannot read {}", file.display())),
    }
}

fn state_of(text: &str, key: &str, wrapper: &Path) -> State {
    let raw = match jsonc::get(text, key) {
        Ok(Some(raw)) => raw,
        Ok(None) => return State::Off,
        Err(error) => return State::Unreadable(error.to_string()),
    };
    // The value as written, quotes included; a path is expected.
    match serde_json::from_str::<String>(&raw) {
        // The extension reads an empty text as "not set".
        Ok(value) if value.is_empty() => State::Off,
        Ok(value) if Path::new(&value) == wrapper => State::On,
        Ok(value) => State::Other(value),
        Err(_) => State::Other(raw),
    }
}

pub fn state(env: &Env, tool: &Tool, editor: &Editor) -> State {
    let Ok(key) = setting(tool) else {
        return State::Off;
    };
    match read(&editor.settings) {
        Ok(text) => state_of(&text, key, &tool.wrapper_path(env)),
        Err(error) => State::Unreadable(format!("{error:#}")),
    }
}

/// The line to add by hand when Orberus cannot write it.
pub fn line(env: &Env, tool: &Tool) -> Result<String> {
    let key = serde_json::to_string(setting(tool)?)?;
    let value = serde_json::to_string(&tool.wrapper_path(env).display().to_string())?;
    Ok(format!("{key}: {value}"))
}

/// Writes the text back, in place rather than through a temporary file: a
/// settings file is often a link into a dotfiles repository, and a rename
/// would replace the link by a plain file.
fn write(file: &Path, text: &str) -> Result<()> {
    fs::write(file, text).with_context(|| format!("cannot write {}", file.display()))
}

/// Points the editor's extension to our wrapper. Returns false when it
/// already was. Refuses to replace another program, or to touch a file it
/// does not understand.
pub fn turn_on(env: &Env, tool: &Tool, editor: &Editor) -> Result<bool> {
    let key = setting(tool)?;
    let wrapper = tool.wrapper_path(env);
    let text = read(&editor.settings)?;
    match state_of(&text, key, &wrapper) {
        State::On => return Ok(false),
        State::Off => {}
        State::Other(program) => bail!(
            "{} already starts {} through another program ({program}): left as it is",
            editor.name,
            tool.name
        ),
        State::Unreadable(why) => bail!(
            "cannot change {} ({why}); add this line to it by hand: {}",
            editor.settings.display(),
            line(env, tool)?
        ),
    }
    let changed = jsonc::set(&text, key, &wrapper.display().to_string())?;
    // Read our own work again before writing: a file we would no longer
    // understand is not written.
    if state_of(&changed, key, &wrapper) != State::On {
        bail!(
            "cannot change {} safely; add this line to it by hand: {}",
            editor.settings.display(),
            line(env, tool)?
        );
    }
    write(&editor.settings, &changed)?;
    Ok(true)
}

/// Removes the setting when it names our wrapper. Returns false when there
/// was nothing of ours: another program's setting is never removed.
pub fn turn_off(env: &Env, tool: &Tool, editor: &Editor) -> Result<bool> {
    let key = setting(tool)?;
    let wrapper = tool.wrapper_path(env);
    let text = read(&editor.settings)?;
    if state_of(&text, key, &wrapper) != State::On {
        return Ok(false);
    }
    let Some(changed) = jsonc::remove(&text, key)? else {
        return Ok(false);
    };
    if state_of(&changed, key, &wrapper) == State::On {
        // The setting was written twice: the earlier one is ours too.
        bail!(
            "cannot change {} safely; remove {key} from it by hand",
            editor.settings.display()
        );
    }
    write(&editor.settings, &changed)?;
    Ok(true)
}

/// True when a folder of extensions holds the tool's extension: one folder
/// per extension, named after its identifier and its version.
fn holds_extension(folder: &Path, tool: &Tool) -> bool {
    let Some(extension) = &tool.extension else {
        return false;
    };
    let prefix = format!("{}-", extension.id);
    fs::read_dir(folder).is_ok_and(|extensions| {
        extensions
            .flatten()
            .any(|extension| extension.file_name().to_string_lossy().starts_with(&prefix))
    })
}

/// Whether the tool's extension is installed in this editor. `None` when
/// that cannot be told, because the editor's app was not found.
pub fn has_extension(tool: &Tool, editor: &Editor) -> Option<bool> {
    editor
        .extensions
        .as_deref()
        .map(|folder| holds_extension(folder, tool))
}

/// True when the tool's extension is installed in some editor, whichever.
///
/// Editors of this family keep their extensions in a folder of their own
/// under the home folder (`~/.vscode/extensions`, `~/.cursor/extensions`).
fn extension_somewhere(env: &Env, tool: &Tool) -> bool {
    let Ok(entries) = fs::read_dir(&env.home) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry.file_name().to_string_lossy().starts_with('.')
            && holds_extension(&entry.path().join("extensions"), tool)
    })
}

/// The editors where the tool's extension starts the tool by itself: the
/// setting is not on, and the extension is installed there. For an editor
/// that cannot be told apart, an extension installed anywhere counts.
pub fn skipping(env: &Env, tool: &Tool) -> Vec<Editor> {
    // Looked up at most once, and only if some editor needs it.
    let mut somewhere = None;
    editors(env)
        .into_iter()
        .filter(|editor| state(env, tool, editor) == State::Off)
        .filter(|editor| match has_extension(tool, editor) {
            Some(installed) => installed,
            None => *somewhere.get_or_insert_with(|| extension_somewhere(env, tool)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::CLAUDE;

    const KEY: &str = "claudeCode.claudeProcessWrapper";

    fn world() -> (tempfile::TempDir, Env) {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let env = Env {
            root: home.join(".orberus"),
            home,
        };
        (temp, env)
    }

    /// The editors, with apps looked for in the test's own folder only.
    fn found(env: &Env) -> Vec<Editor> {
        editors_with(env, &[env.home.join("Applications")])
    }

    /// The app of an editor, with what its `product.json` says.
    fn app(env: &Env, name: &str, settings_folder: &str, data_folder: &str) {
        let folder = env
            .home
            .join("Applications")
            .join(name)
            .join("Contents/Resources/app");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("product.json"),
            format!(
                "{{\"nameShort\": \"{settings_folder}\", \"dataFolderName\": \"{data_folder}\"}}"
            ),
        )
        .unwrap();
    }

    /// An editor as it leaves its folders behind, with these settings.
    fn editor(env: &Env, folder: &str, settings: Option<&str>) -> PathBuf {
        let user = settings_base(env).join(folder).join("User");
        fs::create_dir_all(user.join("globalStorage")).unwrap();
        let file = user.join("settings.json");
        if let Some(settings) = settings {
            fs::write(&file, settings).unwrap();
        }
        file
    }

    #[test]
    fn finds_the_editors_of_the_family_and_nothing_else() {
        let (_temp, env) = world();
        editor(&env, "Cursor", Some("{}"));
        editor(&env, "Code", None);
        // An app that only has a folder there is not an editor.
        fs::create_dir_all(settings_base(&env).join("Slack/User")).unwrap();
        fs::create_dir_all(settings_base(&env).join("Claude")).unwrap();

        let found = found(&env);
        let names: Vec<&str> = found.iter().map(|editor| editor.name.as_str()).collect();
        assert_eq!(names, ["Cursor", "VS Code"]);
        assert!(found[1].settings.ends_with("Code/User/settings.json"));
    }

    #[test]
    fn an_editor_profile_with_its_own_settings_is_listed_apart() {
        let (_temp, env) = world();
        let main = editor(&env, "Code", Some("{}"));
        let profiles = main.parent().unwrap().join("profiles");
        fs::create_dir_all(profiles.join("-1a2b3c")).unwrap();
        fs::write(profiles.join("-1a2b3c/settings.json"), "{}").unwrap();
        // A profile folder with no settings of its own is not listed.
        fs::create_dir_all(profiles.join("builtin/agents")).unwrap();

        let names: Vec<String> = found(&env).into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["VS Code", "VS Code (profile -1a2b3c)"]);
    }

    #[test]
    fn on_writes_the_setting_and_off_gives_the_file_back() {
        let (_temp, env) = world();
        let before = "{\n    // my settings\n    \"editor.fontSize\": 13,\n}\n";
        let file = editor(&env, "Code", Some(before));
        let code = found(&env).remove(0);
        assert_eq!(state(&env, &CLAUDE, &code), State::Off);

        assert!(turn_on(&env, &CLAUDE, &code).unwrap());
        assert_eq!(state(&env, &CLAUDE, &code), State::On);
        let written = fs::read_to_string(&file).unwrap();
        let wrapper = CLAUDE.wrapper_path(&env);
        assert!(written.contains(&format!("\"{KEY}\": \"{}\"", wrapper.display())));
        assert!(written.contains("// my settings"));
        // A second time changes nothing.
        assert!(!turn_on(&env, &CLAUDE, &code).unwrap());

        assert!(turn_off(&env, &CLAUDE, &code).unwrap());
        assert_eq!(fs::read_to_string(&file).unwrap(), before);
        assert!(!turn_off(&env, &CLAUDE, &code).unwrap());
    }

    #[test]
    fn an_editor_with_no_settings_file_gets_one() {
        let (_temp, env) = world();
        let file = editor(&env, "Cursor", None);
        let cursor = found(&env).remove(0);
        assert_eq!(state(&env, &CLAUDE, &cursor), State::Off);
        assert!(turn_on(&env, &CLAUDE, &cursor).unwrap());
        assert_eq!(state(&env, &CLAUDE, &cursor), State::On);
        // Strict JSON, as an editor would have written it.
        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(parsed[KEY], CLAUDE.wrapper_path(&env).display().to_string());
    }

    #[test]
    fn another_program_is_never_replaced_nor_removed() {
        let (_temp, env) = world();
        let before = format!("{{\n    \"{KEY}\": \"/opt/company/claude-wrap\"\n}}\n");
        let file = editor(&env, "Code", Some(&before));
        let code = found(&env).remove(0);
        assert_eq!(
            state(&env, &CLAUDE, &code),
            State::Other("/opt/company/claude-wrap".to_string())
        );
        let error = turn_on(&env, &CLAUDE, &code).unwrap_err().to_string();
        assert!(error.contains("another program"), "{error}");
        assert!(!turn_off(&env, &CLAUDE, &code).unwrap());
        assert_eq!(fs::read_to_string(&file).unwrap(), before);
    }

    #[test]
    fn an_empty_value_counts_as_not_set() {
        let (_temp, env) = world();
        let file = editor(
            &env,
            "Code",
            Some(&format!("{{\n    \"{KEY}\": \"\"\n}}\n")),
        );
        let code = found(&env).remove(0);
        assert_eq!(state(&env, &CLAUDE, &code), State::Off);
        assert!(turn_on(&env, &CLAUDE, &code).unwrap());
        assert_eq!(state(&env, &CLAUDE, &code), State::On);
        // Replaced in place: the setting is still written once.
        assert_eq!(fs::read_to_string(&file).unwrap().matches(KEY).count(), 1);
    }

    #[test]
    fn a_file_it_does_not_understand_is_left_alone() {
        let (_temp, env) = world();
        let before = "{ \"editor.fontSize\": 13";
        let file = editor(&env, "Code", Some(before));
        let code = found(&env).remove(0);
        assert!(matches!(state(&env, &CLAUDE, &code), State::Unreadable(_)));
        let error = turn_on(&env, &CLAUDE, &code).unwrap_err().to_string();
        // The message carries the line to add by hand.
        assert!(error.contains(KEY), "{error}");
        assert!(!turn_off(&env, &CLAUDE, &code).unwrap_or(false));
        assert_eq!(fs::read_to_string(&file).unwrap(), before);
    }

    #[test]
    fn the_extension_is_looked_for_in_each_editors_own_folder() {
        let (_temp, env) = world();
        editor(&env, "Code", Some("{}"));
        editor(&env, "Cursor", Some("{}"));
        editor(&env, "Unknown Fork", Some("{}"));
        app(&env, "Visual Studio Code.app", "Code", ".vscode");
        app(&env, "Cursor.app", "Cursor", ".cursor");
        // An app that is not an editor, and one whose file says nothing.
        fs::create_dir_all(env.home.join("Applications/Notes.app/Contents")).unwrap();
        app(&env, "Broken.app", "", "");
        fs::create_dir_all(
            env.home
                .join(".vscode/extensions/anthropic.claude-code-2.1.325-darwin-arm64"),
        )
        .unwrap();
        fs::create_dir_all(env.home.join(".cursor/extensions/ms-python.python-1.0")).unwrap();

        let found = found(&env);
        let has: Vec<(&str, Option<bool>)> = found
            .iter()
            .map(|editor| (editor.name.as_str(), has_extension(&CLAUDE, editor)))
            .collect();
        assert_eq!(
            has,
            [
                ("Cursor", Some(false)),
                // No app found for it: it cannot be told.
                ("Unknown Fork", None),
                ("VS Code", Some(true)),
            ]
        );
    }

    #[test]
    fn the_extension_anywhere_counts_only_under_an_editors_folder() {
        let (_temp, env) = world();
        assert!(!extension_somewhere(&env, &CLAUDE));
        fs::create_dir_all(env.home.join(".vscode/extensions/ms-python.python-1.0")).unwrap();
        // The same name outside an editor's extensions folder is nothing.
        fs::create_dir_all(env.home.join("extensions/anthropic.claude-code-2.1")).unwrap();
        assert!(!extension_somewhere(&env, &CLAUDE));
        fs::create_dir_all(
            env.home
                .join(".cursor/extensions/anthropic.claude-code-2.1.325-darwin-arm64"),
        )
        .unwrap();
        assert!(extension_somewhere(&env, &CLAUDE));
    }
}
