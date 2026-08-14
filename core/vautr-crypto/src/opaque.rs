//! OPAQUE PAKE integration (client + server helpers).
//!
//! Spec: [`docs/architecture/crypto.md`] §2.2 Step 2; [`docs/architecture/api.md`] §3.
//! The server stores only an OPAQUE registration record (our `opaque_record`
//! column) and a server-setup public key (`server_config.opaque_server_public_key`).
//! The Master Password / MK never reaches the server (REQ-AUTH-02).
//!
//! Uses opaque-ke 4.x. Cipher suite: Ristretto255 OPRF, Triple-DH key exchange,
//! `argon2::Argon2` KSF (Argon2id) — an expensive KSF so a leaked server DB
//! cannot be brute-forced offline (REQ-AUTH-02: the MK/salt never leaves the
//! client, but the OPAQUE record still must resist offline attack).
//!
//! The OPAQUE KSF is independent of the Vault MK Argon2id (crypto.md Step 1);
//! the `argon2` feature on `opaque-ke` supplies `impl Ksf for argon2::Argon2`.

use crate::error::{CryptoError, Result};
use opaque_ke::{
    CipherSuite, ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters, ServerLogin, ServerLoginParameters, ServerRegistration,
    ServerSetup,
};
use rand::rngs::OsRng;
use sha2::Sha512;

/// Vautr OPAQUE ciphersuite: Ristretto255 + Triple-DH + Argon2 KSF.
#[derive(Clone, Copy)]
pub struct VautrSuite;

impl CipherSuite for VautrSuite {
    type OprfCs = opaque_ke::Ristretto255;
    type KeyExchange = opaque_ke::TripleDh<opaque_ke::Ristretto255, Sha512>;
    type Ksf = opaque_ke::argon2::Argon2<'static>;
}

// --- Server setup (the OPAQUE server long-term keypair, published as public key) ---

/// Generate the server OPAQUE setup; returns its serializable public key bytes
/// (`server_config.opaque_server_public_key`). Stored once per server instance.
/// The corresponding `ServerSetup::deserialize` is used to reload it at runtime.
pub fn server_setup_public_key() -> Result<Vec<u8>> {
    let setup = ServerSetup::<VautrSuite>::new(&mut OsRng);
    Ok(setup.serialize().to_vec())
}

fn load_server_setup(bytes: &[u8]) -> Result<ServerSetup<VautrSuite>> {
    ServerSetup::<VautrSuite>::deserialize(bytes).map_err(|e| CryptoError::AuthError(e.to_string()))
}

// --- Registration (client start → server start → client finish → server finish) ---

/// Client registration start. Returns `(registration_request_bytes, client_state_bytes)`.
/// The client persists `client_state_bytes` until [`client_register_finish`].
pub fn client_register_start(password: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut rng = OsRng;
    let res = ClientRegistration::<VautrSuite>::start(&mut rng, password)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    Ok((
        res.message.serialize().to_vec(),
        res.state.serialize().to_vec(),
    ))
}

/// Server registration start. `server_setup` = bytes from [`server_setup_public_key`].
/// Returns `registration_response_bytes`.
pub fn server_register_start(
    server_setup: &[u8],
    request: &[u8],
    username: &[u8],
) -> Result<Vec<u8>> {
    let setup = load_server_setup(server_setup)?;
    let req = opaque_ke::RegistrationRequest::<VautrSuite>::deserialize(request)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    let res = ServerRegistration::<VautrSuite>::start(&setup, req, username)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    Ok(res.message.serialize().to_vec())
}

/// Client registration finish. Returns `(registration_upload_bytes, export_key)`.
/// `export_key` is the OPAQUE export key (can be used as additional auth binding).
pub fn client_register_finish(
    client_state: &[u8],
    server_response: &[u8],
    password: &[u8],
    credential_identifier: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let state = opaque_ke::ClientRegistration::<VautrSuite>::deserialize(client_state)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    let resp = opaque_ke::RegistrationResponse::<VautrSuite>::deserialize(server_response)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    let params = ClientRegistrationFinishParameters {
        identifiers: opaque_ke::Identifiers {
            client: Some(credential_identifier),
            server: Some(b"vautr-server"),
        },
        ..Default::default()
    };
    let mut rng = OsRng;
    let res = state
        .finish(&mut rng, password, resp, params)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    Ok((res.message.serialize().to_vec(), res.export_key.to_vec()))
}

/// Server registration finish. Returns the stored `opaque_record` bytes.
pub fn server_register_finish(upload: &[u8]) -> Result<Vec<u8>> {
    let up = opaque_ke::RegistrationUpload::<VautrSuite>::deserialize(upload)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    let rec = ServerRegistration::<VautrSuite>::finish(up);
    Ok(rec.serialize().to_vec())
}

