//! A profile's own Claude Code settings file: `settings.json` in its config
//! folder. Read and written as a whole JSON object, so every setting we do
//! not know stays as it is, in its place (`serde_json` keeps the order).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};

use crate::config::{self, Config};
use crate::paths::Env;

/// Where the settings of a profile are.
pub(crate) fn file(env: &Env, config: &Config, name: &str) -> Result<PathBuf> {
    let dir = match config::profile_dir(env, config, name)? {
        Some(dir) => dir,
        None => crate::tool::CLAUDE.native_dir(env),
    };
    Ok(dir.join("settings.json"))
}

/// Reads the file as a JSON object. `None` when the file does not exist.
pub(crate) fn read(file: &Path) -> Result<Option<Map<String, Value>>> {
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        // No file yet is normal for a profile that was never started.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("cannot read {}", file.display())),
    };
    match serde_json::from_str(&text) {
        Ok(Value::Object(settings)) => Ok(Some(settings)),
        Ok(_) => bail!("{} is not a JSON object", file.display()),
        Err(error) => bail!("{} is not valid JSON: {error}", file.display()),
    }
}

/// Writes the whole object back.
pub(crate) fn write(file: &Path, settings: Map<String, Value>) -> Result<()> {
    let mut text = serde_json::to_string_pretty(&Value::Object(settings))?;
    text.push('\n');
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    // Written next to the file, then moved over it: a crash in the middle
    // leaves the old settings whole instead of half a file.
    let draft = file.with_extension("json.orberus-tmp");
    fs::write(&draft, text).with_context(|| format!("cannot write {}", draft.display()))?;
    fs::rename(&draft, file).with_context(|| format!("cannot write {}", file.display()))?;
    Ok(())
}
