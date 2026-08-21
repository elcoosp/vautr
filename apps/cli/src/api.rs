//! Minimal HTTP client for the Vautr server (OpenAPI: `packages/api-contract`).
//!
//! Talks to the live server over reqwest. Wire shapes mirror the server
//! handlers and the `packages/api-contract/openapi.json` schemas for:
//! projects, secrets (incl. reveal), machine accounts, access tokens, and the
//! OPAQUE auth handshake.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{CliError, CliResult};

/// A project as returned by `GET/POST /projects`.
#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub uuid: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub proj_type: String,
    pub role: Option<String>,
    pub permission: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A secret's metadata as returned by `GET /projects/{uuid}/secrets`.
#[derive(Debug, Clone, Deserialize)]
pub struct Secret {
    pub uuid: String,
    pub project_uuid: String,
    pub key: String,
    pub version: i64,
    pub created_by: String,
    pub last_accessed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A machine account as returned by `POST /machine-accounts`.
#[derive(Debug, Clone, Deserialize)]
pub struct MachineAccount {
    pub uuid: String,
    pub name: String,
    pub status: String,
}

/// A freshly-issued access token (`POST /tokens`).
#[derive(Debug, Clone, Deserialize)]
pub struct IssuedToken {
    pub token: String,
    pub token_id: String,
}

/// `POST /backup/export` response.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupExportResponse {
    pub backup_id: String,
    pub download_url: Option<String>,
    pub size_bytes: u64,
    pub checksum: String,
    pub created_at: i64,
}

/// `POST /backup/restore` (restore test) response.
#[derive(Debug, Clone, Deserialize)]
pub struct BackupRestoreResponse {
    pub status: String,
    pub test_id: String,
    pub restored_records: u64,
    pub restored_at: i64,
}

/// The OPAQUE login-start response.
#[derive(Deserialize)]
struct LoginStartResp {
    login_response: String,
}

/// The OPAQUE login-finish response (session token).
#[derive(Deserialize)]
struct LoginFinishResp {
    session_token: String,
    #[allow(dead_code)]
    expires_at: i64,
}

/// The OPAQUE register-start response.
#[derive(Deserialize)]
struct RegisterStartResp {
    registration_response: String,
}

/// Thin async HTTP client for the Vautr server.
#[derive(Debug, Clone)]
pub struct Api {
    base: String,
    client: reqwest::Client,
}

