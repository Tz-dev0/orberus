//! The tools Orberus chooses an account for. Today there is one: Claude Code.
//!
//! Everything that names a tool goes through here: the command people type,
//! the variable that tells the tool where its config folder is, and the
//! folder it uses when that variable is not set. The shim, `setup`, `doctor`
//! and `uninstall` read this table instead of spelling `claude` themselves,
//! so adding a tool starts with one more entry in `ALL`.
//!
//! What a tool entry does not cover, on purpose: reading who is logged in
//! (`view::account`), the conversations and their size (`sessions`, `disk`,
//! `usage`), how long they are kept (`retention`) and the keychain entry
//! (`purge`). Those modules read Claude Code's own files, whose layout no
//! other tool shares. A second tool needs its own readers there, and one
//! decision first: where its config folder lives for a profile (a profile's
//! folder under `profiles/` is Claude Code's config folder itself today).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::config::Config;
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
}

pub const CLAUDE: Tool = Tool {
    id: "claude",
    name: "Claude Code",
    command: "claude",
    config_env: "CLAUDE_CONFIG_DIR",
    native_dir: ".claude",
};

/// Every tool Orberus knows. `setup` installs one shim per entry.
pub const ALL: &[&Tool] = &[&CLAUDE];

/// The tool this program was started as, if any: `claude` (or `claude.exe`)
/// means "act as the shim of Claude Code". Any other name, `orberus` first
/// of all, is not a tool and gets the command line interface.
pub fn for_command(invoked: &OsStr) -> Option<&'static Tool> {
    let stem = Path::new(invoked).file_stem()?;
    ALL.iter().copied().find(|tool| stem == tool.command)
}

impl Tool {
    /// The tool's own default config folder: `~/.claude`.
    pub fn native_dir(&self, env: &Env) -> PathBuf {
        env.home.join(self.native_dir)
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
        assert_eq!(for_command(OsStr::new("")), None);
    }

    #[test]
    fn every_tool_has_its_own_command_and_variable() {
        // Two tools sharing a command would install the same shim; sharing
        // a variable would send both to the same config folder.
        for (index, tool) in ALL.iter().enumerate() {
            for other in &ALL[index + 1..] {
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
    }
}
