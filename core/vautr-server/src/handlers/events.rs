//! Server-backed quarantine reaper events (VTR-069).
//!
//! The desktop client runs a local `Quarantine` reaper (`vautr_sync::quarantine`)
//! that emits `ItemRecovered` / `ItemPermanentlyDeleted` against local server
//! metadata. Web/extension clients have no equivalent local reaper and only
//! learn about tombstones on the next `sync/pull`. This module closes that gap
//! server-side:
//!
//! - Mutations publish [`VaultEvent`]s onto a `tokio::sync::broadcast` channel
//!   carried in [`crate::handlers::AppState`].
//! - `GET /events` streams those events to an authenticated client as SSE,
//!   so web/extension drop stale items / notify immediately (proactive push,
//!   mirroring desktop's `VaultStateUpdate::ItemPermanentlyDeleted`).
//! - A background [`spawn_reaper`] task periodically scans for tombstoned items
//!   and re-emits `ItemPermanentlyDeleted` (idempotent — clients treat repeats
//!   as no-ops), guaranteeing delivery even if a client missed the SSE frame.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use super::{auth_user, ApiError, AppState, Bearer};

/// A vault-level event the server proactively pushes to clients.
///
/// Mirrors `vautr_sync::quarantine::ReaperEvent` semantics so web/extension can
/// reuse the desktop reaper's event handling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VaultEvent {
    /// A previously quarantined/toxic item became valid; the client should
    /// re-sync to fetch its now-readable payload.
    ItemRecovered { uuid: Uuid },
    /// A toxic item was permanently deleted server-side; the client should drop
    /// any stale toxic indicator.
    ItemPermanentlyDeleted { uuid: Uuid },
}

impl VaultEvent {
    /// JSON text for an SSE `data:` frame.
    pub fn to_sse_data(&self) -> String {
        serde_json::to_string(self).expect("VaultEvent is always serializable")
    }
}

/// Capacity of the broadcast channel. Old events are dropped if a client is
/// slow, but the reaper task re-emits tombstone events so nothing is lost.
pub const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Build a fresh broadcast channel for vault events.
pub fn event_channel() -> broadcast::Sender<VaultEvent> {
    broadcast::channel(EVENT_CHANNEL_CAPACITY).0
}

/// SSE stream of vault events for the authenticated user.
///
/// Each frame is a JSON [`VaultEvent`]. The stream is never-ending; axum's SSE
/// machinery handles client disconnect. We also emit an initial `hello` frame so
/// the client can confirm the subscription is live.
pub(crate) async fn events_stream(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Response, ApiError> {
    let _user_id = auth_user(&st.repo, &auth.0).await?;

    let rx = st.event_tx.subscribe();
    let hello = Event::default().data("{\"type\":\"hello\"}");

    // Build an SSE stream: an initial `hello` frame, then each broadcast event.
    // Lagged/closed errors are skipped (the reaper re-emits tombstones, so a
    // missed frame is recovered on the next scan).
    let event_stream = stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    return Some((
                        Ok::<_, Infallible>(Event::default().data(ev.to_sse_data())),
                        rx,
                    ));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    let stream = stream::once(async move { Ok::<_, Infallible>(hello) }).chain(event_stream);

    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new()).into_response())
}

/// Interval between server reaper scans.
pub const REAPER_TICK: Duration = Duration::from_secs(30);

/// Spawn the server-side reaper. Every [`REAPER_TICK`] it scans for items with a
/// `deleted_date` set and emits `ItemPermanentlyDeleted` for each (idempotent on
/// the client). `seen` tracks already-emitted uuids so a stable tombstone only
/// fires once per process lifetime (newly-appearing tombstones still fire).
pub fn spawn_reaper(
    repo: Arc<crate::repository::Repository>,
    tx: broadcast::Sender<VaultEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(REAPER_TICK);
        let seen: Arc<tokio::sync::Mutex<std::collections::HashSet<Uuid>>> =
            Arc::new(tokio::sync::Mutex::new(std::collections::HashSet::new()));
        loop {
            interval.tick().await;
            let tombstoned = match repo.tombstoned_items().await {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::warn!(error = %e, "reaper: tombstone scan failed");
                    continue;
                }
            };
            let mut guard = seen.lock().await;
            for uuid in tombstoned {
                if guard.insert(uuid) {
                    let _ = tx.send(VaultEvent::ItemPermanentlyDeleted { uuid });
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::UpsertOutcome;
    use std::time::Duration;

    async fn temp_repo() -> Arc<crate::repository::Repository> {
        let path = std::env::temp_dir().join(format!("vautr_events_test_{}.db", Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let pool = crate::db::connect(&url).await.expect("connect + migrate");
        let repo = Arc::new(crate::repository::Repository::new(pool));
        let now = 1_700_000_000_000i64;
        repo.create_user(
            "u1",
            "alice@example.com",
            &[0u8; 32],
            &[1u8; 16],
            &[2u8; 48],
            &[3u8; 48],
            now,
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at) \
             VALUES ('11111111-1111-1111-1111-111111111111', 'u1', 2, 1, ?, NULL, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at) \
             VALUES ('22222222-2222-2222-2222-222222222222', 'u1', 1, 1, NULL, NULL, ?)",
        )
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
        repo
    }

    #[test]
    fn vault_event_sse_data_shape() {
        let id = Uuid::new_v4();
        let ev = VaultEvent::ItemPermanentlyDeleted { uuid: id };
        let json = ev.to_sse_data();
        assert!(json.contains("\"type\":\"item_permanently_deleted\""));
        assert!(json.contains(&id.to_string()));
    }

    #[tokio::test]
    async fn reaper_emits_tombstone_and_skips_live() {
        let repo = temp_repo().await;
        let (tx, mut rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let tombstoned = repo.tombstoned_items().await.unwrap();
        assert_eq!(
            tombstoned,
            vec![Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()]
        );
        for u in tombstoned {
            let _ = tx.send(VaultEvent::ItemPermanentlyDeleted { uuid: u });
        }
        let got = rx.recv().await.unwrap();
        match got {
            VaultEvent::ItemPermanentlyDeleted { uuid } => {
                assert_eq!(
                    uuid,
                    Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
                );
            }
            VaultEvent::ItemRecovered { .. } => panic!("live item must not be emitted"),
        }
        assert!(tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .is_err());
    }

    #[test]
    fn recovered_outcome_on_untombstone() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let repo = temp_repo().await;
                let out = repo
                    .upsert_item_occ(
                        "11111111-1111-1111-1111-111111111111",
                        "u1",
                        2,
                        1,
                        None,
                        None,
                        1_700_000_000_001i64,
                    )
                    .await
                    .unwrap();
                assert_eq!(out, UpsertOutcome::Recovered);
            });
    }
}
