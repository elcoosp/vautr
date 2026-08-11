//! `vautr-cli run -- <cmd>` — inject secrets into a child process environment.
//!
//! Safe stub (Wave 0.3): resolves to [`CliError::NotYetImplemented`]. Real env
//! injection + `std::process::Command` spawning lands in Wave B4.

use crate::error::{CliError, CliResult};

/// Run a command with resolved secrets injected into its environment.
pub fn run() -> CliResult<()> {
    Err(CliError::NotYetImplemented("run"))
}
