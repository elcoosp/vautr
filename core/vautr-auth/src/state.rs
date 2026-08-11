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
    let (msg, state) = opaque::client_register_start(password.as_bytes())
        .expect("opaque registration start");
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
    let (msg, state) =
        opaque::client_login_start(password.as_bytes()).expect("opaque login start");
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
    (upload, Zeroizing::new(session_key), LoginState::Authenticated)
}
