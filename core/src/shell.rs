//! Putting the shim folder in `PATH`, in the user's shell config.
//!
//! A shell does not read the same files each time it starts. zsh reads
//! `~/.zshrc` only when it is interactive (a terminal window), and
//! `~/.zprofile` when it is a login shell. An app or a script that starts
//! zsh to run one command gets a login shell that is not interactive: with
//! the line in `~/.zshrc` alone, `claude` there is the real one and the shim
//! is skipped without a word. So for zsh the line goes in both files.
//!
//! In `~/.zprofile` it is added at the end, after the line Homebrew puts
//! there, which also adds to the front of `PATH`.
//!
//! [`crate::uninstall`] removes these lines again.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::paths::Env;
use crate::uninstall::{path_lines, PATH_COMMENT};

/// The config files of a shell, by its name (`zsh`, `bash`). The first one
/// is read by a terminal window; the others cover apps and scripts. Empty
/// for a shell we do not know how to write to.
pub fn configs(env: &Env, shell: &str) -> Vec<PathBuf> {
    let names: &[&str] = match shell {
        "zsh" => &[".zshrc", ".zprofile"],
        // Terminal on macOS starts bash as a login shell, which reads
        // ~/.bash_profile and not ~/.bashrc.
        "bash" if cfg!(target_os = "macos") => &[".bash_profile"],
        "bash" => &[".bashrc"],
        _ => &[],
    };
    names.iter().map(|name| env.home.join(name)).collect()
}

/// The line to add, written with `$HOME` when possible, so it still works
/// if the home folder moves (a restored backup, a renamed account).
pub fn path_line(env: &Env) -> String {
    // `path_lines` gives the absolute spelling first, then the `$HOME` one.
    path_lines(env).pop().unwrap_or_default()
}

/// True when the file already names the shim folder, in either spelling,
/// whoever wrote the line.
pub fn mentions(env: &Env, file: &Path) -> bool {
    let text = fs::read_to_string(file).unwrap_or_default();
    let bin = env.bin_dir();
    let with_home = bin
        .strip_prefix(&env.home)
        .ok()
        .map(|rest| format!("$HOME/{}", rest.display()));
    text.contains(&bin.display().to_string())
        || with_home.is_some_and(|spelling| text.contains(&spelling))
}

/// Adds our comment and our line at the end of the file, creating it when
/// it does not exist. Nothing already there is rewritten.
pub fn add_line(env: &Env, file: &Path) -> Result<()> {
    let mut open = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
        .with_context(|| format!("cannot open {}", file.display()))?;
    writeln!(open, "\n{PATH_COMMENT}\n{}", path_line(env))
        .with_context(|| format!("cannot write {}", file.display()))
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
    fn zsh_gets_the_line_for_terminals_and_for_apps() {
        let (_temp, env) = world();
        assert_eq!(
            configs(&env, "zsh"),
            vec![env.home.join(".zshrc"), env.home.join(".zprofile")]
        );
        assert!(configs(&env, "fish").is_empty());
    }

    #[test]
    fn add_line_goes_after_what_is_there_and_uninstall_removes_it() {
        let (_temp, env) = world();
        let file = env.home.join(".zprofile");
        let brew = "eval \"$(/opt/homebrew/bin/brew shellenv zsh)\"\n";
        fs::write(&file, brew).unwrap();
        assert!(!mentions(&env, &file));

        add_line(&env, &file).unwrap();
        assert!(mentions(&env, &file));
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.starts_with(brew));
        assert!(text.ends_with("export PATH=\"$HOME/.orberus/bin:$PATH\"\n"));

        let done = crate::uninstall::clean_shell_files(&env).unwrap();
        assert_eq!(done.cleaned, vec![file.clone()]);
        assert_eq!(fs::read_to_string(&file).unwrap(), brew);
    }

    #[test]
    fn a_line_written_by_hand_counts_as_there() {
        let (_temp, env) = world();
        let file = env.home.join(".zshrc");
        fs::write(&file, "path=($HOME/.orberus/bin $path)\n").unwrap();
        assert!(mentions(&env, &file));
    }
}
