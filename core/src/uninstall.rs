//! Undoing what `orberus setup` and the install commands put on the
//! machine: the shim, the editors' wrapper, the `PATH` line in the shell
//! config, the binary, the menu bar app and, on request only, the profiles
//! themselves.
//!
//! Each step is a small function that says what it did, so the CLI can
//! report it line by line. `~/.claude` is never touched by any of them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::desktop;
use crate::login;
use crate::paths::Env;
use crate::tool;

/// The comment `setup` writes above its `PATH` line. `uninstall` looks for
/// the same text, which is why it lives in one place.
pub const PATH_COMMENT: &str = "# orberus: put the shims before the tools they launch";
const OLD_PATH_COMMENT: &str = "# orberus: the claude shim must come before the real claude";

/// Where the menu bar app is looked for, instead of the usual Applications
/// folders. Used by tests, so that they never see the real ones.
pub const APPS_ENV: &str = "ORBERUS_APPS";

/// The identifier of the menu bar app, as written in its Info.plist.
pub(crate) const APP_IDENTIFIER: &str = "com.github.tzdev0.orberus";
const APP_NAME: &str = "Orberus.app";

/// The line `setup` adds to a shell config, in its two spellings: with
/// `$HOME` when the folder is under the home folder, and absolute.
pub fn path_lines(env: &Env) -> Vec<String> {
    let bin = env.bin_dir();
    let mut lines = vec![format!("export PATH=\"{}:$PATH\"", bin.display())];
    if let Ok(rest) = bin.strip_prefix(&env.home) {
        lines.push(format!("export PATH=\"$HOME/{}:$PATH\"", rest.display()));
    }
    lines
}

/// The shell config files where `setup` may have written its line.
fn shell_files(env: &Env) -> Vec<PathBuf> {
    [".zshrc", ".zprofile", ".bash_profile", ".bashrc"]
        .iter()
        .map(|name| env.home.join(name))
        .collect()
}

/// `text` without the lines `setup` wrote, or `None` when it holds none.
///
/// Only exact matches go: the comment, and the `PATH` line in one of its two
/// spellings. A line the user wrote in another way is left alone. The empty
/// line `setup` put before its comment goes with it.
fn strip_path_lines(text: &str, lines: &[String]) -> Option<String> {
    let mut kept: Vec<&str> = Vec::new();
    let mut changed = false;
    for line in text.lines() {
        let trimmed = line.trim();
        let comment = matches!(trimmed, PATH_COMMENT | OLD_PATH_COMMENT);
        let ours = comment || lines.iter().any(|ours| ours == trimmed);
        if !ours {
            kept.push(line);
            continue;
        }
        changed = true;
        // `last` looks at the previous kept line without removing it;
        // `pop` then drops it when it was the blank line above our comment.
        if comment && kept.last().is_some_and(|last| last.trim().is_empty()) {
            kept.pop();
        }
    }
    if !changed {
        return None;
    }
    let mut result = kept.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    Some(result)
}

/// What became of the shell config files.
#[derive(Debug, Default)]
pub struct ShellFiles {
    /// Files our lines were removed from.
    pub cleaned: Vec<PathBuf>,
    /// Files that still name the shim folder, in a line we did not write.
    pub still_mention: Vec<PathBuf>,
}

/// Removes the `PATH` line from every shell config that has it.
pub fn clean_shell_files(env: &Env) -> Result<ShellFiles> {
    let lines = path_lines(env);
    let mut result = ShellFiles::default();
    for file in shell_files(env) {
        // A file that is missing or unreadable holds nothing of ours.
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let text = match strip_path_lines(&text, &lines) {
            Some(stripped) => {
                // Written in place rather than through a temporary file: a
                // dotfile is often a link into a dotfiles repository, and a
                // rename would replace the link by a plain file.
                fs::write(&file, &stripped)
                    .with_context(|| format!("cannot write {}", file.display()))?;
                result.cleaned.push(file.clone());
                stripped
            }
            None => text,
        };
        if text.contains(".orberus/bin") {
            result.still_mention.push(file);
        }
    }
    Ok(result)
}

