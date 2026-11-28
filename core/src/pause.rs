//! Pausing Orberus without uninstalling it.
//!
//! While paused, the shim and the editors' wrapper change nothing: the
//! tool starts as if Orberus was not installed, rules and locked rules
//! included. One empty file says so; the shim only checks that it exists,
//! which costs next to nothing each time `claude` starts.
//!
//! A pause is a deliberate, visible act of the user, not a way around the
//! rules that goes unnoticed: the shim says it is paused each time it
//! starts the tool in a terminal, and `status`, `doctor` and the menu bar
//! app say it too.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::paths::Env;

/// The file whose presence means "paused".
fn note(env: &Env) -> PathBuf {
    env.root.join("paused")
}

pub fn is_paused(env: &Env) -> bool {
    note(env).exists()
}

/// Pauses Orberus. Returns false when it already was.
pub fn pause(env: &Env) -> Result<bool> {
    if is_paused(env) {
        return Ok(false);
    }
    let note = note(env);
    fs::create_dir_all(&env.root)
        .with_context(|| format!("cannot create {}", env.root.display()))?;
    fs::write(&note, "").with_context(|| format!("cannot write {}", note.display()))?;
    Ok(true)
}

/// Ends the pause. Returns false when Orberus was not paused.
pub fn resume(env: &Env) -> Result<bool> {
    let note = note(env);
    match fs::remove_file(&note) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("cannot delete {}", note.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pause_lasts_until_resumed() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let env = Env {
            // The data folder does not exist yet: pausing creates it.
            root: home.join(".orberus"),
            home,
        };
        assert!(!is_paused(&env));
        assert!(!resume(&env).unwrap());

        assert!(pause(&env).unwrap());
        assert!(is_paused(&env));
        assert!(!pause(&env).unwrap(), "already paused");

        assert!(resume(&env).unwrap());
        assert!(!is_paused(&env));
    }
}
