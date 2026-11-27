//! `orberus ide`: the editors found here, and whether their Claude Code
//! extension goes through Orberus.

use anyhow::{bail, Result};
use orberus_core::ide::{self, Editor, State};
use orberus_core::paths::Env;
use orberus_core::tool::{self, Tool};
use orberus_core::view::short_path;

use crate::commands::setup::install_link;
use crate::commands::Switch;
use crate::ui::{Cell, Style, Ui};

pub(crate) fn ide_command(
    ui: &Ui,
    env: &Env,
    switch: Option<Switch>,
    editor: Option<String>,
) -> Result<i32> {
    let tool = &tool::CLAUDE;
    let editors = ide::editors(env);
    let Some(switch) = switch else {
        return list(ui, env, tool, &editors);
    };
    let named = editor.is_some();
    let editors = chosen(editors, editor.as_deref())?;
    if editors.is_empty() {
        println!("{NO_EDITOR}");
        return Ok(0);
    }
    match switch {
        Switch::On => on(ui, env, tool, &editors, named),
        Switch::Off => off(ui, env, tool, &editors),
    }
}

/// The editors to act on: the one that was named, or all of them. The name
/// is the one `orberus ide` lists, in any case: `cursor`, `"VS Code"`.
fn chosen(editors: Vec<Editor>, name: Option<&str>) -> Result<Vec<Editor>> {
    let Some(name) = name else {
        return Ok(editors);
    };
    // `partition` sorts the list in two: the editors of that name, and the
    // others, kept to name them in the error.
    let (named, others): (Vec<Editor>, Vec<Editor>) = editors
        .into_iter()
        .partition(|editor| editor.name.eq_ignore_ascii_case(name));
    if named.is_empty() {
        if others.is_empty() {
            bail!("no editor named '{name}': none was found on this machine");
        }
        let names: Vec<&str> = others.iter().map(|editor| editor.name.as_str()).collect();
        bail!("no editor named '{name}' (found: {})", names.join(", "));
    }
    Ok(named)
}

const NO_EDITOR: &str = "No editor of the VS Code family found (VS Code, Cursor and the like).";

/// What the shim already covers, said each time: the question people ask
/// next is "and my other editor?".
fn others(ui: &Ui) {
    ui.detail("JetBrains IDEs, and any editor where Claude Code runs as the `claude` command,");
    ui.detail("already go through the shim: nothing to turn on for them.");
}

fn list(ui: &Ui, env: &Env, tool: &Tool, editors: &[Editor]) -> Result<i32> {
    if editors.is_empty() {
        println!("{NO_EDITOR}");
        others(ui);
        return Ok(0);
    }
    let mut skipped = false;
    let rows: Vec<Vec<Cell>> = editors
        .iter()
        .map(|editor| {
            let extension = match ide::state(env, tool, editor) {
                State::On => Cell::styled("through Orberus", Style::Green),
                // Nothing skips Orberus where the extension is not there.
                State::Off if ide::has_extension(tool, editor) == Some(false) => {
                    Cell::styled("not installed", Style::Dim)
                }
                State::Off => {
                    skipped = true;
                    Cell::styled("skips Orberus", Style::Yellow)
                }
                State::Other(program) => Cell::new(format!("through {program}")),
                State::Unreadable(_) => Cell::styled("settings not readable", Style::Red),
            };
            vec![
                Cell::new(editor.name.as_str()),
                extension,
                Cell::styled(short_path(env, &editor.settings), Style::Dim),
            ]
        })
        .collect();
    ui.table(
        &["Editor", &format!("{} extension", tool.name), "Settings"],
        &rows,
    );
    if skipped {
        println!();
        println!(
            "To pick the account by folder there too: {}",
            ui.command("orberus ide on")
        );
    }
    println!();
    others(ui);
    Ok(0)
}

/// `named` tells that the user asked for this editor by its name: it is
/// then written even where the extension is not installed yet. Without a
/// name, an editor that has no use for the setting is left as it is.
fn on(ui: &Ui, env: &Env, tool: &Tool, editors: &[Editor], named: bool) -> Result<i32> {
    // The file the setting names must be there before an editor reads it.
    let wrapper = tool.wrapper_path(env);
    if !wrapper.exists() {
        install_link(&wrapper)?;
    }
    let mut problems = 0;
    let mut written = 0;
    for editor in editors {
        let without_extension = ide::has_extension(tool, editor) == Some(false);
        if without_extension && !named && ide::state(env, tool, editor) == State::Off {
            ui.detail(&format!(
                "{}: the {} extension is not installed there, left as it is",
                editor.name, tool.name
            ));
            continue;
        }
        match ide::turn_on(env, tool, editor) {
            Ok(true) => {
                written += 1;
                ui.success(&format!(
                    "{}: its {} extension now goes through Orberus",
                    editor.name, tool.name
                ));
            }
            Ok(false) => ui.success(&format!("{}: already on", editor.name)),
            Err(error) => {
                problems += 1;
                ui.warning(&format!("{error:#}"));
            }
        }
    }
    if written > 0 {
        println!();
        ui.detail("The account is picked from the folder the editor has open, by the same rules");
        ui.detail("as in a terminal. Reload the editor's window for it to apply.");
    }
    Ok(if problems == 0 { 0 } else { 1 })
}

fn off(ui: &Ui, env: &Env, tool: &Tool, editors: &[Editor]) -> Result<i32> {
    let mut problems = 0;
    for editor in editors {
        match ide::turn_off(env, tool, editor) {
            Ok(true) => ui.success(&format!(
                "{}: its {} extension starts {} itself again",
                editor.name, tool.name, tool.name
            )),
            Ok(false) => ui.detail(&format!("{}: nothing to remove", editor.name)),
            Err(error) => {
                problems += 1;
                ui.warning(&format!("{error:#}"));
            }
        }
    }
    Ok(if problems == 0 { 0 } else { 1 })
}
