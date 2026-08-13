//! HTTP `Transport` implementation (api.md §4-5). Talks to the Vautr server
//! over reqwest. Mirrors the exact JSON contract enforced by `vautr-server`.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;
use vautr_sync::engine::{PulledOverview, PushOutcome, Transport, TransportError};

use crate::project_transport::{ProjectSecretSummary, ProjectSummary};

/// Client transport over HTTP. Holds a shared `reqwest::Client`, the base URL,
/// and the bearer session token.
pub struct HttpTransport {
    client: reqwest::Client,
    base: String,
    token: Arc<tokio::sync::Mutex<String>>,
}

impl HttpTransport {
    /// Build a transport for `base_url` (e.g. `https://vault.example.com`) with
    /// an initial bearer `token`.
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base: base_url.into().trim_end_matches('/').to_string(),
            token: Arc::new(tokio::sync::Mutex::new(token.into())),
        }
    }

    /// Replace the bearer token (e.g. after a re-auth).
    pub fn set_token(&self, token: impl Into<String>) {
        // Synchronous: the caller's async context awaits the surrounding op.
        if let Ok(mut g) = self.token.try_lock() {
            *g = token.into();
        }
    }

    /// Read the current token (best-effort; falls back to empty on contention).
    fn auth(&self) -> String {
        self.token.try_lock().map(|g| g.clone()).unwrap_or_default()
    }
}

// --- JSON request/response shapes (api.md §4-5) ---------------------------

#[derive(serde::Deserialize)]
struct PullResp {
    new_cursor: u64,
    items: Vec<PullItem>,
}

#[derive(serde::Deserialize)]
struct PullItem {
    uuid: String,
    version: i64,
    enc_key_gen: i64,
    deleted_date: Option<i64>,
}

#[derive(serde::Serialize)]
struct PullPayloadsReq {
    items: Vec<PullTarget>,
}

#[derive(serde::Serialize)]
struct PullTarget {
    uuid: String,
    version: u64,
}

#[derive(serde::Deserialize)]
struct PullPayloadsResp {
    results: Vec<PayloadResult>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct PayloadResult {
    uuid: String,
    status: String,
    payload: Option<String>,
}

#[derive(serde::Serialize)]
struct PushBatchReq {
    items: Vec<PushItem>,
}

#[derive(serde::Serialize)]
struct PushItem {
    uuid: String,
    target_version: u64,
    enc_key_gen: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deleted_date: Option<i64>,
}

#[derive(serde::Deserialize)]
struct PushBatchResp {
    results: Vec<PushResult>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct PushResult {
    uuid: String,
    status: String,
    current_server_state: Option<ServerState>,
}

#[derive(serde::Deserialize)]
struct ServerState {
    version: i64,
}

#[derive(serde::Deserialize)]
struct AccountStatusResp {
    min_enc_key_gen: i64,
    svk_ciphertext_blob: String,
    #[serde(default)]
    second_factor_method: Option<String>,
}

#[derive(serde::Serialize)]
struct RotateKeyReq {
    new_min_enc_key_gen: i64,
    new_svk_ciphertext_blob: String,
}

#[derive(serde::Deserialize)]
struct RotateKeyResp {
    min_enc_key_gen: i64,
}

fn http_err(status: u16) -> TransportError {
    match status {
        410 => TransportError::CursorExpired,
        401 => TransportError::Http(401),
        422 => TransportError::Other("key_generation_too_old".into()),
        412 => TransportError::Other("precondition_failed".into()),
        _ => TransportError::Http(status),
    }
}

impl Transport for HttpTransport {
    fn pull(
        &self,
        cursor: u64,
    ) -> Pin<Box<dyn Future<Output = Result<(u64, Vec<PulledOverview>), TransportError>> + Send>>
    {
        let client = self.client.clone();
        let base = self.base.clone();
        let token = self.auth();
        Box::pin(async move {
            let resp = client
                .get(format!("{base}/sync/pull", base = base))
                .bearer_auth(token)
                .query(&[("cursor", cursor), ("limit", 100u64)])
                .send()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(http_err(resp.status().as_u16()));
            }
            let body: PullResp = resp
                .json()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            let items = body
                .items
                .into_iter()
                .filter_map(|i| {
                    let uuid = Uuid::parse_str(&i.uuid).ok()?;
                    Some(PulledOverview {
                        uuid,
                        version: i.version as u64,
                        enc_key_gen: i.enc_key_gen as u64,
                        deleted: i.deleted_date.is_some(),
                    })
                })
                .collect();
            Ok((body.new_cursor, items))
        })
    }

    fn fetch_payload(
        &self,
        uuid: &Uuid,
        version: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, TransportError>> + Send>> {
        let client = self.client.clone();
        let base = self.base.clone();
        let token = self.auth();
        let uuid = *uuid;
        Box::pin(async move {
            let resp = client
                .post(format!("{base}/sync/pull-payloads", base = base))
                .bearer_auth(token)
                .json(&PullPayloadsReq {
                    items: vec![PullTarget {
                        uuid: uuid.to_string(),
                        version,
                    }],
                })
                .send()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(http_err(resp.status().as_u16()));
            }
            let body: PullPayloadsResp = resp
                .json()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            match body.results.into_iter().next() {
                Some(r) if r.status == "payload_delivered" => {
                    let b64 = r.payload.ok_or_else(|| {
                        TransportError::Other("payload_delivered without payload".into())
                    })?;
                    B64.decode(&b64)
                        .map_err(|e| TransportError::Other(e.to_string()))
                }
                _ => Err(TransportError::Other("payload not available".into())),
            }
        })
    }

