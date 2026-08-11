//! MFA HTTP handlers (Wave A: group A4). Docs: `mlp-wave-plan.md` §3 A4,
//! `mlp-scope.md` §3 (mandatory MFA) / §5 (policies).
//!
//! Implements:
//!   POST /mfa/totp/issue    — issue a TOTP enrollment (secret + otpauth URI + QR).
//!   POST /mfa/totp/verify   — verify a TOTP code (complete enrollment / login).
//!   GET  /mfa/status        — the caller's MFA configuration (required, methods).
//!   GET  /mfa/policy        — organization MFA + master-password policy.
//!   PUT  /mfa/policy        — update the organization policy.
//!
//! Enforcement: `enforce_mfa_required` is invoked from the auth flow (auth.rs
//! login_finish). When the org policy makes MFA mandatory and the caller has no
//! configured method, login is rejected with 403 `mfa_required`. The server
//! never sees the master password (OPAQUE), so the master-password policy is
//! stored + served and validated for sane ranges, and `password_satisfies`
//! provides the client-side evaluation logic.

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, TOTP};

use super::{ApiError, AppState, Bearer, auth_user, now_ms};
use crate::repository::mfa::{MasterPasswordPolicy, MfaPolicy};

/// TOTP enrollment lifetime before it expires (15 minutes).
const ENROLLMENT_TTL_MS: i64 = 15 * 60_000;
/// TOTP window (step tolerance): skew=1 accepts one step before and after.
const TOTP_SKEW: u8 = 1;
/// Recovery-code alphabet (no 0/O/1/I to avoid transcription errors).
const RECOVERY_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::internal(&e.to_string())
}

/// Build this feature's router (merged into the main router in mod.rs).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mfa/status", get(mfa_status))
        .route("/mfa/totp/issue", post(issue_totp))
        .route("/mfa/totp/verify", post(verify_totp))
        .route("/mfa/policy", get(policy_get))
        .route("/mfa/policy", put(policy_put))
}

// ---------------------------------------------------------------------------
// Request / response types (field names mirror packages/api-contract/openapi.json)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct MfaStatusResp {
    required: bool,
    configured_methods: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct TotpIssueResp {
    enrollment_id: String,
    otpauth_url: String,
    secret: String,
    qr_code_data_url: String,
}

#[derive(Deserialize)]
pub(crate) struct TotpVerifyReq {
    enrollment_id: Option<String>,
    code: String,
}

#[derive(Serialize)]
pub(crate) struct TotpVerifyResp {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_codes: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub(crate) struct MfaPolicyUpdateReq {
    #[serde(default)]
    required: bool,
    #[serde(default)]
    allowed_methods: Vec<String>,
    #[serde(default)]
    master_password_policy: MasterPasswordPolicy,
}

// ---------------------------------------------------------------------------
// TOTP / QR / helpers
// ---------------------------------------------------------------------------

/// The configured MFA methods for a user, in contract order (totp, webauthn).
async fn configured_methods(st: &AppState, user_id: &str) -> Result<Vec<String>, ApiError> {
    let mut methods = Vec::new();
    if st.repo.mfa_has_totp(user_id).await.map_err(internal)? {
        methods.push("totp".to_string());
    }
    #[cfg(feature = "webauthn")]
    {
        if st
            .repo
            .webauthn_has_credentials(user_id)
            .await
            .map_err(internal)?
        {
            methods.push("webauthn".to_string());
        }
    }
    Ok(methods)
}

/// MFA enforcement hook for the auth flow. When the org policy makes MFA
/// mandatory and the user has no configured method, returns a 403 `mfa_required`
/// so the caller is rejected at login.
pub(crate) async fn enforce_mfa_required(
    st: &AppState,
    user_id: &str,
) -> Result<(), ApiError> {
    let policy = st.repo.mfa_get_policy().await.map_err(internal)?;
    if !policy.required {
        return Ok(());
    }
    let methods = configured_methods(st, user_id).await?;
    if methods.is_empty() {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "mfa_required",
            "MFA is mandatory for this organization and is not configured for this account",
        ));
    }
    Ok(())
}

