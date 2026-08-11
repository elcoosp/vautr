//! `vautr-cli login` — machine-account login.
//!
//! Safe stub (Wave 0.3): resolves to [`CliError::NotYetImplemented`]. Real
//! access-token based login flows land in Wave B4 (depends on Wave A2).

use crate::error::{CliError, CliResult};

/// Log in a machine account and persist a session.
pub fn run() -> CliResult<()> {
    Err(CliError::NotYetImplemented("login"))
}
