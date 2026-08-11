//! WebAuthn (FIDO2) optional second factor for MP unlock (VTR-052).
//!
//! When the `webauthn` server feature is enabled, a user may register one or
//! more hardware security keys / passkeys. During MP unlock, if the user has at
//! least one registered credential, the client is expected to complete a
//! WebAuthn assertion before the session is used.
//!
//! Endpoints (all Bearer-authenticated):
//!   POST   /webauthn/register/start          — begin a credential registration.
//!   POST   /webauthn/register/verify         — finish + persist the credential.
//!   POST   /webauthn/assert/start            — begin a second-factor assertion.
//!   POST   /webauthn/assert/verify           — verify the assertion (online-only).
//!   GET    /webauthn/status                  — second-factor-required signal.
//!   GET    /webauthn/credentials             — list my registered credentials.
//!   DELETE /webauthn/credentials/{cred_id}  — remove a credential (disable 2FA).
//!
//! Ceremony state (registration / authentication challenges) is kept in-memory
//! and is NOT serialised to the client, per webauthn-rs replay-prevention
//! guidance. A single-node server is assumed for this feature.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use webauthn_rs::prelude::*;
use webauthn_rs::Webauthn;

use super::{ApiError, AppState, Bearer, auth_user, now_ms};

/// In-memory, single-node WebAuthn service: the `Webauthn` instance plus the
/// pending registration / authentication ceremony states (replay-safe, server
/// side only).
pub struct WebauthnService {
    webauthn: Webauthn,
    /// token -> (user_id, label, SecurityKeyRegistration)
    regs: Mutex<HashMap<String, (String, String, SecurityKeyRegistration)>>,
    /// token -> (user_id, SecurityKeyAuthentication)
    auths: Mutex<HashMap<String, (String, SecurityKeyAuthentication)>>,
    /// Session tokens that have already satisfied the second factor this login.
    /// Used to gate `/account/status` (the wrapped-SVK fetch) until an assertion
    /// has been verified. In-memory only (single-node; not serialised).
    verified: Mutex<HashSet<String>>,
}

impl WebauthnService {
    /// Build the service. Relying-party identity is configurable via env
    /// (`VAUTR_WEBAUTHN_RP_ID`, `VAUTR_WEBAUTHN_ORIGIN`) with a localhost
    /// default suitable for development + the virtual-authenticator e2e.
    pub fn new() -> Self {
        let rp_id = std::env::var("VAUTR_WEBAUTHN_RP_ID").unwrap_or_else(|_| "localhost".into());
        let origin = std::env::var("VAUTR_WEBAUTHN_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:3000".into());
        let rp_origin = Url::parse(&origin)
            .unwrap_or_else(|_| Url::parse("http://localhost:3000").expect("static url"));
        let webauthn = WebauthnBuilder::new(&rp_id, &rp_origin)
            .expect("invalid WebAuthn configuration")
            .rp_name("Vautr")
            .build()
            .expect("invalid WebAuthn configuration");
        Self {
            webauthn,
            regs: Mutex::new(HashMap::new()),
            auths: Mutex::new(HashMap::new()),
            verified: Mutex::new(HashSet::new()),
        }
    }

    /// The relying party id (used by tests / the e2e harness).
    pub fn rp_id(&self) -> String {
        // Re-read from env to keep it stable with the browser's expected rp id.
        std::env::var("VAUTR_WEBAUTHN_RP_ID").unwrap_or_else(|_| "localhost".into())
    }

    /// Record that a session token has satisfied the second factor.
    pub fn mark_second_factor_satisfied(&self, session_token: &str) {
        if let Ok(mut verified) = self.verified.lock() {
            verified.insert(session_token.to_string());
        }
    }

    /// Whether a session token has already satisfied the second factor.
    pub fn is_second_factor_satisfied(&self, session_token: &str) -> bool {
        self.verified
            .lock()
            .map(|v| v.contains(session_token))
            .unwrap_or(false)
    }

