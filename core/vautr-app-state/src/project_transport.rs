//! Projects transport (mlp-wave-plan.md §3 A1, mlp-scope.md §2).
//!
//! The server owns project membership + access control; the client never
//! decrypts server-stored ciphertext. Projects and project-scoped secret
//! *metadata* are therefore queried directly from the server over this
//! transport (an untrusted relay, exactly like [`crate::sharing`] and
//! [`crate::file_transfer`]).
//!
//! This mirrors the `ShareTransport` / `FileTransport` pattern: a trait object
//! (`ProjectTransportHandle`) with a **default** no-op implementation so the
//! existing `MockTransport` / `RoundTripTransport` test doubles keep compiling
//! even though they don't model projects. `HttpTransport` implements the real
//! endpoints; an in-memory relay is provided for tests.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use serde::{Deserialize, Serialize};

/// Handle type for the projects transport (avoids nested `>` in struct fields).
pub type ProjectTransportHandle = Arc<dyn ProjectTransport>;

/// The client-facing projects surface (read-only metadata; the server enforces
/// membership and reveal scopes). Implemented by `HttpTransport` in production
/// and `InMemoryProjectRelay` in tests.
pub trait ProjectTransport: Send + Sync {
    /// `GET /projects` — list projects visible to the caller.
    ///
    /// Provided as a default returning `Err` so transports that don't model
    /// projects keep compiling; `HttpTransport` overrides it.
    fn list_projects(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ProjectSummary>, String>> + Send>> {
        let _ = self;
        Box::pin(async move { Err("projects transport does not list projects".into()) })
    }

    /// `GET /projects/{uuid}/secrets` — list secret metadata within a project.
    /// The server enforces project access (CanView) before returning anything.
    ///
    /// Provided as a default returning `Err` so transports that don't model
    /// projects keep compiling; `HttpTransport` overrides it.
    fn list_project_secrets(
        &self,
        project_uuid: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ProjectSecretSummary>, String>> + Send>> {
        let _ = project_uuid;
        Box::pin(async move { Err("projects transport does not list project secrets".into()) })
    }
}

/// A project as returned by `GET /projects` (mirrors `ProjectResp` wire JSON;
/// `type` is lowercase, `role`/`permission` are lowercase wire strings).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub uuid: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub role: String,
    pub permission: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Secret metadata within a project (`GET /projects/{uuid}/secrets`),
/// mirroring `Secret` wire JSON. The `value_ciphertext` is never returned by
/// this endpoint (metadata only; reveal is a separate gated call).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSecretSummary {
    pub uuid: String,
    pub project_uuid: String,
    pub key: String,
    pub version: i64,
    pub created_by: String,
    pub last_accessed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// In-memory relay for tests (mirrors `InMemoryShareRelay`). Holds a set of
/// projects and the secret metadata grouped per project, and answers queries
/// without a server.
#[derive(Clone, Default)]
pub struct InMemoryProjectRelay {
    state: Arc<Mutex<RelayState>>,
}

#[derive(Default)]
struct RelayState {
    /// project_uuid -> project summary
    projects: HashMap<Uuid, ProjectSummary>,
    /// project_uuid -> secret metadata list
    secrets: HashMap<Uuid, Vec<ProjectSecretSummary>>,
}

impl InMemoryProjectRelay {
    /// Seed a project the caller can see.
    pub fn add_project(&self, summary: ProjectSummary) {
        if let Ok(id) = Uuid::parse_str(&summary.uuid) {
            self.state.lock().unwrap().projects.insert(id, summary);
        }
    }

    /// Seed secret metadata inside a project.
    pub fn add_secret(&self, project_uuid: Uuid, secret: ProjectSecretSummary) {
        self.state
            .lock()
            .unwrap()
            .secrets
            .entry(project_uuid)
            .or_default()
            .push(secret);
    }
}

impl ProjectTransport for InMemoryProjectRelay {
    fn list_projects(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ProjectSummary>, String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move { Ok(state.lock().unwrap().projects.values().cloned().collect()) })
    }

    fn list_project_secrets(
        &self,
        project_uuid: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ProjectSecretSummary>, String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move {
            Ok(state
                .lock()
                .unwrap()
                .secrets
                .get(&project_uuid)
                .cloned()
                .unwrap_or_default())
        })
    }
}