/// True when a shell config holds a line `setup` wrote.
pub fn shell_files_have_our_line(env: &Env) -> bool {
    let lines = path_lines(env);
    shell_files(env).iter().any(|file| {
        fs::read_to_string(file).is_ok_and(|text| strip_path_lines(&text, &lines).is_some())
    })
}

/// The shim, when it is installed.
pub fn shim(env: &Env, tool: &tool::Tool) -> Option<PathBuf> {
    let shim = tool.shim_path(env);
    // `symlink_metadata` does not follow the link: a shim whose target is
    // gone still counts as installed.
    let installed = if tool == &tool::CODEX {
        crate::launch::is_orberus_link(&shim)
    } else {
        shim.symlink_metadata().is_ok()
    };
    installed.then_some(shim)
}

/// Removes the shim. Returns false when there was none.
pub fn remove_shim(env: &Env, tool: &tool::Tool) -> Result<bool> {
    let Some(shim) = shim(env, tool) else {
        return Ok(false);
    };
    fs::remove_file(&shim).with_context(|| format!("cannot remove {}", shim.display()))?;
    Ok(true)
}

/// Removes the wrapper that editor extensions start, and its folder.
/// Returns false when there was none.
///
/// To be called after [`crate::ide::turn_off`] for every editor: a setting
/// left behind would name a file that is gone, and the extension would no
/// longer start.
pub fn remove_wrapper(env: &Env) -> Result<bool> {
    let wrapper = tool::CLAUDE.wrapper_path(env);
    let existed = wrapper.symlink_metadata().is_ok();
    if existed {
        fs::remove_file(&wrapper)
            .with_context(|| format!("cannot remove {}", wrapper.display()))?;
    }
    // Only an empty folder goes, as for `bin` below.
    let _ = fs::remove_dir(env.ide_dir());
    Ok(existed)
}

/// Where the running `orberus` is, and whether `uninstall` may remove it.
#[derive(Debug)]
pub enum Binary {
    /// In our own `bin` folder, put there by the install command.
    Ours(PathBuf),
    /// Somewhere else (`cargo install`, a package manager): whoever
    /// installed it removes it.
    Elsewhere(PathBuf),
}

pub fn binary(env: &Env) -> Result<Binary> {
    let me = std::env::current_exe().context("cannot find the running program")?;
    // Both sides are made canonical, so a link in the way does not hide
    // that the program sits in our folder.
    let folder = me.canonicalize().ok().and_then(|path| {
        let parent = path.parent()?.to_path_buf();
        Some(parent)
    });
    let ours = env.bin_dir().canonicalize().ok();
    if folder.is_some() && folder == ours {
        Ok(Binary::Ours(me))
    } else {
        Ok(Binary::Elsewhere(me))
    }
}

/// Removes the running program when it lives in our `bin` folder, then the
/// folder when nothing else is in it. A running program can be deleted on
/// Unix: the file goes when the process ends.
pub fn remove_binary(env: &Env) -> Result<Binary> {
    let found = binary(env)?;
    if let Binary::Ours(path) = &found {
        fs::remove_file(path).with_context(|| format!("cannot remove {}", path.display()))?;
    }
    // `remove_dir` only removes an empty folder, which is what we want: it
    // fails, harmlessly, when the user keeps something else there.
    let _ = fs::remove_dir(env.bin_dir());
    Ok(found)
}

/// The launch scripts the menu bar app wrote. Returns false when there were
/// none.
pub fn remove_launchers(env: &Env) -> Result<bool> {
    let run = env.root.join("run");
    if !run.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(&run).with_context(|| format!("cannot delete {}", run.display()))?;
    Ok(true)
}

/// The folders where apps are installed: the one named by `ORBERUS_APPS`
/// when it is set (tests), the usual two on a Mac, none elsewhere.
pub(crate) fn app_folders(env: &Env) -> Vec<PathBuf> {
    match std::env::var_os(APPS_ENV) {
        Some(folder) if !folder.is_empty() => vec![PathBuf::from(folder)],
        _ if cfg!(target_os = "macos") => vec![
            PathBuf::from("/Applications"),
            env.home.join("Applications"),
        ],
        _ => Vec::new(),
    }
}

