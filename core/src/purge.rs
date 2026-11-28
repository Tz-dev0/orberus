//! Deleting what a profile leaves on the machine: its Claude Code folder,
//! on macOS its login in the keychain, and the data of its own copy of
//! Claude Desktop when it had one, plus its managed Codex home and login.
//!
//! This is the one place that destroys data, and only on request
//! (`orberus remove --purge`, or the checkbox in the menu bar app). It is
//! never on the shim's path, so running other programs here is fine.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::config::{self, Config};
use crate::launch::{self, Target};
use crate::paths::Env;
use crate::tool::{self, CODEX};

/// What `purge_profile` did, for the message shown afterwards.
#[derive(Debug)]
pub struct Purged {
    /// The folder that was deleted.
    pub folder: PathBuf,
    /// True when a login was found in the keychain and removed.
    pub login: bool,
    /// How many of the user's rules went with the profile.
    pub rules: usize,
    /// True when the profile had its own Claude Desktop, whose data folder
    /// (and so its sign-in there) was deleted.
    pub desktop: bool,
    /// The Codex home deleted after signing out, if this profile used Codex.
    pub codex: Option<PathBuf>,
}

/// The folder a purge of this profile would delete, or why it must not be.
///
/// A profile's folder can be set by hand in `config.toml`, so it is checked
/// before anything is deleted: never `~/.claude`, never the home folder or
/// one above it, never the folder that holds this tool's own config.
pub fn folder(env: &Env, config: &Config, name: &str) -> Result<PathBuf> {
    let Some(dir) = config::profile_dir(env, config, name)? else {
        bail!(
            "profile '{name}' reuses ~/.claude, which is never deleted: \
             remove the profile without deleting its folder"
        );
    };
    // `starts_with` compares whole path components: `/a/b` starts with `/a`.
    let holds_home = env.home.starts_with(&dir);
    let holds_config = env.root.starts_with(&dir);
    if holds_home || holds_config || dir == crate::tool::CLAUDE.native_dir(env) {
        bail!(
            "refusing to delete {}: it holds more than this profile",
            dir.display()
        );
    }
    Ok(dir)
}

/// The existing managed Codex home. A symlink could resolve to a different
/// login, so refuse to sign out through one.
pub fn codex_folder(env: &Env, name: &str) -> Result<Option<PathBuf>> {
    let dir = tool::codex_dir(env, name)?;
    let metadata = match dir.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("cannot inspect {}", dir.display()))
        }
    };
    let parent = dir.parent().context("the Codex home has no parent")?;
    let parent = parent
        .symlink_metadata()
        .context("cannot inspect the Codex profiles directory")?;
    if !metadata.is_dir() || !parent.is_dir() {
        bail!(
            "refusing to purge {}: the Codex home must be a managed directory",
            dir.display()
        );
    }
    Ok(Some(dir))
}

/// The name of the keychain entry Claude Code uses for a config folder.
///
/// Seen on macOS with Claude Code 2.1.323: `Claude Code-credentials-` plus
/// the first 8 hex digits of the SHA-256 of the folder's path. This is not a
/// documented interface, so a future version may name it differently; the
/// entry would then simply not be found.
fn keychain_service(dir: &Path) -> Result<String> {
    // `shasum` ships with macOS. Hashing by hand or adding a crate for one
    // hash would be more code than this call.
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("cannot run `shasum`")?;
    // `take` moves the pipe out of `child`, so it is closed when this block
    // ends. `shasum` waits for that before it answers.
    {
        let mut input = child.stdin.take().context("cannot write to `shasum`")?;
        input.write_all(dir.as_os_str().as_encoded_bytes())?;
    }
    let output = child.wait_with_output().context("`shasum` failed")?;
    let text = String::from_utf8_lossy(&output.stdout);
    let digest = text.split_whitespace().next().unwrap_or_default();
    if !output.status.success() || digest.len() < 8 {
        bail!("`shasum` gave no digest");
    }
    Ok(format!("Claude Code-credentials-{}", &digest[..8]))
}

/// Removes the login Claude Code stored in the macOS keychain for this
/// config folder. Returns false when there was none. Does nothing on other
/// systems, where the login lives inside the folder.
pub fn forget_login(dir: &Path) -> Result<bool> {
    if !cfg!(target_os = "macos") {
        return Ok(false);
    }
    let service = keychain_service(dir)?;
    let status = Command::new("security")
        .args(["delete-generic-password", "-s", &service])
        // On success `security` prints the entry it deleted: not needed.
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("cannot run `security`")?;
    match status.code() {
        Some(0) => Ok(true),
        // 44 is "the item could not be found": nobody was logged in.
        Some(44) => Ok(false),
        _ => bail!("cannot remove '{service}' from the keychain"),
    }
}

/// Removes a profile's Claude Desktop, Codex and Claude Code data, then its
/// entry and rules in the config.
///
/// The order matters: if a step fails, the ones after it have not run, and
/// the profile is still in the config to try again. Claude Desktop can refuse
/// when a copy is open, so check it before signing out of Codex or Claude Code.
pub fn purge_profile(env: &Env, config: &mut Config, name: &str) -> Result<Purged> {
    let folder = folder(env, config, name)?;
    let codex = codex_folder(env, name)?;
    let codex_logout = codex
        .as_deref()
        .map(|dir| {
            let real = launch::find_real(env, config, &CODEX)?;
            launch::command(
                &CODEX,
                &real,
                Target::Profile {
                    name,
                    dir: Some(dir),
                },
            )
        })
        .transpose()
        .with_context(|| format!("cannot prepare Codex logout for profile '{name}'"))?;
    let desktop = crate::desktop::forget(env, config, name)?;
    if let Some((dir, mut command)) = codex.as_deref().zip(codex_logout) {
        let status = command
            .arg("logout")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .with_context(|| format!("cannot run Codex logout for profile '{name}'"))?;
        if !status.success() {
            bail!("Codex logout failed for profile '{name}' ({status})");
        }
        fs::remove_dir_all(dir).with_context(|| format!("cannot delete {}", dir.display()))?;
    }
    let login = forget_login(&folder)?;
    if folder.exists() {
        fs::remove_dir_all(&folder)
            .with_context(|| format!("cannot delete {}", folder.display()))?;
    }
    let rules = config::remove_profile(env, config, name)?;
    Ok(Purged {
        folder,
        login,
        rules,
        desktop,
        codex,
    })
}
