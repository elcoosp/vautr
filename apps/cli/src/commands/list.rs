//! `vautr-cli list` — list secrets and projects.
//!
//! Safe stub (Wave 0.3): resolves to [`CliError::NotYetImplemented`]. Real listing
//! over the machine-account session lands in Wave B4.

use crate::error::{CliError, CliResult};

/// List secrets and projects for the authenticated machine account.
pub fn run() -> CliResult<()> {
    Err(CliError::NotYetImplemented("list"))
}