/// One copy of the menu bar app on this machine.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledApp {
    /// Its `.app` folder.
    pub bundle: PathBuf,
    /// The numbers of the programs running from it. Empty when it is not
    /// running.
    pub running: Vec<u32>,
}

/// True when that folder is our app: its Info.plist carries our identifier,
/// so another app of the same name is never touched.
fn is_ours(bundle: &Path) -> bool {
    fs::read_to_string(bundle.join("Contents/Info.plist"))
        .is_ok_and(|plist| plist.contains(APP_IDENTIFIER))
}

/// The `.app` folder a program belongs to, when it sits where an app keeps
/// its program: `<Name>.app/Contents/MacOS/<program>`.
fn bundle_of(program: &Path) -> Option<PathBuf> {
    let macos = program.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    // `ends_with` compares whole path components: `NotMacOS` does not end
    // with `MacOS`.
    let shaped = macos.ends_with("MacOS")
        && contents.ends_with("Contents")
        && bundle.extension().is_some_and(|ext| ext == "app");
    // `then` turns the yes or no into "some folder" or "none".
    shaped.then(|| bundle.to_path_buf())
}

/// The running copies among the lines our script printed: one per line, its
/// number, a tab, then its `.app` folder.
fn copies_in(listing: &str) -> Vec<(u32, PathBuf)> {
    listing
        .lines()
        .filter_map(|line| {
            let (pid, bundle) = line.split_once('\t')?;
            if bundle.is_empty() {
                return None;
            }
            Some((pid.trim().parse().ok()?, PathBuf::from(bundle)))
        })
        .collect()
}

/// The copies of the menu bar app that are running, as macOS knows them:
/// for each, its number and its `.app` folder. `None` when macOS could not
/// be asked.
///
/// Asked by the app's identifier rather than by its name or its folder: a
/// copy started from a build folder or from Downloads is found too.
/// `osascript` ships with macOS, and the script only reads a list from
/// AppKit, so no permission is asked for.
fn running_copies() -> Option<Vec<(u32, PathBuf)>> {
    // A test names its own Applications folder. It must never see, let
    // alone quit, the app of the person who runs it.
    if std::env::var_os(APPS_ENV).is_some_and(|folder| !folder.is_empty()) {
        return Some(Vec::new());
    }
    const SCRIPT: &str = r#"ObjC.import("AppKit");
function run(argv) {
  var apps = $.NSRunningApplication.runningApplicationsWithBundleIdentifier(argv[0]);
  var lines = [];
  for (var i = 0; i < apps.count; i++) {
    var app = apps.objectAtIndex(i);
    var url = app.bundleURL;
    lines.push(app.processIdentifier + "\t" + (url.isNil() ? "" : ObjC.unwrap(url.path)));
  }
  return lines.join("\n");
}"#;
    let output = Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT])
        .arg(APP_IDENTIFIER)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| copies_in(&String::from_utf8_lossy(&output.stdout)))
}

/// Keeps, among the folders where a copy may be, the ones that are our app,
/// each once, with the numbers of the programs running from it.
fn copies(candidates: Vec<PathBuf>, running: &[(u32, PathBuf)]) -> Vec<InstalledApp> {
    // One folder can be reached under two names (a link on the way). Names
    // are compared once made canonical; the name shown stays the one found.
    let real = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut found = Vec::new();
    for bundle in candidates {
        let place = real(&bundle);
        if seen.contains(&place) || !is_ours(&bundle) {
            continue;
        }
        let pids = running
            .iter()
            .filter(|(_, at)| real(at) == place)
            .map(|(pid, _)| *pid)
            .collect();
        seen.push(place);
        found.push(InstalledApp {
            bundle,
            running: pids,
        });
    }
    found
}

