//! `stratad`, the Strata server (PLAN §7.1 `bin/stratad`, §14).
//!
//! Subcommands: `serve`, `keygen`, `create-user [--admin]`, `openapi`, `migrate`,
//! `bootstrap-roles`. The library form exists so the startup checks and commands are tested
//! directly; `main.rs` only parses arguments and dispatches.

pub mod checks;
pub mod cli;
pub mod commands;
pub mod logging;
pub mod serve;
