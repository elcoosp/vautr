//! # vautr-wasm
//!
//! wasm-bindgen bindings for the web client and (a crypto-only subset for) the
//! browser-extension service worker (ADR-005). The full client runs in a Web
//! Worker; the extension SW uses `--target nodejs` of **only** `vautr-crypto`.

pub mod auth;
pub mod client;
pub mod sharing;

// VTRFIX-FEAT-H03 (tracked): the competitor-import parsers live in
// `vautr-import`. Exposing them through this wasm crate requires adding
// `vautr-import` as a dependency (it pulls sea-orm + sqlx, which do not
// compile to wasm32). A translate-only surface would need a feature split in
// `vautr-import` first. Tracked in docs/issues/VTRFIX-LOG.md.