/// The copies of the menu bar app on this machine, wherever they are.
///
/// Three places are looked at: the Applications folders, the copy that
/// opens at login (the file names its program), and the copies that are
/// running. Nothing is searched for on the disk: a copy that sits in
/// another folder, does not open at login and is not running stays unseen.
pub fn installed_apps(env: &Env) -> Vec<InstalledApp> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    // A list that cannot be read finds no copy; it does not stop a removal.
    let running = running_copies().unwrap_or_default();
    let mut candidates: Vec<PathBuf> = app_folders(env)
        .into_iter()
        .map(|folder| folder.join(APP_NAME))
        .collect();
    // `extend` takes anything that yields folders: an `Option` yields one
    // or none.
    candidates.extend(login::program(env).as_deref().and_then(bundle_of));
    candidates.extend(running.iter().map(|(_, bundle)| bundle.clone()));
    copies(candidates, &running)
}

/// How long a copy that was asked to quit is waited for: 5 seconds, in
/// steps of 200 ms. The app has nothing to save, so it goes at once.
const QUIT_STEPS: u32 = 25;

/// Asks a running copy to quit, as "Quit" in its menu does, and waits for
/// it. True when it is no longer running, and at once when it was not.
///
/// Nothing is forced. False also covers "macOS could not be asked": saying
/// "it quit" without knowing would be a guess.
pub fn quit_app(app: &InstalledApp) -> bool {
    for pid in &app.running {
        // An error here is not worth stopping a removal half way: the wait
        // below finds the copy still running, and the caller says so.
        let _ = desktop::quit(*pid);
    }
    let mut left = app.running.clone();
    for _ in 0..QUIT_STEPS {
        if left.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(200));
        if let Some(now) = running_copies() {
            // `retain` keeps the numbers that are still in the new list.
            left.retain(|pid| now.iter().any(|(running, _)| running == pid));
        }
    }
    left.is_empty()
}

/// Moves one copy of the app to the Trash and returns where it went.
/// Nothing is deleted: the user can put it back.
pub fn trash_app(env: &Env, app: &Path) -> Result<PathBuf> {
    let trash = env.home.join(".Trash");
    fs::create_dir_all(&trash).with_context(|| format!("cannot open {}", trash.display()))?;
    let mut target = trash.join(APP_NAME);
    // The Trash may already hold an older copy: pick a free name.
    let mut number = 2;
    while target.symlink_metadata().is_ok() {
        target = trash.join(format!("Orberus {number}.app"));
        number += 1;
    }
    fs::rename(app, &target)
        .with_context(|| format!("cannot move {} to the Trash", app.display()))?;
    Ok(target)
}

/// The folder that holds the config and the profiles, or why it must not be
/// deleted as a whole.
///
/// `ORBERUS_HOME` can point anywhere, so this is checked before
/// `delete_data` removes the folder: never the home folder or one above it,
/// never `~/.claude` or a folder that holds it.
pub fn data_folder(env: &Env) -> Result<PathBuf> {
    let root = env.root.clone();
    let claude = tool::CLAUDE.native_dir(env);
    // `starts_with` compares whole path components: `/a/b` starts with `/a`.
    if env.home.starts_with(&root) || claude.starts_with(&root) {
        bail!(
            "refusing to delete {}: it holds more than Orberus",
            root.display()
        );
    }
    Ok(root)
}

