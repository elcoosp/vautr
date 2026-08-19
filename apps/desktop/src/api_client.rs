//! Real HTTP client for the Vautr server's Projects / roles / members / Secrets
//! API. Mirrors `packages/api-contract/openapi.json` exactly against the live
//! server (Wave A endpoints), using a bearer session token from OPAQUE login.
//!
//! Zero-knowledge: secret values are encrypted client-side with the DEK before
//! they are sent (`value_ciphertext`); the server never sees plaintext. The
//! "reveal" path fetches the stored ciphertext via `/secrets/{uuid}/value` and
//! the desktop decrypts it locally with the DEK.

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};

// ── Wire types (mirror the frozen OpenAPI contract) ───────────────────────

/// A project as returned by the server.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProjectDto {
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub role: String,
    #[serde(default)]
    pub permission: Option<String>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

/// A project member with their org role + per-project permission.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProjectMemberDto {
    pub user_uuid: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub permission: String,
    #[serde(default)]
    pub added_at: i64,
    #[serde(default)]
    pub hide_password: bool,
}

/// Secret metadata (never the plaintext value).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SecretDto {
    pub uuid: String,
    pub project_uuid: String,
    pub key: String,
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub created_by: String,
    #[serde(default)]
    pub last_accessed_at: Option<i64>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

/// The reveal payload: `{ uuid, key, value_ciphertext }`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SecretValueDto {
    pub uuid: String,
    pub key: String,
    pub value_ciphertext: String,
}

/// Offboard (revoke-all) response.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OffboardDto {
    pub status: String,
    pub user_uuid: String,
    #[serde(default)]
    pub revoked_projects: u64,
    #[serde(default)]
    pub revoked_memberships: u64,
    #[serde(default)]
    pub revoked_tokens: u64,
    #[serde(default)]
    pub revoked_at: i64,
}

/// Server error envelope: `{ "error": "...", "message": "..." }`.
#[derive(Deserialize)]
struct ErrorEnvelope {
    #[serde(default)]
    error: String,
    #[serde(default)]
    message: String,
}

/// Wrap the JSON list responses.
#[derive(Deserialize)]
struct ProjectListEnvelope {
    projects: Vec<ProjectDto>,
}

#[derive(Deserialize)]
struct ProjectMemberListEnvelope {
    members: Vec<ProjectMemberDto>,
}

#[derive(Deserialize)]
struct SecretListEnvelope {
    secrets: Vec<SecretDto>,
}

#[derive(Deserialize)]
struct StatusEnvelope {
    #[allow(dead_code)]
    status: String,
}

// ── MFA / machine-account / token wire types (Wave A2/A4) ────────────────

/// MFA status: whether MFA is required + which methods are configured.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MfaStatusDto {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub configured_methods: Vec<String>,
}

/// TOTP enrollment result (otpauth URL + manual secret).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TotpIssueDto {
    pub enrollment_id: String,
    pub otpauth_url: String,
    pub secret: String,
    #[serde(default)]
    pub qr_code_data_url: Option<String>,
}

/// TOTP verify result (enrollment complete + optional recovery codes).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TotpVerifyDto {
    pub status: String,
    #[serde(default)]
    pub recovery_codes: Option<Vec<String>>,
}

/// A machine account (non-human identity).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MachineAccountDto {
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub project_uuid: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub last_used_at: Option<i64>,
    #[serde(default)]
    pub created_at: i64,
}

/// An access token (metadata; the raw secret is only returned on creation).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccessTokenDto {
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub machine_account_uuid: Option<String>,
    #[serde(default)]
    pub project_uuid: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub revoked_at: Option<i64>,
    #[serde(default)]
    pub last_used_at: Option<i64>,
    #[serde(default)]
    pub created_at: i64,
}

/// The one-time raw token value returned by `POST /tokens`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccessTokenCreateDto {
    pub token: String,
    pub token_id: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// `GET /backup` — backup configuration and last-run state.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BackupStatusDto {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub schedule: String,
    #[serde(default)]
    pub last_backup_at: Option<i64>,
    #[serde(default)]
    pub last_backup_size_bytes: Option<u64>,
    #[serde(default)]
    pub last_restore_test_at: Option<i64>,
    #[serde(default)]
    pub last_restore_test_status: Option<String>,
}