    /// Invalidate the 2FA-satisfied flag for a session (e.g. after the user
    /// removes their last credential, or the session is revoked).
    pub fn clear_second_factor_satisfied(&self, session_token: &str) {
        if let Ok(mut verified) = self.verified.lock() {
            verified.remove(session_token);
        }
    }
}

impl Default for WebauthnService {
    fn default() -> Self {
        Self::new()
    }
}

/// Build this feature's router (merged into the main router in mod.rs).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/webauthn/register/start", post(register_start))
        .route("/webauthn/register/verify", post(register_verify))
        .route("/webauthn/assert/start", post(assert_start))
        .route("/webauthn/assert/verify", post(assert_verify))
        .route("/webauthn/status", get(webauthn_status))
        .route("/webauthn/credentials", get(list_credentials))
        .route("/webauthn/credentials/{cred_id}", delete(remove_credential))
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct RegisterStartReq {
    /// Human friendly device label (e.g. "YubiKey 5 NFC").
    label: String,
}
#[derive(Serialize)]
pub(crate) struct RegisterStartResp {
    /// Opaque token used to pair this ceremony with `/register/verify`.
    request_id: String,
    /// `PublicKeyCredentialCreationOptions` (camelCase) for navigator.credentials.create.
    challenge: Value,
}

#[derive(Deserialize)]
pub(crate) struct RegisterVerifyReq {
    request_id: String,
    /// The `PublicKeyCredential` returned by navigator.credentials.create.
    credential: Value,
}
#[derive(Serialize)]
pub(crate) struct StatusResp {
    status: String,
}

#[derive(Serialize)]
pub(crate) struct AssertStartResp {
    request_id: String,
    /// `PublicKeyCredentialRequestOptions` (camelCase) for navigator.credentials.get.
    challenge: Value,
}

#[derive(Deserialize)]
pub(crate) struct AssertVerifyReq {
    request_id: String,
    /// The `PublicKeyCredential` returned by navigator.credentials.get.
    credential: Value,
}
#[derive(Serialize)]
pub(crate) struct AssertVerifyResp {
    status: String,
    user_id: String,
}

#[derive(Serialize)]
pub(crate) struct StatusResp2fa {
    second_factor_required: bool,
    credentials: Vec<CredentialSummary>,
}
#[derive(Serialize)]
pub(crate) struct CredentialSummary {
    cred_id: String,
    label: String,
}
#[derive(Serialize)]
pub(crate) struct ListResp {
    credentials: Vec<CredentialSummary>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::internal(&e.to_string())
}

fn b64url(b: &[u8]) -> String {
    B64URL.encode(b)
}

/// Begin a security-key registration ceremony.
pub(crate) async fn register_start(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<RegisterStartReq>,
) -> Result<Json<RegisterStartResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;

    // Exclude already-registered credentials to prevent duplicates.
    let existing = st.repo.list_webauthn_credentials(&user_id).await.map_err(internal)?;
    let exclude: Option<Vec<CredentialID>> = if existing.is_empty() {
        None
    } else {
        let mut ids = Vec::with_capacity(existing.len());
        for row in &existing {
            if let Ok(sk) = serde_json::from_slice::<SecurityKey>(&row.serialized) {
                ids.push(sk.cred_id().clone());
            }
        }
        Some(ids)
    };

    let user_uuid = uuid::Uuid::parse_str(&user_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
    let (ccr, reg_state) = st
        .webauthn
        .webauthn
        .start_securitykey_registration(user_uuid, &user_id, &user_id, exclude, None, None)
        .map_err(internal)?;

    let request_id = uuid::Uuid::new_v4().to_string();
    st.webauthn
        .regs
        .lock()
        .map_err(|_| ApiError::internal("ceremony lock"))?
        .insert(request_id.clone(), (user_id, req.label, reg_state));

    Ok(Json(RegisterStartResp {
        request_id,
        challenge: serde_json::to_value(ccr).map_err(internal)?,
    }))
}

