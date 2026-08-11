//! Epoch / Read-Only Gate management. REQ-AUTH-05, core.md §1.
//!
//! The client maintains a single `Arc<AtomicU64>` `sync_epoch` that increments
//! on every `vault_mode` transition. Mutations capture the epoch at enqueue
//! time; the `PersistenceWorker` verifies it still matches at commit time and
//! applies Context-Aware Resolution when it does not.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The client's vault mode. Drives the Read-Only Gate (core.md §1, §2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultMode {
    /// Writes allowed.
    ReadWrite,
    /// A key update was detected: local_gen < server_min_gen. Writes blocked.
    KeyUpdateRequired,
    /// A background key rotation is in progress. Writes allowed but the worker
    /// re-binds to the newly active SVK when the epoch changed (core.md §2.3).
    Migrating,
    /// Vault is locked; no operations permitted.
    Locked,
}

/// Shared epoch + vault-mode state. Cheap to clone (Arc).
#[derive(Clone)]
pub struct EpochState {
    sync_epoch: Arc<AtomicU64>,
    min_enc_key_gen: Arc<AtomicU64>,
}

impl EpochState {
    /// Fresh epoch state (epoch 0, no server key-gen known yet).
    pub fn new() -> Self {
        Self {
            sync_epoch: Arc::new(AtomicU64::new(0)),
            min_enc_key_gen: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Current sync epoch.
    pub fn current(&self) -> u64 {
        self.sync_epoch.load(Ordering::SeqCst)
    }

    /// The epoch value captured by a task at enqueue time.
    pub fn capture(&self) -> u64 {
        self.current()
    }

    /// Bump the epoch. Called on every `vault_mode` transition (core.md §1.1).
    /// Returns the new epoch value.
    pub fn bump(&self) -> u64 {
        self.sync_epoch.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Server's minimum required `enc_key_gen` (from sync_meta / server cursor).
    pub fn set_min_enc_key_gen(&self, gen: u64) {
        self.min_enc_key_gen.store(gen, Ordering::SeqCst);
    }

    /// Server's minimum required `enc_key_gen` (epoch gate source).
    pub fn current_min_gen(&self) -> u64 {
        self.min_enc_key_gen.load(Ordering::SeqCst)
    }

    /// Local `enc_key_gen` is the client's active vault key generation.
    /// Read-Only when the local generation lags the server's minimum.
    pub fn is_read_only(&self, local_gen: u64) -> bool {
        local_gen < self.min_enc_key_gen.load(Ordering::SeqCst)
    }

    /// Resolve the effective vault mode given a local `enc_key_gen`.
    pub fn vault_mode(&self, local_gen: u64) -> VaultMode {
        if self.is_read_only(local_gen) {
            VaultMode::KeyUpdateRequired
        } else {
            VaultMode::ReadWrite
        }
    }

    /// Commit-time check (core.md §1.3 / §2.2).
    /// Returns `(matched, mode)`:
    /// - `matched == true`  → proceed with dynamic key binding + commit.
    /// - `matched == false` → caller must apply Context-Aware Resolution using `mode`:
    ///   * `KeyUpdateRequired` → abort + revert optimistic UI.
    ///   * `ReadWrite` / `Migrating` → safe: re-fetch active SVK, re-encrypt, commit.
    pub fn verify_commit(&self, task_epoch: u64, local_gen: u64) -> (bool, VaultMode) {
        let mode = self.vault_mode(local_gen);
        (task_epoch == self.current(), mode)
    }
}

impl Default for EpochState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_when_local_lags() {
        let st = EpochState::new();
        st.set_min_enc_key_gen(3);
        assert!(st.is_read_only(2));
        assert!(!st.is_read_only(3));
        assert!(!st.is_read_only(4));
        assert_eq!(st.vault_mode(2), VaultMode::KeyUpdateRequired);
        assert_eq!(st.vault_mode(3), VaultMode::ReadWrite);
    }

    #[test]
    fn epoch_bump_changes_match() {
        let st = EpochState::new();
        let e0 = st.capture();
        assert!(st.verify_commit(e0, 5).0);
        let e1 = st.bump();
        assert_eq!(e1, e0 + 1);
        assert!(!st.verify_commit(e0, 5).0);
        assert!(st.verify_commit(e1, 5).0);
    }
}
