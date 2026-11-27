//! Undoing what `orberus setup` and the install commands put on the
//! machine: the shim, the editors' wrapper, the `PATH` line in the shell
//! config, the binary, the menu bar app and, on request only, the profiles
//! themselves.
//!
//! Each step is a small function that says what it did, so the CLI can
//! report it line by line. `~/.claude` is never touched by any of them.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::paths::Env;
use crate::tool;

/// The comment `setup` writes above its `PATH` line. `uninstall` looks for
/// the same text, which is why it lives in one place.
pub const PATH_COMMENT: &str = "# orberus: the claude shim must come before the real claude";

/// Where the menu bar app is looked for, instead of the usual Applications
/// folders. Used by tests, so that they never see the real ones.
pub const APPS_ENV: &str = "ORBERUS_APPS";

/// The identifier of the menu bar app, as written in its Info.plist.
const APP_IDENTIFIER: &str = "com.github.tzdev0.orberus";
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
        let ours = trimmed == PATH_COMMENT || lines.iter().any(|ours| ours == trimmed);
        if !ours {
            kept.push(line);
            continue;
        }
        changed = true;
        // `last` looks at the previous kept line without removing it;
        // `pop` then drops it when it was the blank line above our comment.
        if trimmed == PATH_COMMENT && kept.last().is_some_and(|last| last.trim().is_empty()) {
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
pub fn shim(env: &Env) -> Option<PathBuf> {
    let shim = tool::CLAUDE.shim_path(env);
    // `symlink_metadata` does not follow the link: a shim whose target is
    // gone still counts as installed.
    shim.symlink_metadata().is_ok().then_some(shim)
}

/// Removes the shim. Returns false when there was none.
pub fn remove_shim(env: &Env) -> Result<bool> {
    let Some(shim) = shim(env) else {
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

/// The copies of the menu bar app that are installed.
///
/// A folder named like the app is only counted when its Info.plist carries
/// our identifier, so another app of the same name is never touched.
pub fn installed_apps(env: &Env) -> Vec<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    app_folders(env)
        .into_iter()
        .map(|folder| folder.join(APP_NAME))
        .filter(|app| {
            fs::read_to_string(app.join("Contents/Info.plist"))
                .is_ok_and(|plist| plist.contains(APP_IDENTIFIER))
        })
        .collect()
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