    fn push_batch(
        &self,
        items: Vec<(Uuid, u64, u64, Option<Vec<u8>>)>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PushOutcome>, TransportError>> + Send>> {
        let client = self.client.clone();
        let base = self.base.clone();
        let token = self.auth();
        Box::pin(async move {
            let req_items: Vec<PushItem> = items
                .into_iter()
                .map(|(uuid, target_version, enc_key_gen, payload)| PushItem {
                    uuid: uuid.to_string(),
                    target_version,
                    enc_key_gen,
                    payload: payload.map(|p| B64.encode(&p)),
                    deleted_date: None,
                })
                .collect();
            let resp = client
                .post(format!("{base}/sync/push-batch", base = base))
                .bearer_auth(token)
                .json(&PushBatchReq { items: req_items })
                .send()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(http_err(resp.status().as_u16()));
            }
            let body: PushBatchResp = resp
                .json()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            Ok(body
                .results
                .into_iter()
                .map(|r| match r.status.as_str() {
                    "success" => PushOutcome::Applied,
                    "epoch_too_old" => PushOutcome::EpochTooOld,
                    _ => PushOutcome::Conflict(
                        r.current_server_state
                            .map(|s| s.version as u64)
                            .unwrap_or(0),
                    ),
                })
                .collect())
        })
    }

    fn account_status(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(u64, Vec<u8>), TransportError>> + Send>> {
        let client = self.client.clone();
        let base = self.base.clone();
        let token = self.auth();
        Box::pin(async move {
            let resp = client
                .get(format!("{base}/account/status", base = base))
                .bearer_auth(token)
                .send()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(http_err(resp.status().as_u16()));
            }
            let body: AccountStatusResp = resp
                .json()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            let blob = B64
                .decode(&body.svk_ciphertext_blob)
                .map_err(|e| TransportError::Other(e.to_string()))?;
            Ok((body.min_enc_key_gen as u64, blob))
        })
    }

    fn second_factor_method(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>, TransportError>> + Send>> {
        let client = self.client.clone();
        let base = self.base.clone();
        let token = self.auth();
        Box::pin(async move {
            let resp = client
                .get(format!("{base}/account/status", base = base))
                .bearer_auth(token)
                .send()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(http_err(resp.status().as_u16()));
            }
            let body: AccountStatusResp = resp
                .json()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            Ok(body.second_factor_method)
        })
    }

    fn rotate_key(
        &self,
        new_min_gen: u64,
        new_svk_blob: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64, TransportError>> + Send>> {
        let client = self.client.clone();
        let base = self.base.clone();
        let token = self.auth();
        Box::pin(async move {
            let resp = client
                .post(format!("{base}/account/rotate-key", base = base))
                .bearer_auth(token)
                .json(&RotateKeyReq {
                    new_min_enc_key_gen: new_min_gen as i64,
                    new_svk_ciphertext_blob: B64.encode(&new_svk_blob),
                })
                .send()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(http_err(resp.status().as_u16()));
            }
            let body: RotateKeyResp = resp
                .json()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            Ok(body.min_enc_key_gen as u64)
        })
    }

    fn verify_totp(
        &self,
        code: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send>> {
        let client = self.client.clone();
        let base = self.base.clone();
        let token = self.auth();
        let code = code.to_string();
        Box::pin(async move {
            let resp = client
                .post(format!("{base}/mfa/totp/verify", base = base))
                .bearer_auth(token)
                .json(&serde_json::json!({ "code": code }))
                .send()
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(http_err(resp.status().as_u16()));
            }
            Ok(())
        })
    }
}

// --- Projects transport (mlp-wave-plan §3 A1) -----------------------------

impl crate::project_transport::ProjectTransport for HttpTransport {
    fn list_projects(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ProjectSummary>, String>> + Send>> {
        let client = self.client.clone();
        let base = self.base.clone();
        let token = self.auth();
        Box::pin(async move {
            let resp = client
                .get(format!("{base}/projects", base = base))
                .bearer_auth(token)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("projects list failed: {}", resp.status().as_u16()));
            }
            let body: ProjectsListResp = resp.json().await.map_err(|e| e.to_string())?;
            Ok(body.projects)
        })
    }

    fn list_project_secrets(
        &self,
        project_uuid: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ProjectSecretSummary>, String>> + Send>> {
        let client = self.client.clone();
        let base = self.base.clone();
        let token = self.auth();
        Box::pin(async move {
            let resp = client
                .get(format!(
                    "{base}/projects/{id}/secrets",
                    base = base,
                    id = project_uuid
                ))
                .bearer_auth(token)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!(
                    "projects list secrets failed: {}",
                    resp.status().as_u16()
                ));
            }
            let body: SecretListResp = resp.json().await.map_err(|e| e.to_string())?;
            Ok(body.secrets)
        })
    }
}

#[derive(serde::Deserialize)]
struct ProjectsListResp {
    projects: Vec<ProjectSummary>,
}

#[derive(serde::Deserialize)]
struct SecretListResp {
    secrets: Vec<ProjectSecretSummary>,
}