// --- Login (client start → server start → client finish → server finish) ---

/// Client login start. Returns `(login_request_bytes, client_state_bytes)`.
pub fn client_login_start(password: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut rng = OsRng;
    let res = ClientLogin::<VautrSuite>::start(&mut rng, password)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    Ok((
        res.message.serialize().to_vec(),
        res.state.serialize().to_vec(),
    ))
}

/// Server login start. `record` = the stored `opaque_record` for the user.
/// Returns `(login_response_bytes, server_state_bytes)`.
/// A missing record is handled by passing `None` (indistinguishable response).
pub fn server_login_start(
    server_setup: &[u8],
    record: Option<&[u8]>,
    request: &[u8],
    username: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let setup = load_server_setup(server_setup)?;
    let req = opaque_ke::CredentialRequest::<VautrSuite>::deserialize(request)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    let pwfile = match record {
        Some(r) => Some(
            ServerRegistration::<VautrSuite>::deserialize(r)
                .map_err(|e| CryptoError::AuthError(e.to_string()))?,
        ),
        None => None,
    };
    let mut rng = OsRng;
    let params = ServerLoginParameters {
        identifiers: opaque_ke::Identifiers {
            client: Some(username),
            server: Some(b"vautr-server"),
        },
        ..Default::default()
    };
    let res = ServerLogin::<VautrSuite>::start(&mut rng, &setup, pwfile, req, username, params)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    Ok((
        res.message.serialize().to_vec(),
        res.state.serialize().to_vec(),
    ))
}

/// Client login finish. Returns `(login_upload_bytes, session_key)`.
pub fn client_login_finish(
    client_state: &[u8],
    server_response: &[u8],
    password: &[u8],
    credential_identifier: &[u8],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let state = opaque_ke::ClientLogin::<VautrSuite>::deserialize(client_state)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    let resp = opaque_ke::CredentialResponse::<VautrSuite>::deserialize(server_response)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    let mut rng = OsRng;
    let params = ClientLoginFinishParameters {
        identifiers: opaque_ke::Identifiers {
            client: Some(credential_identifier),
            server: Some(b"vautr-server"),
        },
        ..Default::default()
    };
    let res = state
        .finish(&mut rng, password, resp, params)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    Ok((res.message.serialize().to_vec(), res.session_key.to_vec()))
}

/// Server login finish. Returns the session key (used to mint the bearer token).
pub fn server_login_finish(server_state: &[u8], upload: &[u8]) -> Result<Vec<u8>> {
    let state = opaque_ke::ServerLogin::<VautrSuite>::deserialize(server_state)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    let up = opaque_ke::CredentialFinalization::<VautrSuite>::deserialize(upload)
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    let res = state
        .finish(up, ServerLoginParameters::default())
        .map_err(|e| CryptoError::AuthError(e.to_string()))?;
    Ok(res.session_key.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_register_then_login_roundtrip() {
        let pw = b"correct horse battery staple";
        let user = b"alice@example.com";

        let setup = server_setup_public_key().unwrap();

        // registration
        let (creq, cstate) = client_register_start(pw).unwrap();
        let sresp = server_register_start(&setup, &creq, user).unwrap();
        let (cupload, _export) = client_register_finish(&cstate, &sresp, pw, user).unwrap();
        let record = server_register_finish(&cupload).unwrap();

        // login
        let (lreq, lstate) = client_login_start(pw).unwrap();
        let (lresp, sstate) = server_login_start(&setup, Some(&record), &lreq, user).unwrap();
        let (lupload, c_session) = client_login_finish(&lstate, &lresp, pw, user).unwrap();
        let s_session = server_login_finish(&sstate, &lupload).unwrap();

        assert_eq!(
            c_session, s_session,
            "client/server session keys must match"
        );
        assert_eq!(c_session.len(), 64);
    }

    #[test]
    fn opaque_login_wrong_password_fails() {
        let pw = b"right password";
        let wrong = b"wrong password";
        let user = b"bob@example.com";
        let setup = server_setup_public_key().unwrap();

        let (creq, cstate) = client_register_start(pw).unwrap();
        let sresp = server_register_start(&setup, &creq, user).unwrap();
        let (cupload, _) = client_register_finish(&cstate, &sresp, pw, user).unwrap();
        let record = server_register_finish(&cupload).unwrap();

        let (lreq, lstate) = client_login_start(wrong).unwrap();
        let (lresp, _sstate) = server_login_start(&setup, Some(&record), &lreq, user).unwrap();
        let res = client_login_finish(&lstate, &lresp, wrong, user);
        assert!(res.is_err(), "login with wrong password must fail");
    }
}
