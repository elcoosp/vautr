//! # vautr-wasm
//!
//! wasm-bindgen bindings for the web client and (a crypto-only subset for) the
//! browser-extension service worker (ADR-005). The full client runs in a Web
//! Worker; the extension SW uses `--target nodejs` of **only** `vautr-crypto`.

pub mod auth;
pub mod client;
