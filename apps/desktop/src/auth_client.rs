//! Real OPAQUE auth HTTP client. Mirrors `packages/vautr-client-sdk/src/realClient.ts`
//! register/login flow exactly against the live Vautr server (api.md §3).
//!
//! Uses `vautr-auth::state` for the OPAQUE state machine, `reqwest` for HTTP,
//! and `vautr-crypto` / `vautr-keyring` for key derivation and SVK wrapping.

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use reqwest::Client;
use serde::Deserialize;

use uuid::Uuid;

use vautr_auth::state;
use vautr_crypto::{kdf, key_tree, recovery};
use vautr_keyring::wrap;

/// Server error envelope: `{ "error": "...", "message": "..." }`.
#[derive(Deserialize)]
struct ErrorEnvelope {
    #[serde(default)]
    error: String,
    #[serde(default)]
    message: String,
}

/// Send a request and decode the JSON body, but surface a clear error (with the
/// server's `error`/`message` when present) on any non-2xx response instead of
/// letting reqwest fail with a bare "parse error decoding response body".
async fn send_json<T: serde::de::DeserializeOwned>(resp: reqwest::Response) -> Result<T, String> {
    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("read response: {e}"))?;
    if !status.is_success() {
        let msg = serde_json::from_slice::<ErrorEnvelope>(&bytes)
            .ok()
            .and_then(|e| {
                if e.message.is_empty() {
                    None
                } else {
                    Some(format!("{}: {}", e.error, e.message))
                }
            })
            .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
        return Err(format!("HTTP {}: {}", status.as_u16(), msg));
    }
    serde_json::from_slice::<T>(&bytes).map_err(|e| format!("parse response: {e}"))
}

// ── JSON request/response types (api.md §3) ──────────────────────────────

#[derive(Deserialize)]
struct RegisterStartResp {
    registration_response: String,
}

#[derive(Deserialize)]
struct RegisterFinishResp {
    #[allow(dead_code)]
    status: String,
}

#[derive(Deserialize)]
struct LoginStartResp {
    login_response: String,
}

#[derive(Deserialize)]
struct LoginFinishResp {
    session_token: String,
    #[allow(dead_code)]
    expires_at: i64,
}

#[derive(Deserialize)]
struct AccountStatusResp {
    min_enc_key_gen: i64,
    svk_ciphertext_blob: String,
    #[allow(dead_code)]
    #[serde(default)]
    second_factor_required: bool,
}

// ── Result types ─────────────────────────────────────────────────────────

/// Material produced by a successful registration.
pub struct RegisterResult {
    /// Argon2id KDF salt (32 bytes). Must be persisted locally for future logins.
    pub kdf_salt: [u8; 32],
    /// 24-word BIP-39 recovery mnemonic (Emergency Kit).
    pub recovery_mnemonic: String,
}

/// Material produced by a successful login.
pub struct LoginResult {
    /// Bearer session token for authenticated API calls.
    pub session_token: String,
    /// MP-wrapped SVK blob (base64-decoded from `/account/status`).
    pub wrapped_svk: Vec<u8>,
    /// Server's minimum encryption-key generation.
    pub min_enc_key_gen: u64,
}

/// The real OPAQUE auth client for the Vautr server.
pub struct AuthClient {
    client: Client,
    base_url: String,
}

