//! The tools Orberus chooses an account for.
//!
//! Everything that names a tool goes through here: the command people type,
//! the variable that tells the tool where its config folder is, and the
//! folder it uses when that variable is not set. Claude Code supports a
//! shim and an editor wrapper. Codex CLI is available through `orberus run`.
//!
//! A tool can be started in two ways, and Orberus stands in front of both.
//! People and most editors run its command, which finds the shim first in
//! `PATH`. An editor extension that carries its own copy of the tool starts
//! that copy directly instead: for those there is the wrapper, a second
//! name of this program that the extension is told to start in place of its
//! copy (see [`crate::ide`]).
//!
//! What a tool entry does not cover, on purpose: reading who is logged in
//! (`view::account`), the conversations and their size (`sessions`, `disk`,
//! `usage`), how long they are kept (`retention`) and the keychain entry
//! (`purge`). Those modules read Claude Code's own files, whose layout no
//! other tool shares.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::{self, Config};
use crate::paths::Env;

// `'static` strings: the table is written in the program itself, so every
// entry lives as long as the program and can be shared freely as `&Tool`.
#[derive(Debug, PartialEq, Eq)]
pub struct Tool {
    /// Short name used in code and, later, in config keys: `claude`.
    pub id: &'static str,
    /// The name people know it by: `Claude Code`.
    pub name: &'static str,
    /// What people type, and the name the shim is installed under.
    pub command: &'static str,
    /// The variable that points the tool to a config folder.
    pub config_env: &'static str,
    /// The folder the tool uses when that variable is not set, under the
    /// home folder. A native profile keeps using it.
    native_dir: &'static str,
    /// The extension that brings the tool into editors of the VS Code
    /// family, when it has one.
    pub extension: Option<Extension>,
}

/// An editor extension that starts its own copy of a tool.
#[derive(Debug, PartialEq, Eq)]
pub struct Extension {
    /// Its identifier. An installed copy is a folder whose name starts
    /// with it: `anthropic.claude-code-2.1.325-darwin-arm64`.
    pub id: &'static str,
    /// Its setting that names a program to start in place of its own copy
    /// of the tool. That program is handed the copy as its first argument.
    pub wrapper_setting: &'static str,
}

pub const CLAUDE: Tool = Tool {
    id: "claude",
    name: "Claude Code",
    command: "claude",
    config_env: "CLAUDE_CONFIG_DIR",
    native_dir: ".claude",
    // Both names are read in the extension's own package.json, and the
    // setting is in Claude Code's documentation of its VS Code extension.
    extension: Some(Extension {
        id: "anthropic.claude-code",
        wrapper_setting: "claudeCode.claudeProcessWrapper",
    }),
};

pub const CODEX: Tool = Tool {
    id: "codex",
    name: "Codex CLI",
    command: "codex",
    config_env: "CODEX_HOME",
    native_dir: ".codex",
    extension: None,
};

/// Tools that support being invoked as a shim. Codex is opt-in through `run`.
pub const ALL: &[&Tool] = &[&CLAUDE];

/// The tool this program was started as, if any: `claude` (or `claude.exe`)
/// means "act as the shim of Claude Code". Any other name, `orberus` first
/// of all, is not a tool and gets the command line interface.
pub fn for_command(invoked: &OsStr) -> Option<&'static Tool> {
    let stem = Path::new(invoked).file_stem()?;
    ALL.iter().copied().find(|tool| stem == tool.command)
}

/// The tool this program was started as a wrapper for, if any:
/// `claude-wrapper` means "an editor extension starts me in place of its own
/// copy of Claude Code".
pub fn for_wrapper(invoked: &OsStr) -> Option<&'static Tool> {
    let stem = Path::new(invoked).file_stem()?.to_str()?;
    let command = stem.strip_suffix(WRAPPER_SUFFIX)?;
    ALL.iter().copied().find(|tool| tool.command == command)
}

/// What follows a tool's command in the name of its wrapper.
const WRAPPER_SUFFIX: &str = "-wrapper";

/// Managed Codex home for a profile. Lowercase names keep their usual spelling.
pub(crate) fn codex_dir(env: &Env, name: &str) -> Result<PathBuf> {
    config::validate_name(name)?;
    // Escape uppercase letters so `Work` and `work` get separate homes
    // even on case-insensitive volumes. `~` cannot appear in a profile name.
    let mut folder = String::with_capacity(name.len());
    for letter in name.chars() {
        if letter.is_ascii_uppercase() {
            folder.push('~');
        }
        folder.push(letter.to_ascii_lowercase());
    }
    let dir = env.root.join("codex").join(folder);
    std::path::absolute(&dir).with_context(|| format!("cannot resolve {}", dir.display()))
}

impl Tool {
    /// The tool's own default config folder, such as `~/.claude`.
    pub fn native_dir(&self, env: &Env) -> PathBuf {
        env.home.join(self.native_dir)
    }

