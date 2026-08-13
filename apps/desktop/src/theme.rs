//! Vautr desktop theme — "The Vault Ledger".
//!
//! This module is now a thin re-export of the generated token set in
//! `theme_tokens.rs`, which is produced by `packages/design-tokens/scripts/build.mjs`
//! from the single source of truth `tokens.json` (+ `gpui-map.json`). Edit the
//! tokens, run `pnpm --filter @vautr/design-tokens build:tokens`, and every
//! client updates. Do not hand-edit `theme_tokens.rs`.
//!
//! The named consts (`BG`, `SURFACE`, `TEXT`, `ACCENT`, `DANGER_BG`, …) used
//! across `desktop_view.rs` are all re-exported below.

#[path = "theme_tokens.rs"]
mod theme_tokens;

pub use theme_tokens::*;
