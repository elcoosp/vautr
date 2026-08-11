//! Safety Reaper: background zeroization of idle secret handles. REQ-SECRET-05.
//! Runs every 10s; zeroizes entries idle for > 60s.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A tracked secret handle. The `data` is zeroized on drop/eviction.
pub struct SecretHandle {
    pub id: u64,
    pub last_accessed: Instant,
    data: zeroize::Zeroizing<Vec<u8>>,
}

impl SecretHandle {
    /// Create a tracked handle for `data`.
    pub fn new(id: u64, data: Vec<u8>) -> Self {
        Self {
            id,
            last_accessed: Instant::now(),
            data: zeroize::Zeroizing::new(data),
        }
    }

    /// Touch the handle (resets its idle timer).
    pub fn access(&mut self) -> &[u8] {
        self.last_accessed = Instant::now();
        &self.data
    }
}

/// Shared, lock-protected handle table owned by `vautr-app-state`.
pub type HandleTable = Arc<Mutex<HashMap<u64, SecretHandle>>>;

/// Idle TTL before the reaper zeroizes a handle (REQ-SECRET-05).
pub const IDLE_TTL: Duration = Duration::from_secs(60);
/// Reaper tick interval.
pub const TICK: Duration = Duration::from_secs(10);

/// Spawn the reaper loop (tokio task). Zeroizes handles idle longer than
/// [`IDLE_TTL`] every [`TICK`].
pub fn spawn_reaper(table: HandleTable) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(TICK);
        loop {
            interval.tick().await;
            let now = Instant::now();
            if let Ok(mut guard) = table.lock() {
                let stale: Vec<u64> = guard
                    .iter()
                    .filter(|(_, h)| now.duration_since(h.last_accessed) > IDLE_TTL)
                    .map(|(id, _)| *id)
                    .collect();
                for id in stale {
                    // Removing the entry drops `SecretHandle`, zeroizing `data`.
                    guard.remove(&id);
                }
            }
        }
    })
}
