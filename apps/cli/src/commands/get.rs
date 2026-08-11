//! `vautr-cli get <id>` — reveal a single secret.
//!
//! Safe stub (Wave 0.3): resolves to [`CliError::NotYetImplemented`]. Real secret
//! retrieval via the client SDK lands in Wave B4.

use crate::error::{CliError, CliResult};

/// Reveal a single secret to stdout. `id` is the secret identifier.
pub fn run(id: Option<String>) -> CliResult<()> {
    match id {
        Some(_) => Err(CliError::NotYetImplemented("get")),
        None => Err(CliError::MissingArgument("id")),
    }
}
