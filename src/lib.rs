//! Core of claude-profiles: configuration, rules, profile resolution and
//! launching the real `claude` binary. The CLI and the shim are thin layers
//! on top of this library, and a future menu bar app will reuse it as is.

pub mod config;
pub mod git;
pub mod launch;
pub mod paths;
pub mod purge;
pub mod resolve;
pub mod view;