/// Deletes the config, the team rules and every profile folder kept under
/// the data folder. Returns false when the folder was already gone.
pub fn delete_data(env: &Env) -> Result<bool> {
    let root = data_folder(env)?;
    if !root.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(&root).with_context(|| format!("cannot delete {}", root.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(home: &str) -> Env {
        Env {
            home: PathBuf::from(home),
            root: PathBuf::from(home).join(".orberus"),
        }
    }

    #[test]
    fn removes_the_lines_setup_wrote_and_nothing_else() {
        let env = env("/home/me");
        let before = "export EDITOR=vim\n\n# orberus: the claude shim must come before the real claude\nexport PATH=\"$HOME/.orberus/bin:$PATH\"\nalias ll='ls -l'\n";
        let after = strip_path_lines(before, &path_lines(&env)).unwrap();
        assert_eq!(after, "export EDITOR=vim\nalias ll='ls -l'\n");
    }

    #[test]
    fn removes_the_absolute_spelling_too() {
        let env = env("/home/me");
        let before = "export PATH=\"/home/me/.orberus/bin:$PATH\"\n";
        assert_eq!(strip_path_lines(before, &path_lines(&env)).unwrap(), "");
    }

    #[test]
    fn leaves_a_file_without_our_lines_alone() {
        let env = env("/home/me");
        // Written by hand, in another way: not ours to remove.
        let text = "PATH=$HOME/.orberus/bin:$PATH\nexport PATH\n";
        assert!(strip_path_lines(text, &path_lines(&env)).is_none());
    }

    /// A folder shaped like the app, with an Info.plist that carries the
    /// given identifier.
    fn app_at(bundle: &Path, identifier: &str) {
        fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        fs::write(
            bundle.join("Contents/Info.plist"),
            format!("<key>CFBundleIdentifier</key><string>{identifier}</string>"),
        )
        .unwrap();
    }

    #[test]
    fn a_program_tells_which_app_it_belongs_to() {
        assert_eq!(
            bundle_of(Path::new(
                "/Users/me/builds/Orberus.app/Contents/MacOS/orberus-app"
            )),
            Some(PathBuf::from("/Users/me/builds/Orberus.app"))
        );
        // Not where an app keeps its program: not an app's program.
        assert_eq!(bundle_of(Path::new("/Users/me/bin/orberus-app")), None);
        assert_eq!(
            bundle_of(Path::new("/Users/me/Orberus.app/orberus-app")),
            None
        );
        assert_eq!(
            bundle_of(Path::new("/Users/me/Orberus/Contents/MacOS/orberus-app")),
            None
        );
    }

    #[test]
    fn reads_the_running_copies_macos_listed() {
        let listing = "412\t/Applications/Orberus.app\n9130\t/Users/me/My builds/Orberus.app\n";
        assert_eq!(
            copies_in(listing),
            vec![
                (412, PathBuf::from("/Applications/Orberus.app")),
                (9130, PathBuf::from("/Users/me/My builds/Orberus.app")),
            ]
        );
        // A copy macOS gives no folder for, and a line that is not ours.
        assert!(copies_in("77\t\nnot a line\n").is_empty());
        assert!(copies_in("").is_empty());
    }

    #[test]
    fn a_copy_is_found_wherever_it_is_and_only_when_it_is_ours() {
        let temp = tempfile::tempdir().unwrap();
        let usual = temp.path().join("Applications/Orberus.app");
        let built = temp.path().join("builds/Orberus.app");
        let other = temp.path().join("elsewhere/Orberus.app");
        app_at(&usual, APP_IDENTIFIER);
        app_at(&built, APP_IDENTIFIER);
        app_at(&other, "com.example.somethingelse");
        let gone = temp.path().join("moved away/Orberus.app");

        let running = vec![(412, built.clone()), (9130, other.clone())];
        let found = copies(
            vec![
                usual.clone(),
                // Named twice: by the file that opens it at login, and as
                // a running copy.
                built.clone(),
                built.clone(),
                other,
                gone,
            ],
            &running,
        );
        assert_eq!(
            found,
            vec![
                InstalledApp {
                    bundle: usual,
                    running: vec![],
                },
                InstalledApp {
                    bundle: built,
                    running: vec![412],
                },
            ]
        );
    }

    #[test]
    fn a_copy_that_is_not_running_has_nothing_to_quit() {
        let app = InstalledApp {
            bundle: PathBuf::from("/Applications/Orberus.app"),
            running: vec![],
        };
        assert!(quit_app(&app));
    }

    #[test]
    fn refuses_a_data_folder_that_holds_the_home_or_claude_folder() {
        let mut env = env("/home/me");
        assert_eq!(data_folder(&env).unwrap(), Path::new("/home/me/.orberus"));
        env.root = PathBuf::from("/home/me");
        assert!(data_folder(&env).is_err());
        env.root = PathBuf::from("/home");
        assert!(data_folder(&env).is_err());
        env.root = PathBuf::from("/home/me/.claude");
        assert!(data_folder(&env).is_err());
    }
}
