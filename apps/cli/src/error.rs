//! Error types for the Vautr CLI.

use thiserror::Error;

/// Top-level CLI error type.
#[derive(Debug, Error)]
pub enum CliError {
    /// An unknown command/subcommand was requested.
    #[error("unknown command '{0}'")]
    UnknownCommand(String),

    /// A required argument was missing.
    #[error("missing required argument: {0}")]
    MissingArgument(&'static str),

    /// An I/O error (reading a config file, spawning a process, etc.).
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON (de)serialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A base64 decoding error.
    #[error("base64 error: {0}")]
    Base64(#[from] base64::DecodeError),

    /// An HTTP transport error.
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// A non-2xx HTTP response. Carries the status and server error message.
    #[error("server error {status}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Server-provided error message.
        message: String,
    },

    /// A cryptographic error from the `vautr-crypto` crate.
    #[error("crypto error: {0}")]
    Crypto(String),

    /// Not logged in / no active credential.
    #[error("not logged in; run `vautr-cli login` first")]
    NotLoggedIn,

    /// The requested secret could not be found by key.
    #[error("no secret with key '{0}'")]
    SecretNotFound(String),

    /// The project key needed to decrypt a secret is not stored locally.
    #[error("no project key available for project {0} (was it created by another client?)")]
    MissingProjectKey(String),

    /// A failure decrypting a secret value.
    #[error("failed to decrypt secret: {0}")]
    Decrypt(String),

    /// No password was provided.
    #[error("no password provided (set VAUTR_PASSWORD or pass via stdin)")]
    NoPassword,
}

/// Convenience alias for CLI results.
pub type CliResult<T> = Result<T, CliError>;

impl CliError {
    /// Build an [`CliError::Api`] from a non-2xx response body.
    ///
    /// Parses the server `ErrorEnvelope` (`{ error, message }`) when possible,
    /// otherwise falls back to the raw body.
    pub fn from_response_status(status: u16, body: &str) -> Self {
        let message = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
            .or_else(|| v(body))
            .unwrap_or_else(|| body.trim().to_string());
        CliError::Api { status, message }
    }
}

fn v(body: &str) -> Option<String> {
    let t = body.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_server_error_envelope() {
        let e =
            CliError::from_response_status(403, r#"{"error":"forbidden","message":"no access"}"#);
        match e {
            CliError::Api { status, message } => {
                assert_eq!(status, 403);
                assert_eq!(message, "no access");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn falls_back_to_raw_body() {
        let e = CliError::from_response_status(500, "boom");
        match e {
            CliError::Api { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "boom");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