impl Api {
    /// Build a client for the given server base URL.
    pub fn new(base: impl Into<String>) -> CliResult<Self> {
        let client = reqwest::Client::builder().build().map_err(CliError::Http)?;
        Ok(Self {
            base: base.into(),
            client,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.trim_end_matches('/'), path)
    }

    /// OPAQUE login start. Returns the base64 server `login_response`.
    pub async fn login_start(&self, username: &str, login_start_b64: &str) -> CliResult<String> {
        let resp = self
            .client
            .post(self.url("/auth/login/start"))
            .json(&json!({ "username": username, "login_start": login_start_b64 }))
            .send()
            .await?;
        let body = self.check(resp).await?;
        let parsed: LoginStartResp = serde_json::from_value(body)?;
        Ok(parsed.login_response)
    }

    /// OPAQUE login finish. Returns the session token.
    pub async fn login_finish(&self, username: &str, login_finish_b64: &str) -> CliResult<String> {
        let resp = self
            .client
            .post(self.url("/auth/login/finish"))
            .json(&json!({ "username": username, "login_finish": login_finish_b64 }))
            .send()
            .await?;
        let body = self.check(resp).await?;
        let parsed: LoginFinishResp = serde_json::from_value(body)?;
        Ok(parsed.session_token)
    }

    /// OPAQUE register start. Returns the base64 server `registration_response`.
    pub async fn register_start(&self, username: &str, reg_start_b64: &str) -> CliResult<String> {
        let resp = self
            .client
            .post(self.url("/auth/register/start"))
            .json(&json!({ "username": username, "registration_start": reg_start_b64 }))
            .send()
            .await?;
        let body = self.check(resp).await?;
        let parsed: RegisterStartResp = serde_json::from_value(body)?;
        Ok(parsed.registration_response)
    }

    /// OPAQUE register finish.
    #[allow(clippy::too_many_arguments)]
    pub async fn register_finish(
        &self,
        username: &str,
        reg_finish_b64: &str,
        server_public_key_b64: &str,
        kdf_salt_b64: &str,
        svk_ciphertext_blob_b64: &str,
        svk_ciphertext_blob_rk_b64: &str,
    ) -> CliResult<()> {
        let resp = self
            .client
            .post(self.url("/auth/register/finish"))
            .json(&json!({
                "username": username,
                "registration_finish": reg_finish_b64,
                "server_public_key": server_public_key_b64,
                "kdf_salt": kdf_salt_b64,
                "svk_ciphertext_blob": svk_ciphertext_blob_b64,
                "svk_ciphertext_blob_rk": svk_ciphertext_blob_rk_b64,
            }))
            .send()
            .await?;
        self.check(resp).await?;
        Ok(())
    }

    /// `GET /projects` — list projects for the caller.
    pub async fn list_projects(&self, token: &str) -> CliResult<Vec<Project>> {
        let resp = self
            .client
            .get(self.url("/projects"))
            .bearer_auth(token)
            .send()
            .await?;
        let body = self.check(resp).await?;
        let parsed: Value = body;
        let arr = parsed
            .get("projects")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        serde_json::from_value(Value::Array(arr)).map_err(CliError::Json)
    }

    /// `POST /projects` — create a project.
    pub async fn create_project(
        &self,
        token: &str,
        name: &str,
        description: Option<&str>,
        proj_type: Option<&str>,
    ) -> CliResult<Project> {
        let mut body = serde_json::Map::new();
        body.insert("name".into(), json!(name));
        if let Some(d) = description {
            body.insert("description".into(), json!(d));
        }
        if let Some(t) = proj_type {
            body.insert("type".into(), json!(t));
        }
        let resp = self
            .client
            .post(self.url("/projects"))
            .bearer_auth(token)
            .json(&Value::Object(body))
            .send()
            .await?;
        let body = self.check(resp).await?;
        serde_json::from_value(body).map_err(CliError::Json)
    }

    /// `PATCH /projects/{uuid}` — update a project's name/description.
    pub async fn update_project(
        &self,
        token: &str,
        uuid: &str,
        name: Option<&str>,
        description: Option<&str>,
    ) -> CliResult<Project> {
        let mut body = serde_json::Map::new();
        if let Some(n) = name {
            body.insert("name".into(), json!(n));
        }
        if let Some(d) = description {
            body.insert("description".into(), json!(d));
        }
        let resp = self
            .client
            .patch(self.url(&format!("/projects/{uuid}")))
            .bearer_auth(token)
            .json(&Value::Object(body))
            .send()
            .await?;
        let body = self.check(resp).await?;
        serde_json::from_value(body).map_err(CliError::Json)
    }

    /// `GET /projects/{uuid}/secrets` — list secret metadata in a project.
    pub async fn list_secrets(&self, token: &str, project_uuid: &str) -> CliResult<Vec<Secret>> {
        let resp = self
            .client
            .get(self.url(&format!("/projects/{project_uuid}/secrets")))
            .bearer_auth(token)
            .send()
            .await?;
        let body = self.check(resp).await?;
        let arr = body
            .get("secrets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        serde_json::from_value(Value::Array(arr)).map_err(CliError::Json)
    }

    /// `POST /secrets` — create a secret (value is base64 ciphertext).
    pub async fn create_secret(
        &self,
        token: &str,
        project_uuid: &str,
        key: &str,
        value_ciphertext_b64: &str,
    ) -> CliResult<Secret> {
        let resp = self
            .client
            .post(self.url("/secrets"))
            .bearer_auth(token)
            .json(&json!({
                "project_uuid": project_uuid,
                "key": key,
                "value_ciphertext": value_ciphertext_b64,
            }))
            .send()
            .await?;
        let body = self.check(resp).await?;
        serde_json::from_value(body).map_err(CliError::Json)
    }

    /// `PATCH /secrets/{uuid}` — update a secret's key and/or value.
    pub async fn update_secret(
        &self,
        token: &str,
        uuid: &str,
        key: Option<&str>,
        value_ciphertext_b64: Option<&str>,
    ) -> CliResult<Secret> {
        let mut body = serde_json::Map::new();
        if let Some(k) = key {
            body.insert("key".into(), json!(k));
        }
        if let Some(v) = value_ciphertext_b64 {
            body.insert("value_ciphertext".into(), json!(v));
        }
        let resp = self
            .client
            .patch(self.url(&format!("/secrets/{uuid}")))
            .bearer_auth(token)
            .json(&Value::Object(body))
            .send()
            .await?;
        let body = self.check(resp).await?;
        serde_json::from_value(body).map_err(CliError::Json)
    }

    /// `GET /secrets/{uuid}/value` — reveal a secret's ciphertext (secrets:reveal gate).
    pub async fn reveal_secret(&self, token: &str, uuid: &str) -> CliResult<(String, String)> {
        let resp = self
            .client
            .get(self.url(&format!("/secrets/{uuid}/value")))
            .bearer_auth(token)
            .send()
            .await?;
        let body = self.check(resp).await?;
        let key = body
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let ct = body
            .get("value_ciphertext")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        Ok((key, ct))
    }

    /// `POST /machine-accounts` — provision a machine account.
    pub async fn create_machine_account(
        &self,
        token: &str,
        name: &str,
        scopes: &[&str],
    ) -> CliResult<MachineAccount> {
        let resp = self
            .client
            .post(self.url("/machine-accounts"))
            .bearer_auth(token)
            .json(&json!({ "name": name, "scopes": scopes }))
            .send()
            .await?;
        let body = self.check(resp).await?;
        serde_json::from_value(body).map_err(CliError::Json)
    }

    /// `POST /tokens` — issue an access token bound to a machine account.
    pub async fn issue_token(
        &self,
        token: &str,
        name: &str,
        machine_account_uuid: &str,
        scopes: &[&str],
    ) -> CliResult<IssuedToken> {
        let resp = self
            .client
            .post(self.url("/tokens"))
            .bearer_auth(token)
            .json(&json!({
                "name": name,
                "machine_account_uuid": machine_account_uuid,
                "scopes": scopes,
            }))
            .send()
            .await?;
        let body = self.check(resp).await?;
        serde_json::from_value(body).map_err(CliError::Json)
    }

    /// `POST /backup/export` — create an encrypted backup archive and return
    /// its server-side id + download URL (does not return the archive bytes).
    pub async fn export_backup(
        &self,
        token: &str,
        include_secrets: bool,
    ) -> CliResult<BackupExportResponse> {
        let resp = self
            .client
            .post(self.url("/backup/export"))
            .bearer_auth(token)
            .json(&json!({ "include_secrets": include_secrets }))
            .send()
            .await?;
        let body = self.check(resp).await?;
        serde_json::from_value(body).map_err(CliError::Json)
    }

    /// `POST /backup/restore` — validate a local `.vautr` archive (restore
    /// test) without touching the live store.
    pub async fn restore_backup(
        &self,
        token: &str,
        archive_base64: &str,
    ) -> CliResult<BackupRestoreResponse> {
        let resp = self
            .client
            .post(self.url("/backup/restore"))
            .bearer_auth(token)
            .json(&json!({ "archive_base64": archive_base64 }))
            .send()
            .await?;
        let body = self.check(resp).await?;
        serde_json::from_value(body).map_err(CliError::Json)
    }

    /// Validate a non-2xx response and parse the error envelope.
    async fn check(&self, resp: reqwest::Response) -> CliResult<Value> {
        let status = resp.status();
        if status.is_success() {
            return resp.json::<Value>().await.map_err(CliError::Http);
        }
        let status_u16 = status.as_u16();
        let text = resp.text().await.unwrap_or_default();
        Err(CliError::from_response_status(status_u16, &text))
    }
}

/// The machine-account scopes the CLI provisions by default (read/write/reveal
/// over projects and secrets, so a token can manage and reveal project secrets).
pub const MACHINE_SCOPES: [&str; 5] = [
    "secrets:read",
    "secrets:write",
    "secrets:reveal",
    "projects:read",
    "projects:write",
];
