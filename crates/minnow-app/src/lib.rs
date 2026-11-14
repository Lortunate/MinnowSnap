//! Executable composition and process lifetime.
mod cli;
pub use cli::{Command, parse_command, parse_command_from, run_command};
mod host;
