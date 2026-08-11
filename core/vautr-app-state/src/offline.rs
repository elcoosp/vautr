//! Offline-sync mutation queue (VTR-047 / core.md §2 offline story).
//!
//! When the sync transport is unavailable, mutations are queued locally rather
//! than rejected. Once connectivity returns, the orchestrator drains the queue
//! through the normal `push_batch` path. The queue is append-only and drained
//! FIFO so ordering is preserved for the server's OCC versioning.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use vautr_domain::DomainModel;

/// A locally-queued mutation awaiting a later push.
#[derive(Clone, Debug)]
pub enum QueuedMutation {
    /// Upsert an item with its pre-encrypted payload.
    Save { item: DomainModel, payload: Vec<u8> },
    /// Delete an item.
    Delete { uuid: Uuid },
}

/// Thread-safe FIFO queue of offline mutations.
#[derive(Clone, Default)]
pub struct OfflineQueue {
    queue: Arc<Mutex<VecDeque<QueuedMutation>>>,
}

impl OfflineQueue {
    /// New empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue a mutation. Returns the queue length after enqueue.
    pub fn push(&self, m: QueuedMutation) -> usize {
        let mut g = self.queue.lock().unwrap();
        g.push_back(m);
        g.len()
    }

    /// Number of queued mutations.
    pub fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// True when nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drain the queue FIFO (used by `flush_offline_queue`).
    pub fn drain(&self) -> Vec<QueuedMutation> {
        let mut g = self.queue.lock().unwrap();
        g.drain(..).collect()
    }
}