/// Complete a security-key registration and persist the credential.
pub(crate) async fn register_verify(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<RegisterVerifyReq>,
) -> Result<Json<StatusResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;

    let (cer_user, label, reg_state) = st
        .webauthn
        .regs
        .lock()
        .map_err(|_| ApiError::internal("ceremony lock"))?
        .remove(&req.request_id)
        .ok_or_else(|| ApiError::bad_request("no_active_registration", "unknown or expired registration"))?;
    if cer_user != user_id {
        return Err(ApiError::unauthorized());
    }

    let pkc: RegisterPublicKeyCredential = serde_json::from_value(req.credential)
        .map_err(|e| ApiError::bad_request("invalid_credential", &e.to_string()))?;
    let sk = st
        .webauthn
        .webauthn
        .finish_securitykey_registration(&pkc, &reg_state)
        .map_err(|e| ApiError::new(http_code_for(&e), "registration_failed", &e.to_string()))?;

    let cred_id = b64url(sk.cred_id().as_ref());
    let serialized = serde_json::to_vec(&sk).map_err(internal)?;
    st.repo
        .add_webauthn_credential(&user_id, &cred_id, &label, &serialized, 0, now_ms())
        .await
        .map_err(internal)?;

    Ok(Json(StatusResp {
        status: "success".into(),
    }))
}

/// Begin a second-factor assertion (online-only: the server must verify).
pub(crate) async fn assert_start(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<AssertStartResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;

    let rows = st.repo.list_webauthn_credentials(&user_id).await.map_err(internal)?;
    if rows.is_empty() {
        return Err(ApiError::bad_request(
            "no_second_factor",
            "no registered security key for this account",
        ));
    }
    let mut creds = Vec::with_capacity(rows.len());
    for row in &rows {
        let sk: SecurityKey =
            serde_json::from_slice(&row.serialized).map_err(|e| ApiError::internal(&e.to_string()))?;
        creds.push(sk);
    }

    let (rcr, auth_state) = st
        .webauthn
        .webauthn
        .start_securitykey_authentication(&creds)
        .map_err(internal)?;

    let request_id = uuid::Uuid::new_v4().to_string();
    st.webauthn
        .auths
        .lock()
        .map_err(|_| ApiError::internal("ceremony lock"))?
        .insert(request_id.clone(), (user_id, auth_state));

    Ok(Json(AssertStartResp {
        request_id,
        challenge: serde_json::to_value(rcr).map_err(internal)?,
    }))
}

/// Verify a second-factor assertion and update the credential counter.
pub(crate) async fn assert_verify(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<AssertVerifyReq>,
) -> Result<Json<AssertVerifyResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;

    let (cer_user, auth_state) = st
        .webauthn
        .auths
        .lock()
        .map_err(|_| ApiError::internal("ceremony lock"))?
        .remove(&req.request_id)
        .ok_or_else(|| ApiError::bad_request("no_active_assertion", "unknown or expired assertion"))?;
    if cer_user != user_id {
        return Err(ApiError::unauthorized());
    }

    let pkc: PublicKeyCredential = serde_json::from_value(req.credential)
        .map_err(|e| ApiError::bad_request("invalid_credential", &e.to_string()))?;
    let res = st
        .webauthn
        .webauthn
        .finish_securitykey_authentication(&pkc, &auth_state)
        .map_err(|e| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "second_factor_failed",
                &e.to_string(),
            )
        })?;

    // The session has now satisfied the second factor for this login; this
    // un-gates `/account/status` so the wrapped SVK can be fetched.
    st.webauthn.mark_second_factor_satisfied(&auth.0);

    // Persist the counter for cloned-credential detection.
    let cred_id = b64url(res.cred_id().as_ref());
    let now = now_ms();
    if let Some((serialized, _old_counter)) = st
        .repo
        .get_webauthn_credential(&user_id, &cred_id)
        .await
        .map_err(internal)?
    {
        if let Ok(mut sk) = serde_json::from_slice::<SecurityKey>(&serialized) {
            sk.update_credential(&res);
            let new_serialized = serde_json::to_vec(&sk).map_err(internal)?;
            st.repo
                .update_webauthn_counter(&user_id, &cred_id, res.counter() as i64, &new_serialized, now)
                .await
                .map_err(internal)?;
        }
    }

    Ok(Json(AssertVerifyResp {
        status: "success".into(),
        user_id,
    }))
}

