//! The core of Orberus: everything it knows and does, with no interface.
//!
//! The `orberus` command (`cli/`) and the menu bar app (`app/`) are two
//! faces of this library. They ask it and show the answer; a rule, a check
//! or a file format belongs here, so both get it.
//!
//! Nothing here reads global state. Every function is handed an
//! [`paths::Env`] (home folder, data folder), so a test can run against a
//! temporary folder.
//!
//! - What the user set up: [`config`], [`rules`], [`paths`].
//! - Which account a folder gets, and starting the tool with it:
//!   [`resolve`], [`git`], [`tool`], [`launch`].
//! - Reading Claude Code's own files: [`view`] (who is logged in),
//!   [`sessions`], [`usage`], [`disk`], [`retention`].
//! - Removing things: [`purge`], [`uninstall`].

pub mod config;
pub mod disk;
pub mod git;
pub mod launch;
pub mod paths;
pub mod purge;
pub mod resolve;
pub mod retention;
pub mod rules;
pub mod sessions;
pub mod tool;
pub mod uninstall;
pub mod usage;
pub mod view;
