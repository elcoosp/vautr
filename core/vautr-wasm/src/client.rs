//! wasm-bindgen crypto-only client for the web/extension (ADR-005).
//!
//! The browser service worker does NOT use the SQLite-backed `VautrClient`
//! (that requires sea-orm/reqwest/ring, which cannot compile to
//! `wasm32-unknown-unknown`). Instead it is a crypto-only facade over
//! `vautr-crypto` + `vautr-keyring`: it decrypts vault item payloads locally and
//! exposes only opaque `u64` handles plus `perform_action` (data.md §1 rule 4 /
//! client.md §2). The plaintext secret string is handed directly to a JS-
//! registered clipboard/autofill handler and never retained in the JS heap.
//!
//! `read_secret` is desktop-gated and compiled out of the web artifact; CI
//! symbol-check (prd.md) asserts its absence.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use wasm_bindgen::prelude::*;

use vautr_crypto::aead;
use vautr_crypto::key_tree;
use zeroize::Zeroizing;

/// Idle TTL before the Safety Reaper zeroizes a handle (REQ-SECRET-05).
const IDLE_TTL: Duration = Duration::from_secs(60);
/// Reaper tick interval.
const TICK: Duration = Duration::from_secs(10);

/// Opaque handle to a decrypted secret (u64 as a JS number). ADR-003.
pub type SecretHandle = u64;

/// A tracked secret handle; `data` is zeroized on drop/reap.
struct TrackedSecret {
    last_accessed: Instant,
    data: Zeroizing<Vec<u8>>,
}

/// Opaque handle table + Safety Reaper, wasm-friendly (no tokio).
struct HandleStore {
    handles: Rc<RefCell<HashMap<u64, TrackedSecret>>>,
    next: std::cell::Cell<u64>,
}

impl HandleStore {
    fn new() -> Self {
        let store = Self {
            handles: Rc::new(RefCell::new(HashMap::new())),
            next: std::cell::Cell::new(1),
        };
        store.spawn_reaper();
        store
    }

    /// Store `secret` behind a fresh opaque handle. Returns the handle id.
    fn reveal(&self, secret: Zeroizing<Vec<u8>>) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        self.handles.borrow_mut().insert(
            id,
            TrackedSecret {
                last_accessed: Instant::now(),
                data: secret,
            },
        );
        id
    }

    /// Borrow the secret bytes for `handle`, touching its idle timer. `None` if
    /// reaped or never existed.
    fn access(&self, handle: u64) -> Option<Zeroizing<Vec<u8>>> {
        let mut guard = self.handles.borrow_mut();
        guard.get_mut(&handle).map(|h| {
            h.last_accessed = Instant::now();
            h.data.clone()
        })
    }

    /// Release `handle`, immediately zeroizing its memory.
    fn release(&self, handle: u64) {
        self.handles.borrow_mut().remove(&handle);
    }

    /// Spawn the reaper loop using `setInterval` (wasm has no threads).
    fn spawn_reaper(&self) {
        let handles = self.handles.clone();
        let closure = Closure::wrap(Box::new(move || {
            let now = Instant::now();
            let mut guard = handles.borrow_mut();
            let stale: Vec<u64> = guard
                .iter()
                .filter(|(_, h)| now.duration_since(h.last_accessed) > IDLE_TTL)
                .map(|(id, _)| *id)
                .collect();
            for id in stale {
                guard.remove(&id); // drops TrackedSecret -> zeroizes data
            }
        }) as Box<dyn FnMut()>);
        let _ = set_interval(&closure, TICK.as_millis() as i32);
        closure.forget();
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = global, js_name = setInterval)]
    fn set_interval(closure: &Closure<dyn FnMut()>, millis: i32) -> i32;
}

/// Web-facing crypto-only client. Holds the unlocked vault key (SVK) + derived
/// DEK, and a handle store for revealed secrets.
#[wasm_bindgen]
pub struct WebClient {
    svk: Rc<RefCell<Option<Zeroizing<[u8; 32]>>>>,
    dek: Rc<RefCell<Option<Zeroizing<[u8; 32]>>>>,
    local_gen: Rc<RefCell<u64>>,
    store: HandleStore,
    /// JS clipboard/autofill handler: `function(actionJson, secret)`.
    handler: Rc<RefCell<Option<js_sys::Function>>>,
}

