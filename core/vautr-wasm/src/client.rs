//! wasm-bindgen `VautrClient` surface for web/extension. ADR-003/005.
//! Secrets cross only as opaque handles; `read_secret` is desktop-gated.

#![allow(unused)]

use wasm_bindgen::prelude::*;

/// Opaque handle to a decrypted secret (u64 as string across JS). ADR-003.
pub type SecretHandle = u64;

/// Web-facing client stub. Real impl wraps `vautr_app_state::VautrClient`.
#[wasm_bindgen]
pub struct WebClient;