/// Second-factor-required signal used after MP verification: true when the
/// user has at least one registered credential.
pub(crate) async fn webauthn_status(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<StatusResp2fa>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let rows = st.repo.list_webauthn_credentials(&user_id).await.map_err(internal)?;
    let credentials = rows
        .iter()
        .map(|r| CredentialSummary {
            cred_id: r.cred_id.clone(),
            label: r.label.clone(),
        })
        .collect();
    Ok(Json(StatusResp2fa {
        second_factor_required: !rows.is_empty(),
        credentials,
    }))
}

/// List the current user's registered credentials.
pub(crate) async fn list_credentials(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<ListResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let rows = st.repo.list_webauthn_credentials(&user_id).await.map_err(internal)?;
    Ok(Json(ListResp {
        credentials: rows
            .iter()
            .map(|r| CredentialSummary {
                cred_id: r.cred_id.clone(),
                label: r.label.clone(),
            })
            .collect(),
    }))
}

/// Remove a credential (disables the second factor for that key).
pub(crate) async fn remove_credential(
    State(st): State<AppState>,
    auth: Bearer,
    Path(cred_id): Path<String>,
) -> Result<Json<StatusResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let removed = st
        .repo
        .remove_webauthn_credential(&user_id, &cred_id)
        .await
        .map_err(internal)?;
    if !removed {
        return Err(ApiError::bad_request("not_found", "credential not found"));
    }
    Ok(Json(StatusResp {
        status: "success".into(),
    }))
}

/// Map a webauthn-rs error to an appropriate HTTP status.
fn http_code_for(_e: &WebauthnError) -> StatusCode {
    StatusCode::BAD_REQUEST
}