    /// Config directory for a profile selected by the resolver.
    pub fn profile_dir(&self, env: &Env, config: &Config, name: &str) -> Result<Option<PathBuf>> {
        if self == &CODEX {
            codex_dir(env, name).map(Some)
        } else {
            config::profile_dir(env, config, name)
        }
    }

    /// The file name of the shim in Orberus' `bin` folder.
    pub fn shim_name(&self) -> String {
        if cfg!(windows) {
            format!("{}.exe", self.command)
        } else {
            self.command.to_string()
        }
    }

    /// Where the shim of this tool is installed.
    pub fn shim_path(&self, env: &Env) -> PathBuf {
        env.bin_dir().join(self.shim_name())
    }

    /// The file name of the wrapper: `claude-wrapper`.
    pub fn wrapper_name(&self) -> String {
        let extension = if cfg!(windows) { ".exe" } else { "" };
        format!("{}{WRAPPER_SUFFIX}{extension}", self.command)
    }

    /// Where the wrapper of this tool is installed. Not in the `bin`
    /// folder: that one is in `PATH`, and a second name starting with
    /// `claude` there would get in the way each time `claude` is typed.
    pub fn wrapper_path(&self, env: &Env) -> PathBuf {
        env.ide_dir().join(self.wrapper_name())
    }

    /// The file names the real tool can have in a PATH folder.
    pub fn executables(&self) -> Vec<String> {
        if cfg!(windows) {
            vec![
                format!("{}.exe", self.command),
                format!("{}.cmd", self.command),
            ]
        } else {
            vec![self.command.to_string()]
        }
    }

    /// The path of the real binary when the config forces one
    /// (`claude_bin`), instead of looking for it in PATH.
    pub fn forced_bin<'a>(&self, config: &'a Config) -> Option<&'a str> {
        // One setting per tool. Written as a `match` on the id so that a new
        // tool cannot silently inherit Claude Code's path.
        match self.id {
            "claude" => config.claude_bin.as_deref(),
            "codex" => config.codex_bin.as_deref(),
            _ => None,
        }
    }

    /// The name of that setting, for messages: `claude_bin`.
    pub fn bin_setting(&self) -> String {
        format!("{}_bin", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_it_is_started_under_picks_the_tool() {
        assert_eq!(for_command(OsStr::new("claude")), Some(&CLAUDE));
        assert_eq!(
            for_command(OsStr::new("/home/me/.orberus/bin/claude")),
            Some(&CLAUDE)
        );
        // Windows starts the copy named claude.exe.
        assert_eq!(for_command(OsStr::new("claude.exe")), Some(&CLAUDE));
    }

    #[test]
    fn any_other_name_is_the_command_line() {
        assert_eq!(for_command(OsStr::new("orberus")), None);
        assert_eq!(for_command(OsStr::new("/usr/local/bin/orberus")), None);
        // Close to a tool's name is not the tool.
        assert_eq!(for_command(OsStr::new("claude-wrapper")), None);
        assert_eq!(for_command(OsStr::new("codex")), None);
        assert_eq!(for_command(OsStr::new("")), None);
    }

    #[test]
    fn the_wrapper_has_its_own_name() {
        assert_eq!(for_wrapper(OsStr::new("claude-wrapper")), Some(&CLAUDE));
        assert_eq!(
            for_wrapper(OsStr::new("/home/me/.orberus/ide/claude-wrapper")),
            Some(&CLAUDE)
        );
        assert_eq!(for_wrapper(OsStr::new("claude-wrapper.exe")), Some(&CLAUDE));
        // The shim and the command line are not the wrapper, and the
        // wrapper of a tool we do not know is nothing.
        assert_eq!(for_wrapper(OsStr::new("claude")), None);
        assert_eq!(for_wrapper(OsStr::new("orberus")), None);
        assert_eq!(for_wrapper(OsStr::new("codex-wrapper")), None);
        assert_eq!(for_wrapper(OsStr::new("-wrapper")), None);
    }

    #[test]
    fn every_tool_has_its_own_command_and_variable() {
        // Two tools sharing a command would install the same shim; sharing
        // a variable would send both to the same config folder.
        let tools = [&CLAUDE, &CODEX];
        for (index, tool) in tools.iter().enumerate() {
            for other in &tools[index + 1..] {
                assert_ne!(tool.id, other.id);
                assert_ne!(tool.command, other.command);
                assert_ne!(tool.config_env, other.config_env);
            }
        }
    }

    #[test]
    fn the_native_folder_is_under_home() {
        let env = Env {
            home: PathBuf::from("/home/me"),
            root: PathBuf::from("/home/me/.orberus"),
        };
        assert_eq!(CLAUDE.native_dir(&env), PathBuf::from("/home/me/.claude"));
        assert_eq!(
            CLAUDE.shim_path(&env),
            PathBuf::from("/home/me/.orberus/bin").join(CLAUDE.shim_name())
        );
        // The wrapper stays out of the folder that is in PATH.
        assert_eq!(
            CLAUDE.wrapper_path(&env),
            PathBuf::from("/home/me/.orberus/ide").join(CLAUDE.wrapper_name())
        );
    }
}
