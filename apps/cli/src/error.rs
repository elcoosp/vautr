//! Error types for the Vautr CLI.

use thiserror::Error;

/// Top-level CLI error type.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CliError {
    /// An unknown command/subcommand was requested.
    #[error("unknown command '{0}'")]
    UnknownCommand(String),

    /// The requested operation is planned but not yet wired in this wave.
    #[error("'{0}' is not yet implemented (Wave B4)")]
    NotYetImplemented(&'static str),

    /// A required argument was missing.
    #[error("missing required argument: {0}")]
    MissingArgument(&'static str),
}

/// Convenience alias for CLI results.
pub type CliResult<T> = Result<T, CliError>;
