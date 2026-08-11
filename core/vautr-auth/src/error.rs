//! Authentication errors. data.md §8.4 `AuthError`.

#![allow(unused)]

use thiserror::Error;

/// Authentication failures surfaced to the UI.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("biometric unlock unavailable")]
    BiometricUnavailable,
    #[error("keystore error: {0}")]
    KeystoreError(String),
    /// MP verification succeeded but the account has a registered WebAuthn
    /// (FIDO2) second factor that must be satisfied before unlock completes
    /// (VTR-052). Emitted after MP unlock when `webauthn` is enabled.
    #[error("second factor required")]
    SecondFactorRequired,
}
