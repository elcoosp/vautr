//! Subcommand dispatch for the Vautr CLI.
//!
//! Wave 0.3 routes each recognized [`Command`](crate::args::Command) to a safe,
//! non-panicking stub. The real implementations land in Wave B4.

use crate::args::Command;
use crate::error::CliResult;

pub mod get;
pub mod list;
pub mod login;
pub mod run;

/// Dispatch a parsed subcommand to its handler.
pub fn dispatch(command: Command) -> CliResult<()> {
    match command {
        Command::Get => get::run(None),
        Command::List => list::run(),
        Command::Run => run::run(),
        Command::Login => login::run(),
    }
}