// ---------------------------------------------------------------------------
// Round-trip tests (VTR-052 TDD). Run with: cargo test -p vautr-server
// --features webauthn. These build raw WebAuthn payloads with a P-256 key so
// the handlers exercise the same ctap parsing + signature verification a real
// browser (or the CDP virtual authenticator) would.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::bn::BigNumContext;
    use openssl::ec::{EcGroup, EcKey, PointConversionForm};
    use openssl::hash::MessageDigest;
    use openssl::nid::Nid;
    use openssl::pkey::PKey;
    use openssl::sign::Signer;
    use sha2::{Digest, Sha256};
    use std::sync::Arc;

    const RP_ID: &str = "localhost";
    const ORIGIN: &str = "http://localhost:3000";

    // --- tiny CBOR encoder (enough for a COSE EC2 key + attestation object) ---

    fn head(major: u8, len: u64) -> Vec<u8> {
        let mut out = Vec::new();
        if len < 24 {
            out.push(major | len as u8);
        } else if len < 0x100 {
            out.push(major | 0x18);
            out.push(len as u8);
        } else if len < 0x10000 {
            out.push(major | 0x19);
            out.extend_from_slice(&(len as u16).to_be_bytes());
        } else if len < 0x1_0000_0000 {
            out.push(major | 0x1a);
            out.extend_from_slice(&(len as u32).to_be_bytes());
        } else {
            out.push(major | 0x1b);
            out.extend_from_slice(&len.to_be_bytes());
        }
        out
    }

    fn cbor_uint(v: i64) -> Vec<u8> {
        head(0x00, v as u64)
    }

    fn cbor_int(v: i64) -> Vec<u8> {
        if v >= 0 {
            cbor_uint(v)
        } else {
            // Negative integers encode -1-n.
            head(0x20, (-1 - v) as u64)
        }
    }

    fn cbor_bstr(b: &[u8]) -> Vec<u8> {
        let mut out = head(0x40, b.len() as u64);
        out.extend_from_slice(b);
        out
    }

    fn cbor_tstr(s: &str) -> Vec<u8> {
        cbor_bstr(s.as_bytes())
    }

    fn cbor_map(entries: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
        let mut out = head(0xa0, entries.len() as u64);
        for (k, v) in entries {
            out.extend_from_slice(&k);
            out.extend_from_slice(&v);
        }
        out
    }

    fn sha256(b: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b);
        let d = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&d);
        out
    }

    /// COSE_Key for an EC2 P-256 (kty=2, alg=-7, crv=1) public key.
    fn cose_ec2(x: &[u8], y: &[u8]) -> Vec<u8> {
        cbor_map(vec![
            (cbor_uint(1), cbor_uint(2)),   // kty: EC2
            (cbor_uint(3), cbor_int(-7)),   // alg: ES256
            (cbor_int(-1), cbor_uint(1)),   // crv: P-256
            (cbor_int(-2), cbor_bstr(x)),   // x
            (cbor_int(-3), cbor_bstr(y)),   // y
        ])
    }

    fn client_data(challenge_raw: &[u8], op: &str) -> Vec<u8> {
        format!(
            r#"{{"type":"{}","challenge":"{}","origin":"{}","crossOrigin":false}}"#,
            op,
            B64URL.encode(challenge_raw),
            ORIGIN
        )
        .into_bytes()
    }

    /// Registration authenticator data with AT|UV|UP flags and an ACD.
    fn reg_auth_data(x: &[u8], y: &[u8], cred_id: &[u8]) -> Vec<u8> {
        let rp_hash = sha256(RP_ID.as_bytes());
        let mut ad = Vec::new();
        ad.extend_from_slice(&rp_hash);
        ad.push(0x45); // AT | UV | UP
        ad.extend_from_slice(&0u32.to_be_bytes()); // signCount = 0
        ad.extend_from_slice(&[0u8; 16]); // aaguid (all zero)
        ad.extend_from_slice(&(cred_id.len() as u16).to_be_bytes());
        ad.extend_from_slice(cred_id);
        ad.extend_from_slice(&cose_ec2(x, y));
        ad
    }

    /// Assertion authenticator data with UV|UP flags and a running counter.
    fn assert_auth_data(counter: u32) -> Vec<u8> {
        let rp_hash = sha256(RP_ID.as_bytes());
        let mut ad = Vec::new();
        ad.extend_from_slice(&rp_hash);
        ad.push(0x05); // UV | UP (no AT)
        ad.extend_from_slice(&counter.to_be_bytes());
        ad
    }

    /// Attestation object CBOR with fmt = "none".
    fn attestation_object(auth_data: &[u8]) -> Vec<u8> {
        cbor_map(vec![
            (cbor_tstr("fmt"), cbor_tstr("none")),
            (cbor_tstr("attStmt"), vec![0xa0]), // empty map
            (cbor_tstr("authData"), cbor_bstr(auth_data)),
        ])
    }

    /// An in-memory P-256 keypair + its x/y coordinates.
    struct TestKey {
        x: Vec<u8>,
        y: Vec<u8>,
        pkey: PKey<openssl::pkey::Private>,
    }

    fn gen_key() -> TestKey {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let ec = EcKey::generate(&group).unwrap();
        let point = ec
            .public_key()
            .to_bytes(&group, PointConversionForm::UNCOMPRESSED, &mut ctx)
            .unwrap();
        // point = 0x04 || x(32) || y(32)
        TestKey {
            x: point[1..33].to_vec(),
            y: point[33..65].to_vec(),
            pkey: PKey::from_ec_key(ec).unwrap(),
        }
    }

    fn sign(pkey: &PKey<openssl::pkey::Private>, auth_data: &[u8], client_data: &[u8]) -> Vec<u8> {
        let digest = sha256(client_data);
        let mut msg = Vec::with_capacity(auth_data.len() + 32);
        msg.extend_from_slice(auth_data);
        msg.extend_from_slice(&digest);
        let mut signer = Signer::new(MessageDigest::sha256(), pkey).unwrap();
        signer.sign_oneshot_to_vec(&msg).unwrap() // DER ECDSA
    }

    async fn test_state() -> AppState {
        // Force the same RP identity the handler's WebauthnService::new() uses.
        std::env::set_var("VAUTR_WEBAUTHN_RP_ID", RP_ID);
        std::env::set_var("VAUTR_WEBAUTHN_ORIGIN", ORIGIN);
        let path =
            std::env::temp_dir().join(format!("vautr_wbauthn_test_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(crate::repository::Repository::new(pool));
        let now = 1_700_000_000_000i64;
        repo.create_user("u1", "a@b.c", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES ('tok1', 'u1', ?, ?)",
        )
        .bind(4_000_000_000_000i64)
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        AppState::new(repo)
    }

    /// Register a fresh credential, returning (st, cred_id, key).
    async fn register_credential(
        st: &AppState,
        key: &TestKey,
        cred_id: &[u8],
    ) -> (String, String) {
        let start = register_start(
            State(st.clone()),
            Bearer("tok1".into()),
            Json(RegisterStartReq {
                label: "Test YubiKey".into(),
            }),
        )
        .await
        .unwrap();
        let start = start.0;
        let request_id = start.request_id;
        let challenge_raw = B64URL
            .decode(
                start.challenge["publicKey"]["challenge"]
                    .as_str()
                    .expect("challenge b64url"),
            )
            .expect("decode challenge");

        let cd = client_data(&challenge_raw, "webauthn.create");
        let auth_data = reg_auth_data(&key.x, &key.y, cred_id);
        let ao = attestation_object(&auth_data);
        let credential = serde_json::json!({
            "id": b64url(cred_id),
            "rawId": b64url(cred_id),
            "type": "public-key",
            "response": {
                "attestationObject": b64url(&ao),
                "clientDataJSON": b64url(&cd),
            },
        });
        let verify = register_verify(
            State(st.clone()),
            Bearer("tok1".into()),
            Json(RegisterVerifyReq {
                request_id,
                credential,
            }),
        )
        .await
        .expect("register_verify should succeed");
        (verify.0.status, b64url(cred_id))
    }

    /// Build a valid assertion response for the registered credential.
    async fn make_assertion(st: &AppState, key: &TestKey, cred_id: &[u8], counter: u32) -> Value {
        let start = assert_start(State(st.clone()), Bearer("tok1".into()))
            .await
            .unwrap();
        let start = start.0;
        let request_id = start.request_id;
        let challenge_raw = B64URL
            .decode(
                start.challenge["publicKey"]["challenge"]
                    .as_str()
                    .expect("challenge b64url"),
            )
            .expect("decode challenge");

        let cd = client_data(&challenge_raw, "webauthn.get");
        let ad = assert_auth_data(counter);
        let sig = sign(&key.pkey, &ad, &cd);
        let credential = serde_json::json!({
            "id": b64url(cred_id),
            "rawId": b64url(cred_id),
            "type": "public-key",
            "response": {
                "authenticatorData": b64url(&ad),
                "clientDataJSON": b64url(&cd),
                "signature": b64url(&sig),
            },
        });
        Value::Object(serde_json::Map::from_iter(vec![
            ("request_id".to_string(), Value::String(request_id)),
            ("credential".to_string(), credential),
        ]))
    }

    /// Returns `(second_factor_required, has_svk_blob)` from `/account/status`.
    async fn account_status_gated(st: &AppState) -> (bool, bool) {
        let resp = crate::handlers::account::account_status(State(st.clone()), Bearer("tok1".into()))
            .await
            .unwrap();
        let v = serde_json::to_value(resp.0).unwrap();
        let required = v
            .get("second_factor_required")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let svk = v
            .get("svk_ciphertext_blob")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        (required, !svk.is_empty())
    }

    #[tokio::test]
    async fn round_trip_register_then_assert_ungates_status() {
        let st = test_state().await;
        let key = gen_key();
        let cred_id = [0x33; 32];

        // Before any credential: account status is ungated and returns the SVK.
        let (req, has_svk) = account_status_gated(&st).await;
        assert!(!req);
        assert!(has_svk);

        // Register.
        let (status, _) = register_credential(&st, &key, &cred_id).await;
        assert_eq!(status, "success");

        // A registered credential now gates /account/status (SVK withheld).
        let (req, has_svk) = account_status_gated(&st).await;
        assert!(req);
        assert!(!has_svk);

        // Start + verify an assertion with counter = 1 (> stored 0).
        let body = make_assertion(&st, &key, &cred_id, 1).await;
        let request_id = body["request_id"].as_str().unwrap().to_string();
        let credential = body["credential"].clone();
        let resp = assert_verify(
            State(st.clone()),
            Bearer("tok1".into()),
            Json(AssertVerifyReq { request_id, credential }),
        )
        .await
        .expect("assert_verify should succeed");
        assert_eq!(resp.0.status, "success");
        assert_eq!(resp.0.user_id, "u1");

        // Session is now 2FA-satisfied: status is ungated again.
        let (req, has_svk) = account_status_gated(&st).await;
        assert!(!req);
        assert!(has_svk);

        // The stored counter advanced.
        let (_, counter) = st
            .repo
            .get_webauthn_credential("u1", &b64url(&cred_id))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(counter, 1);
    }

    #[tokio::test]
    async fn bad_signature_fails_assertion() {
        let st = test_state().await;
        let key = gen_key();
        let other = gen_key();
        let cred_id = [0x44; 32];
        register_credential(&st, &key, &cred_id).await;

        // Sign with a DIFFERENT key than the registered one.
        let body = make_assertion(&st, &other, &cred_id, 1).await;
        let request_id = body["request_id"].as_str().unwrap().to_string();
        let credential = body["credential"].clone();
        let res = assert_verify(
            State(st.clone()),
            Bearer("tok1".into()),
            Json(AssertVerifyReq { request_id, credential }),
        )
        .await;
        assert!(res.is_err(), "assert_verify must fail with a bad signature");
        let err = res.err().unwrap();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);

        // Status stays gated: second factor was not satisfied.
        let (req, has_svk) = account_status_gated(&st).await;
        assert!(req);
        assert!(!has_svk);
    }

    #[tokio::test]
    async fn remove_credential_disables_second_factor() {
        let st = test_state().await;
        let key = gen_key();
        let cred_id = [0x55; 32];
        register_credential(&st, &key, &cred_id).await;

        let (req, _) = account_status_gated(&st).await;
        assert!(req);

        let cred_id_b64 = b64url(&cred_id);
        let resp = remove_credential(
            State(st.clone()),
            Bearer("tok1".into()),
            Path(cred_id_b64.clone()),
        )
        .await
        .unwrap();
        assert_eq!(resp.0.status, "success");

        let (req, has_svk) = account_status_gated(&st).await;
        assert!(!req);
        assert!(has_svk);
    }

    #[tokio::test]
    async fn multiple_credentials_supported() {
        let st = test_state().await;
        let key1 = gen_key();
        let key2 = gen_key();
        let id1 = [0x66; 32];
        let id2 = [0x77; 32];

        register_credential(&st, &key1, &id1).await;
        register_credential(&st, &key2, &id2).await;

        let rows = st.repo.list_webauthn_credentials("u1").await.unwrap();
        assert_eq!(rows.len(), 2);

        // Assertion works with the backup (second) key.
        let body = make_assertion(&st, &key2, &id2, 1).await;
        let request_id = body["request_id"].as_str().unwrap().to_string();
        let credential = body["credential"].clone();
        let resp = assert_verify(
            State(st.clone()),
            Bearer("tok1".into()),
            Json(AssertVerifyReq { request_id, credential }),
        )
        .await
        .expect("backup key assertion succeeds");
        assert_eq!(resp.0.status, "success");

        let (req, has_svk) = account_status_gated(&st).await;
        assert!(!req);
        assert!(has_svk);
    }
}