/// RFC3986 percent-encode a string for embedding in an otpauth:// URI label.
fn percent_encode(s: &str) -> String {
    const UNRESERVED: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        if UNRESERVED.contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Build a `totp-rs` verifier from raw secret bytes with the standard profile.
fn totp_verifier(secret: &[u8]) -> Result<TOTP, ApiError> {
    TOTP::new(Algorithm::SHA1, 6, TOTP_SKEW, 30, secret.to_vec())
        .map_err(|e| internal(format!("invalid totp secret: {e}")))
}

/// Generate a random recovery code of the form "XXXX-XXXX".
fn gen_recovery_code() -> String {
    let block = |rng: &mut rand::rngs::OsRng| -> String {
        let mut buf = [0u8; 4];
        rng.fill_bytes(&mut buf);
        let mut out = String::with_capacity(4);
        for b in buf {
            out.push(RECOVERY_ALPHABET[(b as usize) % RECOVERY_ALPHABET.len()] as char);
        }
        out
    };
    let mut rng = rand::rngs::OsRng;
    format!("{}-{}", block(&mut rng), block(&mut rng))
}

// ---------------------------------------------------------------------------
// Master-password policy evaluation (client-side / advisory, zero-knowledge)
// ---------------------------------------------------------------------------

/// Approximate entropy (bits) of a password from its character-class pool size.
#[allow(dead_code)] // exercised by unit tests; client-side enforcement helper
pub(crate) fn entropy_bits(pw: &str) -> f64 {
    let n = pw.chars().count() as f64;
    if n == 0.0 {
        return 0.0;
    }
    let bytes = pw.as_bytes();
    let mut pool = 0u32;
    if bytes.iter().any(|b| b.is_ascii_lowercase()) {
        pool += 26;
    }
    if bytes.iter().any(|b| b.is_ascii_uppercase()) {
        pool += 26;
    }
    if bytes.iter().any(|b| b.is_ascii_digit()) {
        pool += 10;
    }
    if bytes.iter().any(|b| !b.is_ascii_alphanumeric()) {
        pool += 33;
    }
    if pool == 0 {
        pool = 1;
    }
    n * (pool as f64).log2()
}

/// Whether a password satisfies the given master-password policy.
#[allow(dead_code)] // exercised by unit tests; client-side enforcement helper
pub(crate) fn password_satisfies(policy: &MasterPasswordPolicy, pw: &str) -> bool {
    if pw.chars().count() < policy.min_length as usize {
        return false;
    }
    let bytes = pw.as_bytes();
    if policy.require_upper && !bytes.iter().any(|b| b.is_ascii_uppercase()) {
        return false;
    }
    if policy.require_lower && !bytes.iter().any(|b| b.is_ascii_lowercase()) {
        return false;
    }
    if policy.require_digit && !bytes.iter().any(|b| b.is_ascii_digit()) {
        return false;
    }
    if policy.require_special && !bytes.iter().any(|b| !b.is_ascii_alphanumeric()) {
        return false;
    }
    entropy_bits(pw) >= policy.min_entropy_bits as f64
}

/// Validate a policy update before persisting it.
fn validate_policy(req: &MfaPolicyUpdateReq) -> Result<(), ApiError> {
    if req.allowed_methods.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_policy",
            "at least one allowed MFA method is required",
        ));
    }
    for m in &req.allowed_methods {
        if !matches!(m.as_str(), "totp" | "webauthn" | "email") {
            return Err(ApiError::bad_request(
                "invalid_policy",
                &format!("unknown MFA method: {m}"),
            ));
        }
    }
    let mp = &req.master_password_policy;
    if mp.min_length < 8 || mp.min_length > 256 {
        return Err(ApiError::bad_request(
            "invalid_policy",
            "master_password_policy.min_length must be 8..=256",
        ));
    }
    if mp.min_entropy_bits > 512 {
        return Err(ApiError::bad_request(
            "invalid_policy",
            "master_password_policy.min_entropy_bits must be 0..=512",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /mfa/status — the caller's MFA configuration.
pub(crate) async fn mfa_status(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<MfaStatusResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let policy = st.repo.mfa_get_policy().await.map_err(internal)?;
    let methods = configured_methods(&st, &user_id).await?;
    Ok(Json(MfaStatusResp {
        required: policy.required,
        configured_methods: methods,
    }))
}

/// POST /mfa/totp/issue — begin TOTP enrollment for the caller.
pub(crate) async fn issue_totp(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<TotpIssueResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;
    let user = st
        .repo
        .get_user_by_id(&user_id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ApiError::unauthorized())?;

    // 160-bit CSPRNG secret (RFC 6238 recommends >= 160 bits).
    let mut secret = [0u8; 20];
    rand::rngs::OsRng.fill_bytes(&mut secret);
    let secret_base32 = base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &secret);

    let label = percent_encode(&format!("Vautr:{}", user.email));
    let otpauth_url = format!(
        "otpauth://totp/{label}?secret={secret_base32}&issuer=Vautr&algorithm=SHA1&digits=6&period=30"
    );

    // Render the provisioning URI as an SVG QR, embedded as a data URL.
    let code = qrcode::QrCode::new(otpauth_url.as_bytes())
        .map_err(|e| internal(format!("qr generation failed: {e}")))?;
    let svg_xml = code.render::<qrcode::render::svg::Color>().build();
    let qr_code_data_url = format!(
        "data:image/svg+xml;base64,{}",
        B64.encode(svg_xml.as_bytes())
    );

    let enrollment_id = uuid::Uuid::new_v4().to_string();
    let now = now_ms();
    st.repo
        .clear_totp_enrollments(&user_id)
        .await
        .map_err(internal)?;
    st.repo
        .create_totp_enrollment(
            &enrollment_id,
            &user_id,
            &secret,
            &secret_base32,
            now,
            now + ENROLLMENT_TTL_MS,
        )
        .await
        .map_err(internal)?;

    Ok(Json(TotpIssueResp {
        enrollment_id,
        otpauth_url,
        secret: secret_base32,
        qr_code_data_url,
    }))
}

/// POST /mfa/totp/verify — verify a code.
///
/// With `enrollment_id` (from /mfa/totp/issue): completes enrollment, activating
/// the secret and (on first enrollment) issuing recovery codes. Without it:
/// verifies the caller's currently active secret (defense-in-depth during login).
pub(crate) async fn verify_totp(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<TotpVerifyReq>,
) -> Result<Json<TotpVerifyResp>, ApiError> {
    let user_id = auth_user(&st.repo, &auth.0).await?;

    match req.enrollment_id {
        Some(enrollment_id) => {
            let row = st
                .repo
                .get_totp_enrollment(&enrollment_id)
                .await
                .map_err(internal)?
                .ok_or_else(|| {
                    ApiError::bad_request("unknown_enrollment", "unknown or expired enrollment")
                })?;
            if row.user_id != user_id {
                return Err(ApiError::unauthorized());
            }
            if now_ms() > row.expires_at {
                st.repo.delete_totp_enrollment(&enrollment_id).await.map_err(internal)?;
                return Err(ApiError::bad_request(
                    "unknown_enrollment",
                    "enrollment has expired; request a new one",
                ));
            }
            let totp = totp_verifier(&row.secret)?;
            if !totp.check_current(&req.code).map_err(internal)? {
                return Err(ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "invalid_totp_code",
                    "invalid one-time code",
                ));
            }

            let had_totp = st.repo.mfa_has_totp(&user_id).await.map_err(internal)?;
            let now = now_ms();
            st.repo
                .activate_totp_secret(&user_id, &row.secret, &row.secret_base32, now)
                .await
                .map_err(internal)?;

            // Issue recovery codes once, on first TOTP enrollment.
            let recovery_codes = if had_totp {
                None
            } else {
                let codes: Vec<String> = (0..5).map(|_| gen_recovery_code()).collect();
                st.repo
                    .insert_recovery_codes(&user_id, &codes, now)
                    .await
                    .map_err(internal)?;
                Some(codes)
            };

            st.repo
                .audit_org_event(
                    Some(&user_id),
                    Some(&user_id),
                    "mfa_totp_enroll",
                    "mfa",
                    None,
                    None,
                    None,
                    now,
                )
                .await
                .map_err(internal)?;

            Ok(Json(TotpVerifyResp {
                status: "success".into(),
                recovery_codes,
            }))
        }
        None => {
            // Login-time verification against the active secret.
            let Some((secret, _)) = st.repo.get_totp_secret(&user_id).await.map_err(internal)?
            else {
                return Err(ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "no_totp_configured",
                    "TOTP is not configured for this account",
                ));
            };
            let totp = totp_verifier(&secret)?;
            if !totp.check_current(&req.code).map_err(internal)? {
                return Err(ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "invalid_totp_code",
                    "invalid one-time code",
                ));
            }
            Ok(Json(TotpVerifyResp {
                status: "success".into(),
                recovery_codes: None,
            }))
        }
    }
}