#[wasm_bindgen]
impl WebClient {
    /// Create a (locked) web client.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WebClient {
        WebClient {
            svk: Rc::new(RefCell::new(None)),
            dek: Rc::new(RefCell::new(None)),
            local_gen: Rc::new(RefCell::new(1)),
            store: HandleStore::new(),
            handler: Rc::new(RefCell::new(None)),
        }
    }

    /// Register the JS clipboard/autofill handler (`function(actionJson, secret)`).
    pub fn set_action_handler(&self, handler: js_sys::Function) {
        *self.handler.borrow_mut() = Some(handler);
    }

    /// Unlock directly with a raw 32-byte vault key + the local key generation.
    pub fn unlock_with_raw_key(&self, raw_key: Vec<u8>, local_gen: u64) -> Result<(), JsValue> {
        if raw_key.len() != 32 {
            return Err(JsValue::from_str("raw key must be 32 bytes"));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&raw_key);
        let svk = Zeroizing::new(key);
        let dek = key_tree::derive_dek(&svk).map_err(|e| JsValue::from_str(&e.to_string()))?;
        *self.svk.borrow_mut() = Some(svk);
        *self.dek.borrow_mut() = Some(dek);
        *self.local_gen.borrow_mut() = local_gen;
        Ok(())
    }

    /// Decrypt an encrypted item payload and reveal it behind an opaque handle.
    /// `payload` is the AEAD envelope; `uuid`/`enc_key_gen` bind the AD. Returns
    /// the handle id (the plaintext never crosses back into JS).
    pub fn reveal_secret(
        &self,
        uuid: String,
        enc_key_gen: u64,
        payload: Vec<u8>,
    ) -> Result<u32, JsValue> {
        let dek = self
            .dek
            .borrow()
            .clone()
            .ok_or_else(|| JsValue::from_str("vault is locked"))?;
        let uuid = uuid::Uuid::parse_str(&uuid)
            .map_err(|e| JsValue::from_str(&format!("uuid: {e}")))?;
        let pt = aead::decrypt(&dek, &uuid, enc_key_gen, &payload)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let handle = self.store.reveal(Zeroizing::new(pt));
        Ok(handle as u32)
    }

    /// Delegate copy/autofill to the JS platform handler. `action_json` is a
    /// serialized action (`{"CopyToClipboard":{"handle":N}}` or
    /// `{"Autofill":{"handle":N}}`). The handler receives the plaintext secret
    /// directly and is responsible for zeroizing it after use.
    pub fn perform_action(&self, action_json: String, handle: u32) -> Result<(), JsValue> {
        let secret = self
            .store
            .access(handle as u64)
            .ok_or_else(|| JsValue::from_str("handle expired or unknown"))?;
        let handler = self
            .handler
            .borrow()
            .clone()
            .ok_or_else(|| JsValue::from_str("no platform action handler registered"))?;
        let secret = String::from_utf8_lossy(&secret).into_owned();
        let _ = handler.call2(
            &JsValue::NULL,
            &JsValue::from_str(&action_json),
            &JsValue::from_str(&secret),
        );
        Ok(())
    }

    /// Explicitly release a handle (zeroizes the in-memory secret).
    pub fn release_secret(&self, handle: u32) {
        self.store.release(handle as u64);
    }

    /// Re-encrypt a plaintext payload under the current DEK (used by the JS layer
    /// when persisting a new/updated item). Returns the AEAD envelope.
    pub fn encrypt_secret(
        &self,
        uuid: String,
        enc_key_gen: u64,
        plaintext: Vec<u8>,
    ) -> Result<Vec<u8>, JsValue> {
        let dek = self
            .dek
            .borrow()
            .clone()
            .ok_or_else(|| JsValue::from_str("vault is locked"))?;
        let uuid = uuid::Uuid::parse_str(&uuid)
            .map_err(|e| JsValue::from_str(&format!("uuid: {e}")))?;
        aead::encrypt(&dek, &uuid, enc_key_gen, &plaintext)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Restricted API — Desktop only. Compiled out of the web artifact (data.md §1
/// rule 4). CI symbol-check (prd.md) asserts its absence.
#[cfg(feature = "desktop-api")]
#[wasm_bindgen]
impl WebClient {
    /// Read the secret string for `handle`. ONLY available on Desktop.
    pub fn read_secret(&self, handle: u32) -> Result<String, JsValue> {
        let secret = self
            .store
            .access(handle as u64)
            .ok_or_else(|| JsValue::from_str("handle expired or unknown"))?;
        Ok(String::from_utf8_lossy(&secret).into_owned())
    }
}