/// `POST /backup/export` — an on-demand encrypted backup archive.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BackupExportDto {
    #[serde(default)]
    pub backup_id: String,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub checksum: String,
    #[serde(default)]
    pub created_at: i64,
}

/// `POST /backup/restore` — restore from a base64 archive or backup id.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BackupRestoreDto {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub test_id: String,
    #[serde(default)]
    pub restored_records: u64,
    #[serde(default)]
    pub restored_at: i64,
}

#[derive(Deserialize)]
struct MachineAccountListEnvelope {
    machine_accounts: Vec<MachineAccountDto>,
}

#[derive(Deserialize)]
struct AccessTokenListEnvelope {
    tokens: Vec<AccessTokenDto>,
}

// ── The client ────────────────────────────────────────────────────────────

/// Lightweight HTTP client for the authenticated Projects/Secrets API.
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    /// Create a client targeting `base_url` (e.g. `http://localhost:8080`).
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        token: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method.clone(), &url).bearer_auth(token);
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await.map_err(|e| {
            let url = url.clone();
            // Connection-level failures (server down, wrong host/port) surface
            // as reqwest "error sending request for url (...)". Surface a clear,
            // actionable message instead of the raw library string (VTR-096).
            if e.is_connect() || e.is_timeout() || e.is_request() {
                format!(
                    "Cannot reach the Vautr server at {url}. Is it running? (Underlying \
                         error: {e})"
                )
            } else {
                format!("request {url}: {e}")
            }
        })?;
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
            return Err(format!("HTTP {} {method} {path}: {msg}", status.as_u16()));
        }
        serde_json::from_slice::<T>(&bytes).map_err(|e| format!("parse {url}: {e}"))
    }

    // ── Projects ────────────────────────────────────────────────────────

    pub async fn list_projects(&self, token: &str) -> Result<Vec<ProjectDto>, String> {
        let env: ProjectListEnvelope = self.send(Method::GET, token, "/projects", None).await?;
        Ok(env.projects)
    }

    pub async fn create_project(
        &self,
        token: &str,
        name: &str,
        kind: &str,
        description: Option<&str>,
    ) -> Result<ProjectDto, String> {
        let mut body = serde_json::Map::new();
        body.insert("name".into(), serde_json::json!(name));
        body.insert("type".into(), serde_json::json!(kind));
        if let Some(d) = description {
            body.insert("description".into(), serde_json::json!(d));
        }
        self.send(
            Method::POST,
            token,
            "/projects",
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    pub async fn update_project(
        &self,
        token: &str,
        uuid: &str,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<ProjectDto, String> {
        let mut body = serde_json::Map::new();
        if let Some(n) = name {
            body.insert("name".into(), serde_json::json!(n));
        }
        if let Some(d) = description {
            body.insert("description".into(), serde_json::json!(d));
        }
        self.send(
            Method::PATCH,
            token,
            &format!("/projects/{uuid}"),
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    pub async fn delete_project(&self, token: &str, uuid: &str) -> Result<(), String> {
        let _: StatusEnvelope = self
            .send(Method::DELETE, token, &format!("/projects/{uuid}"), None)
            .await?;
        Ok(())
    }

    // ── Project members (role + permission controls) ────────────────────

    pub async fn list_members(
        &self,
        token: &str,
        project_uuid: &str,
    ) -> Result<Vec<ProjectMemberDto>, String> {
        let env: ProjectMemberListEnvelope = self
            .send(
                Method::GET,
                token,
                &format!("/projects/{project_uuid}/members"),
                None,
            )
            .await?;
        Ok(env.members)
    }

    pub async fn add_member(
        &self,
        token: &str,
        project_uuid: &str,
        user_uuid: &str,
        role: &str,
        permission: &str,
    ) -> Result<ProjectMemberDto, String> {
        let body = serde_json::json!({
            "user_uuid": user_uuid,
            "role": role,
            "permission": permission,
        });
        self.send(
            Method::POST,
            token,
            &format!("/projects/{project_uuid}/members"),
            Some(body),
        )
        .await
    }

    pub async fn update_member(
        &self,
        token: &str,
        project_uuid: &str,
        user_uuid: &str,
        role: Option<&str>,
        permission: Option<&str>,
    ) -> Result<ProjectMemberDto, String> {
        let mut body = serde_json::Map::new();
        if let Some(r) = role {
            body.insert("role".into(), serde_json::json!(r));
        }
        if let Some(p) = permission {
            body.insert("permission".into(), serde_json::json!(p));
        }
        self.send(
            Method::PATCH,
            token,
            &format!("/projects/{project_uuid}/members/{user_uuid}"),
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    pub async fn remove_member(
        &self,
        token: &str,
        project_uuid: &str,
        user_uuid: &str,
    ) -> Result<(), String> {
        let _: StatusEnvelope = self
            .send(
                Method::DELETE,
                token,
                &format!("/projects/{project_uuid}/members/{user_uuid}"),
                None,
            )
            .await?;
        Ok(())
    }

    // ── Secrets (ciphertext-only; values encrypted with the DEK) ────────

    pub async fn list_secrets(
        &self,
        token: &str,
        project_uuid: &str,
    ) -> Result<Vec<SecretDto>, String> {
        let env: SecretListEnvelope = self
            .send(
                Method::GET,
                token,
                &format!("/projects/{project_uuid}/secrets"),
                None,
            )
            .await?;
        Ok(env.secrets)
    }

    /// Create a secret. `value_ciphertext` is the base64 AEAD ciphertext of the
    /// plaintext value, produced locally with the DEK.
    pub async fn create_secret(
        &self,
        token: &str,
        project_uuid: &str,
        key: &str,
        value_ciphertext_b64: &str,
    ) -> Result<SecretDto, String> {
        let body = serde_json::json!({
            "project_uuid": project_uuid,
            "key": key,
            "value_ciphertext": value_ciphertext_b64,
        });
        self.send(Method::POST, token, "/secrets", Some(body)).await
    }

    pub async fn update_secret(
        &self,
        token: &str,
        uuid: &str,
        key: Option<&str>,
        value_ciphertext_b64: Option<&str>,
    ) -> Result<SecretDto, String> {
        let mut body = serde_json::Map::new();
        if let Some(k) = key {
            body.insert("key".into(), serde_json::json!(k));
        }
        if let Some(v) = value_ciphertext_b64 {
            body.insert("value_ciphertext".into(), serde_json::json!(v));
        }
        self.send(
            Method::PATCH,
            token,
            &format!("/secrets/{uuid}"),
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    pub async fn delete_secret(&self, token: &str, uuid: &str) -> Result<(), String> {
        let _: StatusEnvelope = self
            .send(Method::DELETE, token, &format!("/secrets/{uuid}"), None)
            .await?;
        Ok(())
    }

    /// Fetch a secret's ciphertext value (the server cannot decrypt it).
    pub async fn get_secret_value(
        &self,
        token: &str,
        uuid: &str,
    ) -> Result<SecretValueDto, String> {
        self.send(Method::GET, token, &format!("/secrets/{uuid}/value"), None)
            .await
    }

    // ── Offboarding (revoke all of a user's access) ─────────────────────

    pub async fn offboard(
        &self,
        token: &str,
        user_uuid: &str,
        reason: Option<&str>,
    ) -> Result<OffboardDto, String> {
        let mut body = serde_json::Map::new();
        body.insert("user_uuid".into(), serde_json::json!(user_uuid));
        if let Some(r) = reason {
            body.insert("reason".into(), serde_json::json!(r));
        }
        self.send(
            Method::POST,
            token,
            "/offboard",
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    // ── MFA (Wave A4: mandatory MFA + TOTP) ─────────────────────────────

    /// Whether the account requires MFA + which methods are configured.
    pub async fn mfa_status(&self, token: &str) -> Result<MfaStatusDto, String> {
        self.send(Method::GET, token, "/mfa/status", None).await
    }

    /// Begin TOTP enrollment (returns the otpauth URL + manual secret).
    pub async fn mfa_totp_issue(&self, token: &str) -> Result<TotpIssueDto, String> {
        self.send(Method::POST, token, "/mfa/totp/issue", None)
            .await
    }

    /// Verify a TOTP code to complete enrollment.
    pub async fn mfa_totp_verify(
        &self,
        token: &str,
        enrollment_id: Option<&str>,
        code: &str,
    ) -> Result<TotpVerifyDto, String> {
        let mut body = serde_json::Map::new();
        body.insert("code".into(), serde_json::json!(code));
        if let Some(id) = enrollment_id {
            body.insert("enrollment_id".into(), serde_json::json!(id));
        }
        self.send(
            Method::POST,
            token,
            "/mfa/totp/verify",
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    // ── Machine accounts (Wave A2) ──────────────────────────────────────

    pub async fn list_machine_accounts(
        &self,
        token: &str,
    ) -> Result<Vec<MachineAccountDto>, String> {
        let env: MachineAccountListEnvelope = self
            .send(Method::GET, token, "/machine-accounts", None)
            .await?;
        Ok(env.machine_accounts)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_machine_account(
        &self,
        token: &str,
        name: &str,
        description: Option<&str>,
        project_uuid: Option<&str>,
        scopes: &[&str],
    ) -> Result<MachineAccountDto, String> {
        let mut body = serde_json::Map::new();
        body.insert("name".into(), serde_json::json!(name));
        body.insert(
            "scopes".into(),
            serde_json::json!(scopes.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
        );
        if let Some(d) = description {
            body.insert("description".into(), serde_json::json!(d));
        }
        if let Some(p) = project_uuid {
            body.insert("project_uuid".into(), serde_json::json!(p));
        }
        self.send(
            Method::POST,
            token,
            "/machine-accounts",
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    // ── Access tokens (Wave A2) ─────────────────────────────────────────

    pub async fn list_tokens(&self, token: &str) -> Result<Vec<AccessTokenDto>, String> {
        let env: AccessTokenListEnvelope = self.send(Method::GET, token, "/tokens", None).await?;
        Ok(env.tokens)
    }

    pub async fn create_token(
        &self,
        token: &str,
        name: &str,
        scopes: &[&str],
    ) -> Result<AccessTokenCreateDto, String> {
        let mut body = serde_json::Map::new();
        body.insert("name".into(), serde_json::json!(name));
        body.insert(
            "scopes".into(),
            serde_json::json!(scopes.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
        );
        self.send(
            Method::POST,
            token,
            "/tokens",
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    /// Revoke an access token (`DELETE /tokens/{uuid}`).
    pub async fn revoke_token(&self, token: &str, uuid: &str) -> Result<serde_json::Value, String> {
        self.send(Method::DELETE, token, &format!("/tokens/{uuid}"), None)
            .await
    }

    // ── Machine account mutations ───────────────────────────────────────

    /// Enable/disable a machine account (`PATCH /machine-accounts/{uuid}`).
    pub async fn update_machine_account_status(
        &self,
        token: &str,
        uuid: &str,
        status: &str,
    ) -> Result<MachineAccountDto, String> {
        let mut body = serde_json::Map::new();
        body.insert("status".into(), serde_json::json!(status));
        self.send(
            Method::PATCH,
            token,
            &format!("/machine-accounts/{uuid}"),
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    /// Delete a machine account (`DELETE /machine-accounts/{uuid}`).
    pub async fn delete_machine_account(
        &self,
        token: &str,
        uuid: &str,
    ) -> Result<serde_json::Value, String> {
        self.send(
            Method::DELETE,
            token,
            &format!("/machine-accounts/{uuid}"),
            None,
        )
        .await
    }

    // ── Backup (import/export) ──────────────────────────────────────────

    /// `GET /backup` — backup configuration and last-run state.
    pub async fn backup_status(&self, token: &str) -> Result<BackupStatusDto, String> {
        self.send(Method::GET, token, "/backup", None).await
    }

    /// `POST /backup/export` — create an encrypted backup archive.
    pub async fn backup_export(
        &self,
        token: &str,
        include_secrets: bool,
    ) -> Result<BackupExportDto, String> {
        let mut body = serde_json::Map::new();
        body.insert("include_secrets".into(), serde_json::json!(include_secrets));
        self.send(
            Method::POST,
            token,
            "/backup/export",
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    /// `POST /backup/restore` — restore from a base64 archive.
    pub async fn backup_restore(
        &self,
        token: &str,
        archive_base64: &str,
    ) -> Result<BackupRestoreDto, String> {
        let mut body = serde_json::Map::new();
        body.insert("archive_base64".into(), serde_json::json!(archive_base64));
        self.send(
            Method::POST,
            token,
            "/backup/restore",
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    /// `GET /audit` — list audit-log entries (VTR-071). Metadata-only: the
    /// server never returns secret plaintext. Returns the raw JSON array.
    pub async fn audit_list(
        &self,
        token: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<serde_json::Value, String> {
        let mut path = "/audit".to_string();
        if limit.is_some() || offset.is_some() {
            let mut q = Vec::new();
            if let Some(l) = limit {
                q.push(format!("limit={l}"));
            }
            if let Some(o) = offset {
                q.push(format!("offset={o}"));
            }
            path = format!("{path}?{}", q.join("&"));
        }
        self.send(Method::GET, token, &path, None).await
    }

    /// `GET /shares/inbox` — list shares awaiting the current user (VTR-072).
    /// Returns the raw JSON array of incoming shares.
    pub async fn get_share_inbox(
        &self,
        token: &str,
    ) -> Result<serde_json::Value, String> {
        self.send(Method::GET, token, "/shares/inbox", None).await
    }

    /// `GET /shares/groups` — list the groups the current user belongs to,
    /// including pending invites (VTR-072). Returns the raw JSON array.
    pub async fn get_group_inbox(
        &self,
        token: &str,
    ) -> Result<serde_json::Value, String> {
        self.send(Method::GET, token, "/shares/groups", None).await
    }
}

/// Convenience: base64-encode a ciphertext blob for the wire format.
pub fn b64_encode(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

/// Convenience: base64-decode a ciphertext blob from the wire format.
pub fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    B64.decode(s).map_err(|e| format!("base64 decode: {e}"))
}

/// Build the Associated Data binding a project secret's ciphertext to its
/// (project, key) context.
///
/// The server generates the secret's UUID on create, so the AEAD AD cannot be
/// bound to the (unknown-at-encrypt-time) secret UUID. Instead we bind to the
/// deterministic `(project_uuid, key)` pair, which the desktop knows both when
/// it encrypts a value and when it decrypts the stored ciphertext at reveal.
/// The `v1` version prefix allows the scheme to evolve without ambiguity.
pub fn secret_ad(project_uuid: &str, key: &str) -> Vec<u8> {
    format!("vautr-secret:v1:{project_uuid}:{key}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dto_roundtrips_wire_format() {
        let json = r#"{
            "uuid": "11111111-1111-1111-1111-111111111111",
            "name": "Engineering",
            "description": null,
            "type": "shared",
            "role": "admin",
            "permission": "can_manage",
            "created_at": 1000,
            "updated_at": 2000
        }"#;
        let p: ProjectDto = serde_json::from_str(json).unwrap();
        assert_eq!(p.kind, "shared");
        assert_eq!(p.role, "admin");
        assert_eq!(p.permission.as_deref(), Some("can_manage"));
    }

    #[test]
    fn project_member_roundtrips_wire_format() {
        let json = r#"{
            "user_uuid": "22222222-2222-2222-2222-222222222222",
            "display_name": "alice@example.com",
            "role": "manager",
            "permission": "can_edit",
            "added_at": 1000,
            "hide_password": true
        }"#;
        let m: ProjectMemberDto = serde_json::from_str(json).unwrap();
        assert_eq!(m.role, "manager");
        assert_eq!(m.permission, "can_edit");
        assert!(m.hide_password);
    }

    #[test]
    fn secret_roundtrips_wire_format() {
        let json = r#"{
            "uuid": "33333333-3333-3333-3333-333333333333",
            "project_uuid": "11111111-1111-1111-1111-111111111111",
            "key": "DATABASE_URL",
            "version": 1,
            "created_by": "22222222-2222-2222-2222-222222222222",
            "last_accessed_at": 1500,
            "created_at": 1000,
            "updated_at": 1000
        }"#;
        let s: SecretDto = serde_json::from_str(json).unwrap();
        assert_eq!(s.key, "DATABASE_URL");
        assert_eq!(s.last_accessed_at, Some(1500));
    }
}
