//! OPAQUE client registration/login state machine. REQ-AUTH-02/03, api.md §3.
//! The server never receives the Master Password or any password equivalent.
//!
//! These wrappers drive `vautr_crypto::opaque` so the (async) HTTP layer only
//! ferries byte blobs between client and server.

use vautr_crypto::opaque;
use zeroize::Zeroizing;

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
pub fn registration_start(password: &str) -> (Vec<u8>, Vec<u8>) {
    let (msg, state) =
        opaque::client_register_start(password.as_bytes()).expect("opaque registration start");
    (state, msg)
}

/// Finish OPAQUE registration after the server responds.
/// Returns `(registration_upload, export_key, client_state)`.
pub fn registration_finish(
    client_state: &[u8],
    server_response: &[u8],
    password: &str,
    credential_identifier: &[u8],
) -> (Vec<u8>, Vec<u8>, RegistrationState) {
    let (upload, export) = opaque::client_register_finish(
        client_state,
        server_response,
        password.as_bytes(),
        credential_identifier,
    )
    .expect("opaque registration finish");
    (upload, export, RegistrationState::Complete)
}

/// Begin OPAQUE login. Returns `(client_state_bytes, message_to_server)`.
pub fn login_start(password: &str) -> (Vec<u8>, Vec<u8>) {
    let (msg, state) = opaque::client_login_start(password.as_bytes()).expect("opaque login start");
    (state, msg)
}

/// Finish OPAQUE login after the server responds.
/// Returns `(login_upload, session_key, client_state)`.
pub fn login_finish(
    client_state: &[u8],
    server_response: &[u8],
    password: &str,
    credential_identifier: &[u8],
) -> (Vec<u8>, Zeroizing<Vec<u8>>, LoginState) {
    let (upload, session_key) = opaque::client_login_finish(
        client_state,
        server_response,
        password.as_bytes(),
        credential_identifier,
    )
    .expect("opaque login finish");
    (
        upload,
        Zeroizing::new(session_key),
        LoginState::Authenticated,
    )
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
pub fn registration_start_struct(p: RegistrationStartParams) -> RegistrationStartResult {
    let (client_state, message) = registration_start(&p.password);
    RegistrationStartResult {
        client_state,
        message,
    }
}

/// Finish OPAQUE registration via the structured API.
pub fn registration_finish_struct(p: RegistrationFinishParams) -> RegistrationFinishResult {
    let (upload, export_key, state) = registration_finish(
        &p.client_state,
        &p.server_response,
        &p.password,
        &p.credential_identifier,
    );
    RegistrationFinishResult {
        upload,
        export_key,
        state,
    }
}

/// Begin OPAQUE login via the structured API.
pub fn login_start_struct(p: LoginStartParams) -> LoginStartResult {
    let (client_state, message) = login_start(&p.password);
    LoginStartResult {
        client_state,
        message,
    }
}

/// Finish OPAQUE login via the structured API.
pub fn login_finish_struct(p: LoginFinishParams) -> LoginFinishResult {
    let (upload, session_key, state) = login_finish(
        &p.client_state,
        &p.server_response,
        &p.password,
        &p.credential_identifier,
    );
    LoginFinishResult {
        upload,
        session_key,
        state,
    }
}
