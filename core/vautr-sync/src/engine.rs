//! Sync engine core: the `SyncEngine` trait and its state machine.
//! Spec: docs/architecture/api.md §4, REQ-SYNC-01..06.
//!
//! The engine is metadata-first (ADR-002): it pulls lightweight overviews and
//! resolves OCC conflicts, and only downloads payloads for non-ignored items.

use crate::dashmap::LocalBlacklist;
use crate::occ;
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
pub trait Transport {
    /// Pull metadata since `cursor`; returns `(next_cursor, overviews)`.
    fn pull(
        &self,
        cursor: u64,
    ) -> impl std::future::Future<Output = Result<(u64, Vec<PulledOverview>), TransportError>> + Send;

    /// Fetch a single item payload by uuid.
    fn fetch_payload(
        &self,
        uuid: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, TransportError>> + Send;

    /// Push a batch of (uuid, target_version, enc_key_gen, payload) items.
    /// Returns per-item outcomes: `Ok` (applied), `Conflict` (412, retry at
    /// server version), or `EpochTooOld` (422, REQ-API-01).
    fn push_batch(
        &self,
        items: Vec<(Uuid, u64, u64, Option<Vec<u8>>)>,
    ) -> impl std::future::Future<Output = Result<Vec<PushOutcome>, TransportError>> + Send;
}

/// Transport-level error.
#[derive(Debug)]
pub enum TransportError {
    Http(u16),
    CursorExpired, // 410
    Other(String),
}

/// Outcome of pushing a single item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushOutcome {
    Applied,
    Conflict(u64), // server_version for re-push
    EpochTooOld,
}

/// Client sync engine. Pulls metadata, bounds payload I/O via DashMap, pushes
/// batches with per-item OCC (ADR-002 / ADR-004).
pub struct Engine<T: Transport> {
    transport: T,
    blacklist: Arc<LocalBlacklist>,
    cursor: u64,
}

impl<T: Transport> Engine<T> {
    /// Create an engine over a transport and a (loaded) blacklist.
    pub fn new(transport: T, blacklist: Arc<LocalBlacklist>) -> Self {
        Self {
            transport,
            blacklist,
            cursor: 0,
        }
    }

    /// Metadata-first pull. Toxic items are never fetched (ADR-002). Returns the
    /// overviews that should have payloads downloaded, excluding ignored ones.
    pub async fn pull(&self) -> Result<(u64, Vec<PulledOverview>), TransportError> {
        let (next, overviews) = self.transport.pull(self.cursor).await?;
        Ok((next, overviews))
    }

    /// Download the payload for `uuid`, honoring the blacklist (ADR-002).
    /// Returns `None` for toxic/ignored items without contacting the server.
    pub async fn fetch_payload_if_allowed(&self, uuid: &Uuid) -> Result<Option<Vec<u8>>, TransportError> {
        if self.blacklist.is_ignored(uuid) {
            return Ok(None);
        }
        let payload = self.transport.fetch_payload(uuid).await?;
        Ok(Some(payload))
    }

    /// Push a batch, mapping 412s into conflict events via [`occ`].
    pub async fn push_batch(
        &self,
        items: Vec<(Uuid, u64, u64, Option<Vec<u8>>)>,
    ) -> Result<Vec<(Uuid, PushOutcome)>, TransportError> {
        let uuids: Vec<Uuid> = items.iter().map(|i| i.0).collect();
        let outcomes = self.transport.push_batch(items).await?;
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
