//! Sync engine core: the `SyncEngine` trait and its state machine.
//! Spec: docs/architecture/api.md §4, REQ-SYNC-01..06.
//!
//! The engine is metadata-first (ADR-002): it pulls lightweight overviews and
//! resolves OCC conflicts, and only downloads payloads for non-ignored items.

use crate::dashmap::LocalBlacklist;
use crate::occ;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// A metadata-only item pulled from the server (no payload).
#[derive(Clone, Debug)]
pub struct PulledOverview {
    pub uuid: Uuid,
    pub version: u64,
    pub enc_key_gen: u64,
    pub deleted: bool,
}

/// Transport boundary to the server. Implemented by the HTTP client (api.md §4).
/// The engine calls this; the concrete network code lives outside the core.
///
/// Methods return `Pin<Box<dyn Future>>` (not `impl Future`) so the trait is
/// object-safe and usable as `Arc<dyn Transport>` (required by `VautrClient`).
pub trait Transport: Send + Sync {
    /// Pull metadata since `cursor`; returns `(next_cursor, overviews)`.
    fn pull(
        &self,
        cursor: u64,
    ) -> Pin<Box<dyn Future<Output = Result<(u64, Vec<PulledOverview>), TransportError>> + Send>>;

    /// Fetch a single item payload by uuid and the expected local `version`
    /// (api.md §4 exact-version selective download; prevents AEAD races).
    fn fetch_payload(
        &self,
        uuid: &Uuid,
        version: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, TransportError>> + Send>>;

    /// Push a batch of (uuid, target_version, enc_key_gen, payload) items.
    /// Returns per-item outcomes: `Ok` (applied), `Conflict` (412, retry at
    /// server version), or `EpochTooOld` (422, REQ-API-01).
    fn push_batch(
        &self,
        items: Vec<(Uuid, u64, u64, Option<Vec<u8>>)>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PushOutcome>, TransportError>> + Send>>;

    /// Fetch the account's current epoch gate + wrapped SVK (api.md §5
    /// `GET /account/status`). Returns `(min_enc_key_gen, svk_ciphertext_blob)`.
    fn account_status(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(u64, Vec<u8>), TransportError>> + Send>>;

    /// Advance the global epoch gate before a crash-safe rotation push
    /// (api.md §5 `POST /account/rotate-key`). Returns the confirmed
    /// `min_enc_key_gen` (idempotent — server returns 200 if already at it).
    fn rotate_key(
        &self,
        new_min_gen: u64,
        new_svk_blob: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64, TransportError>> + Send>>;
}

/// Transport-level error.
#[derive(Debug)]
pub enum TransportError {
    Http(u16),
    CursorExpired, // 410
    Other(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Http(code) => write!(f, "transport http error {code}"),
            TransportError::CursorExpired => write!(f, "sync cursor expired (410)"),
            TransportError::Other(m) => write!(f, "transport error: {m}"),
        }
    }
}

/// Outcome of pushing a single item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushOutcome {
    Applied,
    Conflict(u64), // server_version for re-push
    EpochTooOld,
}

/// Client sync engine. Pulls metadata, bounds payload I/O via DashMap, pushes
/// batches with per-item OCC (ADR-002 / ADR-004). Uses `Arc<dyn Transport>` so
/// the concrete network stack (HTTP) is injected by the client.
pub struct Engine {
    transport: Arc<dyn Transport>,
    blacklist: Arc<LocalBlacklist>,
    cursor: u64,
}

impl Engine {
    /// Create an engine over a transport and a (loaded) blacklist.
    pub fn new(transport: Arc<dyn Transport>, blacklist: Arc<LocalBlacklist>) -> Self {
        Self {
            transport,
            blacklist,
            cursor: 0,
        }
    }

    /// Metadata-first pull. Returns `(next_cursor, overviews)`.
    pub async fn pull(&self) -> Result<(u64, Vec<PulledOverview>), TransportError> {
        let (next, overviews) = self.transport.as_ref().pull(self.cursor).await?;
        Ok((next, overviews))
    }

    /// Download the payload for `uuid` at expected `version`, honoring the
    /// blacklist (ADR-002). Returns `None` for toxic/ignored items without
    /// contacting the server.
    pub async fn fetch_payload_if_allowed(
        &self,
        uuid: &Uuid,
        version: u64,
    ) -> Result<Option<Vec<u8>>, TransportError> {
        if self.blacklist.is_ignored(uuid) {
            return Ok(None);
        }
        let payload = self.transport.as_ref().fetch_payload(uuid, version).await?;
        Ok(Some(payload))
    }

    /// Push a batch, mapping 412s into conflict events via [`occ`].
    pub async fn push_batch(
        &self,
        items: Vec<(Uuid, u64, u64, Option<Vec<u8>>)>,
    ) -> Result<Vec<(Uuid, PushOutcome)>, TransportError> {
        let uuids: Vec<Uuid> = items.iter().map(|i| i.0).collect();
        let outcomes = self.transport.as_ref().push_batch(items).await?;
        Ok(uuids.into_iter().zip(outcomes).collect())
    }

    /// Resolve a 412 conflict for `uuid` (wraps [`occ::resolve`]).
    pub fn resolve_conflict(
        &self,
        uuid: Uuid,
        local_version: u64,
        server_version: u64,
        force_overwrite: bool,
    ) -> (occ::ConflictResolution, occ::ConflictEvent) {
        let is_toxic = self.blacklist.is_toxic(&uuid);
        occ::resolve(uuid, local_version, server_version, is_toxic, force_overwrite)
    }
}