/// GET /mfa/policy — the organization MFA + password policy.
pub(crate) async fn policy_get(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<MfaPolicy>, ApiError> {
    let _ = auth_user(&st.repo, &auth.0).await?;
    let policy = st.repo.mfa_get_policy().await.map_err(internal)?;
    Ok(Json(policy))
}

/// PUT /mfa/policy — update the organization MFA + password policy.
pub(crate) async fn policy_put(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<MfaPolicyUpdateReq>,
) -> Result<Json<MfaPolicy>, ApiError> {
    let _ = auth_user(&st.repo, &auth.0).await?;
    validate_policy(&req)?;
    let policy = MfaPolicy {
        required: req.required,
        allowed_methods: req.allowed_methods,
        master_password_policy: req.master_password_policy,
    };
    st.repo.mfa_set_policy(&policy, now_ms()).await.map_err(internal)?;
    Ok(Json(policy))
}

// ---------------------------------------------------------------------------
// Tests (Wave A4 TDD). Run with: cargo test -p vautr-server
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::build_router;
    use crate::repository::Repository;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use serde_json::{json, Value};
    use std::sync::Arc;
    use tower::ServiceExt;

    /// Build an AppState with two users and two live sessions. This bypasses the
    /// HTTP OPAQUE handshake (a separate concern) so the MFA surface + enforcement
    /// can be exercised end-to-end through the real router.
    async fn test_state() -> AppState {
        let path = std::env::temp_dir().join(format!("vautr_mfa_e2e_{}.db", uuid::Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(Repository::new(pool));
        let now = 1_700_000_000_000i64;
        repo.create_user(
            "u1", "alice@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now,
        )
        .await
        .unwrap();
        repo.create_user(
            "u2", "bob@example.com", &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now,
        )
        .await
        .unwrap();
        for (tok, uid) in [("tok-alice", "u1"), ("tok-bob", "u2")] {
            sqlx::query(
                "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(tok)
            .bind(uid)
            .bind(4_000_000_000_000i64)
            .bind(now)
            .execute(repo.pool())
            .await
            .unwrap();
        }
        AppState::new(repo)
    }

    async fn request(
        app: Router,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method(method).uri(path);
        builder = builder.header("Content-Type", "application/json");
        if let Some(t) = token {
            builder = builder.header("Authorization", format!("Bearer {t}"));
        }
        let req = builder
            .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
            .unwrap();
        let resp = app.oneshot(req).await.expect("routed request");
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 2 * 1024 * 1024)
            .await
            .expect("read body");
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, value)
    }

    #[test]
    fn password_policy_evaluation() {
        let policy = MasterPasswordPolicy {
            min_length: 12,
            require_upper: true,
            require_lower: true,
            require_digit: true,
            require_special: true,
            min_entropy_bits: 60,
        };
        // Meets every rule.
        assert!(password_satisfies(&policy, "Correct-Horse-9!Battery"));
        // Too short.
        assert!(!password_satisfies(&policy, "Ab1!"));
        // Missing special char.
        assert!(!password_satisfies(&policy, "CorrectHorse9Battery"));
        // Missing digit.
        assert!(!password_satisfies(&policy, "Correct-Horse-Battery!"));
        // Low-entropy long password is rejected by the entropy rule.
        assert!(!password_satisfies(&policy, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(entropy_bits("abcdefghijklmnop") >= 60.0);
    }

    #[test]
    fn percent_encoding_is_rfc3986() {
        assert_eq!(percent_encode("alice@example.com"), "alice%40example.com");
        assert_eq!(percent_encode("Vautr"), "Vautr");
        assert_eq!(percent_encode("a b"), "a%20b");
    }

    #[tokio::test]
    async fn totp_enroll_verify_and_mandatory_enforcement_e2e() {
        let st = test_state().await;
        let app = build_router(st.clone());

        // Default status: MFA not required, nothing configured.
        let (s, r) = request(app.clone(), "GET", "/mfa/status", Some("tok-alice"), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(r["required"], json!(false));
        assert_eq!(r["configured_methods"].as_array().unwrap().len(), 0);

        // Default policy via GET.
        let (s, r) = request(app.clone(), "GET", "/mfa/policy", Some("tok-alice"), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(r["required"], json!(false));

        // Enforcement is a no-op while MFA is not required.
        assert!(enforce_mfa_required(&st, "u2").await.is_ok());

        // Issue a TOTP enrollment.
        let (s, issue) = request(app.clone(), "POST", "/mfa/totp/issue", Some("tok-alice"), None).await;
        assert_eq!(s, StatusCode::OK, "issue failed: {issue:?}");
        let enrollment_id = issue["enrollment_id"].as_str().unwrap().to_string();
        let secret_b32 = issue["secret"].as_str().unwrap().to_string();
        let otpauth_url = issue["otpauth_url"].as_str().unwrap();
        let qr = issue["qr_code_data_url"].as_str().unwrap();
        assert!(otpauth_url.starts_with("otpauth://totp/"));
        assert!(otpauth_url.contains(&secret_b32), "uri must carry the secret");
        assert!(qr.starts_with("data:image/svg+xml;base64,"));

        // Compute a valid code with an INDEPENDENT implementation (otpauth crate,
        // SHA1/6-digit) and complete enrollment.
        let otp = otpauth::TOTP::from_base32(&secret_b32).expect("decode base32 secret");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let code = otp.generate(30, now).to_string();
        let code = format!("{code:0>6}"); // zero-pad: otpauth drops leading zeros
        let (s, v) = request(
            app.clone(),
            "POST",
            "/mfa/totp/verify",
            Some("tok-alice"),
            Some(json!({ "enrollment_id": enrollment_id, "code": code })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "verify failed: {v:?}");
        assert_eq!(v["status"], json!("success"));
        assert_eq!(v["recovery_codes"].as_array().unwrap().len(), 5);

        // status now reports totp configured.
        let (s, r) = request(app.clone(), "GET", "/mfa/status", Some("tok-alice"), None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(
            r["configured_methods"].as_array().unwrap().contains(&json!("totp")),
            "configured_methods: {r:?}"
        );

        // A stale/wrong code is rejected on the active-secret (login) path.
        let wrong = otp.generate(30, now.saturating_sub(3600)).to_string();
        let wrong = format!("{wrong:0>6}");
        let (s, v) = request(
            app.clone(),
            "POST",
            "/mfa/totp/verify",
            Some("tok-alice"),
            Some(json!({ "code": wrong })),
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(v["error"], json!("invalid_totp_code"));

        // Turn mandatory MFA on.
        let (s, pol) = request(
            app.clone(),
            "PUT",
            "/mfa/policy",
            Some("tok-alice"),
            Some(json!({
                "required": true,
                "allowed_methods": ["totp"],
                "master_password_policy": {
                    "min_length": 12, "require_upper": true, "require_lower": true,
                    "require_digit": true, "require_special": true, "min_entropy_bits": 60
                }
            })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "policy put: {pol:?}");
        assert_eq!(pol["required"], json!(true));

        // ENFORCEMENT: a member with no configured method is DENIED when MFA is
        // mandatory (bob/u2), while a member with TOTP configured is allowed (alice/u1).
        let err = enforce_mfa_required(&st, "u2").await.unwrap_err();
        assert_eq!(err.status, StatusCode::FORBIDDEN);
        assert!(enforce_mfa_required(&st, "u1").await.is_ok());

        // status now reflects required=true.
        let (s, r) = request(app.clone(), "GET", "/mfa/status", Some("tok-alice"), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(r["required"], json!(true));
    }

    #[tokio::test]
    async fn policy_put_rejects_invalid_values_and_unauthenticated() {
        let st = test_state().await;
        let app = build_router(st.clone());

        // Empty allowed_methods is rejected.
        let (s, _) = request(
            app.clone(),
            "PUT",
            "/mfa/policy",
            Some("tok-alice"),
            Some(json!({ "required": true, "allowed_methods": [] })),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);

        // Unknown method is rejected.
        let (s, _) = request(
            app.clone(),
            "PUT",
            "/mfa/policy",
            Some("tok-alice"),
            Some(json!({ "required": true, "allowed_methods": ["sms"] })),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);

        // Unauthenticated access is rejected.
        let (s, _) = request(app.clone(), "GET", "/mfa/policy", None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
}
