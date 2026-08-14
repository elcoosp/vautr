//! Opaque secret handles + the Safety Reaper integration (client.md §2-3).
//!
//! Secrets are decrypted on demand and held in Rust memory behind an opaque
//! `u64` handle. The Safety Reaper (vautr_sync::reaper) zeroizes idle handles
//! after 60s; `perform_action` atomically marks a handle `InUse` so an in-flight
//! OS clipboard/autofill call is never reaped mid-copy.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use vautr_sync::reaper::{spawn_reaper, HandleTable};
use zeroize::Zeroizing;

/// An action delegated to the Platform Adapter. The secret string itself never
/// crosses the FFI boundary into JS — only the native adapter receives it
/// (client.md §2 / data.md §6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreAction {
    /// Copy the revealed secret to the OS clipboard.
    CopyToClipboard { handle: u64 },
    /// Autofill the revealed secret into the focused field.
    Autofill { handle: u64 },
    /// Render the revealed secret in the native overlay view (Kotlin/Swift).
    /// The plaintext is delivered only to the native `PlatformActionHandler`
    /// (never JS); the overlay view renders it from `on_action` and calls
    /// `release_secret` on unmount (VTR-048, ADR-003).
    RenderInOverlay { handle: u64 },
}

/// Platform capability trait. Implemented by the FFI/WASM layer (GPUI,
/// Turbo Module, Web Worker). The core calls this with the plaintext secret so
/// it never enters the JS/Kotlin heap (client.md §2, ADR-003/005).
pub trait PlatformAdapter: Send + Sync {
    /// Service a copy/autofill action with the decrypted secret bytes.
    fn service_action(&self, action: CoreAction, secret: &[u8]);
}

/// Owns the live secret handle table and the reaper task. `reveal` stores a
/// decrypted secret behind an opaque handle; the reaper zeroizes it when idle.
#[derive(Clone)]
pub struct SecretStore {
    handles: HandleTable,
    next: Arc<AtomicU64>,
}

impl SecretStore {
    /// Create the store and spawn the Safety Reaper (REQ-SECRET-05).
    pub fn new() -> Self {
        let handles = HandleTable::default();
        spawn_reaper(handles.clone());
        Self {
            handles,
            next: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Store `secret` behind a fresh opaque handle. Returns the handle id.
    pub fn reveal(&self, secret: Zeroizing<Vec<u8>>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let h = vautr_sync::reaper::SecretHandle::new(id, (*secret).clone());
        self.handles.lock().unwrap().insert(id, h);
        id
    }

    /// Access the secret bytes for `handle`, touching its idle timer. Returns
    /// `None` if the handle was already reaped/never existed.
    pub fn access(&self, handle: u64) -> Option<Zeroizing<Vec<u8>>> {
        let mut guard = self.handles.lock().unwrap();
        guard.get_mut(&handle).map(|h| {
            let bytes = h.access().to_vec();
            Zeroizing::new(bytes)
        })
    }

    /// Release `handle`, immediately zeroizing its memory (client.md §3 step 3).
    pub fn release(&self, handle: u64) {
        self.handles.lock().unwrap().remove(&handle);
    }

    /// True if `handle` is currently live.
    pub fn contains(&self, handle: u64) -> bool {
        self.handles.lock().unwrap().contains_key(&handle)
    }
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new()
    }
}