impl AuthClient {
    /// Create a client targeting `base_url` (e.g. `http://localhost:8080`).
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Register a brand-new account on the live server. Generates local key
    /// material (SVK, KEK, RK), completes the OPAQUE handshake, and persists the
    /// MP-wrapped SVK + RK-wrapped SVK on the server.
    ///
    /// Returns the KDF salt (must be persisted locally) and the 24-word recovery
    /// mnemonic (the user must store it safely).
    pub async fn register(&self, username: &str, password: &str) -> Result<RegisterResult, String> {
        // ── Local key material (crypto.md §2) ──────────────────────────
        let kdf_salt = kdf::generate_kdf_salt();
        let mk =
            kdf::derive_master_key(password, &kdf_salt).map_err(|e| format!("mk derive: {e}"))?;
        let kek = key_tree::derive_kek(&mk).map_err(|e| format!("kek derive: {e}"))?;
        let svk = key_tree::generate_svk();
        let svk_wrapped = wrap::wrap_svk(&kek, &svk);

        // Recovery Key (REQ-RECOVERY-02)
        let mnemonic =
            recovery::generate_recovery_mnemonic().map_err(|e| format!("mnemonic: {e}"))?;
        let mnemonic_bytes = recovery::decode_recovery_mnemonic(&mnemonic)
            .map_err(|e| format!("decode mnemonic: {e}"))?;
        let kek_rk =
            recovery::derive_kek_rk(&mnemonic_bytes).map_err(|e| format!("kek_rk derive: {e}"))?;
        let svk_rk_wrapped = recovery::wrap_svk_with_rk(&svk, &kek_rk, &Uuid::nil())
            .map_err(|e| format!("svk rk wrap: {e}"))?;

        // ── OPAQUE registration (api.md §3.1) ──────────────────────────
        let (cstate, creq) = state::registration_start(password);

        let start_resp: RegisterStartResp = send_json(
            self.client
                .post(format!("{}/auth/register/start", self.base_url))
                .json(&serde_json::json!({
                    "username": username,
                    "registration_start": B64.encode(&creq),
                }))
                .send()
                .await
                .map_err(|e| format!("register/start request: {e}"))?,
        )
        .await
        .map_err(|e| format!("register/start: {e}"))?;

        let sresp = B64
            .decode(&start_resp.registration_response)
            .map_err(|e| format!("register/start b64 decode: {e}"))?;
        let (upload, _export_key, _st) =
            state::registration_finish(&cstate, &sresp, password, username.as_bytes());

        let _finish: RegisterFinishResp = send_json(
            self.client
                .post(format!("{}/auth/register/finish", self.base_url))
                .json(&serde_json::json!({
                    "username": username,
                    "registration_finish": B64.encode(&upload),
                    "server_public_key": B64.encode(&[0u8; 32]),
                    "kdf_salt": B64.encode(&kdf_salt),
                    "svk_ciphertext_blob": B64.encode(&svk_wrapped),
                    "svk_ciphertext_blob_rk": B64.encode(&svk_rk_wrapped),
                }))
                .send()
                .await
                .map_err(|e| format!("register/finish request: {e}"))?,
        )
        .await
        .map_err(|e| format!("register/finish: {e}"))?;

        Ok(RegisterResult {
            kdf_salt,
            recovery_mnemonic: mnemonic,
        })
    }

    /// OPAQUE login → bearer token → fetch wrapped SVK (api.md §3.2 + §5).
    ///
    /// `kdf_salt` is the locally-persisted salt from the registration step.
    pub async fn login(
        &self,
        username: &str,
        password: &str,
        _kdf_salt: &[u8; 32],
    ) -> Result<LoginResult, String> {
        // ── OPAQUE login (api.md §3.2) ──────────────────────────────────
        let (cstate, lreq) = state::login_start(password);

        let start_resp: LoginStartResp = send_json(
            self.client
                .post(format!("{}/auth/login/start", self.base_url))
                .json(&serde_json::json!({
                    "username": username,
                    "login_start": B64.encode(&lreq),
                }))
                .send()
                .await
                .map_err(|e| format!("login/start request: {e}"))?,
        )
        .await
        .map_err(|e| format!("login/start: {e}"))?;

        let sresp = B64
            .decode(&start_resp.login_response)
            .map_err(|e| format!("login/start b64 decode: {e}"))?;
        let (upload, _session_key, _st) =
            state::login_finish(&cstate, &sresp, password, username.as_bytes());

        let finish_resp: LoginFinishResp = send_json(
            self.client
                .post(format!("{}/auth/login/finish", self.base_url))
                .json(&serde_json::json!({
                    "username": username,
                    "login_finish": B64.encode(&upload),
                }))
                .send()
                .await
                .map_err(|e| format!("login/finish request: {e}"))?,
        )
        .await
        .map_err(|e| format!("login/finish: {e}"))?;

        let token = finish_resp.session_token;

        // ── Fetch wrapped SVK (api.md §5) ──────────────────────────────
        let status: AccountStatusResp = send_json(
            self.client
                .get(format!("{}/account/status", self.base_url))
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| format!("account/status request: {e}"))?,
        )
        .await
        .map_err(|e| format!("account/status: {e}"))?;

        let wrapped_svk = B64
            .decode(&status.svk_ciphertext_blob)
            .map_err(|e| format!("account/status svk b64 decode: {e}"))?;

        Ok(LoginResult {
            session_token: token,
            wrapped_svk,
            min_enc_key_gen: status.min_enc_key_gen as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse helper: reads `VAUTR_API_URL` env or defaults to localhost.
    fn base_url() -> String {
        std::env::var("VAUTR_API_URL").unwrap_or_else(|_| "http://localhost:8080".into())
    }

    #[tokio::test]
    #[ignore = "requires live Vautr server at VAUTR_API_URL or http://localhost:8080"]
    async fn register_and_login_roundtrip() {
        let client = AuthClient::new(&base_url());
        let user = format!("e2e-test-{}", Uuid::new_v4());
        let pw = "correct horse battery staple";

        let reg = client.register(&user, pw).await.expect("registration");
        assert_eq!(reg.kdf_salt.len(), 32);
        assert!(!reg.recovery_mnemonic.is_empty());

        let login = client.login(&user, pw, &reg.kdf_salt).await.expect("login");
        assert!(!login.session_token.is_empty());
        assert!(!login.wrapped_svk.is_empty());
        assert!(login.min_enc_key_gen >= 1);
    }
}
