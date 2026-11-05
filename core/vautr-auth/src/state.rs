//! OPAQUE client registration/login state machine. REQ-AUTH-02/03, api.md §3.
//! The server never receives the Master Password or any password equivalent.
//!
//! These wrappers drive `vautr_crypto::opaque` so the (async) HTTP layer only
//! ferries byte blobs between client and server.

use vautr_crypto::error::CryptoError;
use vautr_crypto::opaque;
use zeroize::Zeroizing;

use crate::error::AuthError;

/// Map a raw OPAQUE/crypto failure to the auth-level protocol error.
fn protocol_err(context: &str, e: CryptoError) -> AuthError {
    AuthError::Protocol(format!("{context}: {e}"))
}

/// Client-side registration flow state (api.md §3.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistrationState {
    Start,
    AwaitingServer,
    Complete,
}

/// Client-side login flow state (api.md §3.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginState {
    Start,
    AwaitingServer,
    Authenticated,
}

/// Begin OPAQUE registration. Returns `(client_state_bytes, message_to_server)`.
///
/// # Errors
/// Returns [`AuthError::Protocol`] instead of panicking when the OPAQUE
/// handshake machinery rejects the input (server-controlled data must never
/// panic the client).
pub fn registration_start(password: &str) -> Result<(Vec<u8>, Vec<u8>), AuthError> {
    let (msg, state) = opaque::client_register_start(password.as_bytes())
        .map_err(|e| protocol_err("opaque registration start", e))?;
    Ok((state, msg))
}

/// Finish OPAQUE registration after the server responds.
/// Returns `(registration_upload, export_key, client_state)`.
///
/// # Errors
/// Returns [`AuthError::Protocol`] on a malformed server response.
pub fn registration_finish(
    client_state: &[u8],
    server_response: &[u8],
    password: &str,
    credential_identifier: &[u8],
) -> Result<(Vec<u8>, Vec<u8>, RegistrationState), AuthError> {
    let (upload, export) = opaque::client_register_finish(
        client_state,
        server_response,
        password.as_bytes(),
        credential_identifier,
    )
    .map_err(|e| protocol_err("opaque registration finish", e))?;
    Ok((upload, export, RegistrationState::Complete))
}

/// Begin OPAQUE login. Returns `(client_state_bytes, message_to_server)`.
///
/// # Errors
/// Returns [`AuthError::Protocol`] instead of panicking on bad input.
pub fn login_start(password: &str) -> Result<(Vec<u8>, Vec<u8>), AuthError> {
    let (msg, state) = opaque::client_login_start(password.as_bytes())
        .map_err(|e| protocol_err("opaque login start", e))?;
    Ok((state, msg))
}

/// Finish OPAQUE login after the server responds.
/// Returns `(login_upload, session_key, client_state)`.
///
/// # Errors
/// Returns [`AuthError::Protocol`] on a malformed/tampered server response
/// (including an OPAQUE authentication failure).
pub fn login_finish(
    client_state: &[u8],
    server_response: &[u8],
    password: &str,
    credential_identifier: &[u8],
) -> Result<(Vec<u8>, Zeroizing<Vec<u8>>, LoginState), AuthError> {
    let (upload, session_key) = opaque::client_login_finish(
        client_state,
        server_response,
        password.as_bytes(),
        credential_identifier,
    )
    .map_err(|e| protocol_err("opaque login finish", e))?;
    Ok((
        upload,
        Zeroizing::new(session_key),
        LoginState::Authenticated,
    ))
}

// ---------------------------------------------------------------------------
// Structured (named-parameter) API — reduces call-site parameter bloat and makes
// the registration/login contract self-documenting. Each wrapper delegates to
// the positional fns above so the two surfaces stay in lockstep. New callers
// should prefer these; the positional fns remain for back-compat.
// ---------------------------------------------------------------------------

/// Inputs for [`registration_start_struct`].
pub struct RegistrationStartParams {
    pub password: String,
}

/// Output of [`registration_start_struct`].
pub struct RegistrationStartResult {
    pub client_state: Vec<u8>,
    pub message: Vec<u8>,
}

/// Inputs for [`registration_finish_struct`].
pub struct RegistrationFinishParams {
    pub client_state: Vec<u8>,
    pub server_response: Vec<u8>,
    pub password: String,
    pub credential_identifier: Vec<u8>,
}

/// Output of [`registration_finish_struct`].
pub struct RegistrationFinishResult {
    pub upload: Vec<u8>,
    pub export_key: Vec<u8>,
    pub state: RegistrationState,
}

/// Inputs for [`login_start_struct`].
pub struct LoginStartParams {
    pub password: String,
}

/// Output of [`login_start_struct`].
pub struct LoginStartResult {
    pub client_state: Vec<u8>,
    pub message: Vec<u8>,
}

/// Inputs for [`login_finish_struct`].
pub struct LoginFinishParams {
    pub client_state: Vec<u8>,
    pub server_response: Vec<u8>,
    pub password: String,
    pub credential_identifier: Vec<u8>,
}

/// Output of [`login_finish_struct`].
pub struct LoginFinishResult {
    pub upload: Vec<u8>,
    pub session_key: Zeroizing<Vec<u8>>,
    pub state: LoginState,
}

/// Begin OPAQUE registration via the structured API.
pub fn registration_start_struct(
    p: RegistrationStartParams,
) -> Result<RegistrationStartResult, AuthError> {
    let (client_state, message) = registration_start(&p.password)?;
    Ok(RegistrationStartResult {
        client_state,
        message,
    })
}

/// Finish OPAQUE registration via the structured API.
pub fn registration_finish_struct(
    p: RegistrationFinishParams,
) -> Result<RegistrationFinishResult, AuthError> {
    let (upload, export_key, state) = registration_finish(
        &p.client_state,
        &p.server_response,
        &p.password,
        &p.credential_identifier,
    )?;
    Ok(RegistrationFinishResult {
        upload,
        export_key,
        state,
    })
}

/// Begin OPAQUE login via the structured API.
pub fn login_start_struct(p: LoginStartParams) -> Result<LoginStartResult, AuthError> {
    let (client_state, message) = login_start(&p.password)?;
    Ok(LoginStartResult {
        client_state,
        message,
    })
}

/// Finish OPAQUE login via the structured API.
pub fn login_finish_struct(p: LoginFinishParams) -> Result<LoginFinishResult, AuthError> {
    let (upload, session_key, state) = login_finish(
        &p.client_state,
        &p.server_response,
        &p.password,
        &p.credential_identifier,
    )?;
    Ok(LoginFinishResult {
        upload,
        session_key,
        state,
    })
}
