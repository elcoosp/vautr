//! Vautr desktop theme — "The Vault Ledger".
//!
//! Single source of truth for the desktop surface's colours, mirroring the
//! token contract in `apps/web/src/index.css` (and extension/mobile) so all four
//! clients render the same world. Dark is the operating default; the palette is
//! graphite-and-ink with one confident emerald-teal accent, hairline borders,
//! flat surfaces, and tinted (never pure grey) neutrals.
//!
//! GPUI consumes raw `Rgba`, so the desktop encodes the contract as named
//! constants built from the same hex→RGBA math as `gpui::rgb`, but
//! const-compatible (no function calls in `const`).

use gpui::Rgba;

/// Build an opaque `Rgba` from a `0xRRGGBB` hex value (mirrors `gpui::rgb`).
const fn c(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// Deep canvas. Mirrors web `--background` oklch(0.16 0.02 255) → #080e16.
pub const BG: Rgba = c(0x080e16);
/// Raised card / surface. Mirrors web `--card` oklch(0.195 0.018 255) → #0f151d.
pub const SURFACE: Rgba = c(0x0f151d);
/// Further-raised surface (hover/selection wells). Mirrors web `--accent`
/// oklch(0.26 0.02 255) → #1e252e.
pub const SURFACE_RAISED: Rgba = c(0x1e252e);
/// Hairline structure. Mirrors web `--border` oklch(0.27 0.018 255) → #21272f.
pub const BORDER: Rgba = c(0x21272f);
/// Primary ink. Mirrors web `--foreground` oklch(0.94 0.015 250) → #e4ecf5.
pub const TEXT: Rgba = c(0xe4ecf5);
/// Muted ink. Mirrors web `--muted-foreground` oklch(0.68 0.02 250) → #8f9aa4.
pub const TEXT_MUTED: Rgba = c(0x8f9aa4);
/// Dimmer secondary ink (tuned below muted-fg).
pub const TEXT_DIM: Rgba = c(0x6b7480);
/// Emerald-teal accent (brand primary). Mirrors `--primary` (oklch 0.70 0.11 175,
/// which resolves to #42b59a in CSS Color 4 — the value web/extension actually render).
pub const ACCENT: Rgba = c(0x42b59a);
/// Dark ink placed on the accent. Mirrors web `--primary-foreground`
/// oklch(0.16 0.02 250) → #070e16.
pub const ACCENT_INK: Rgba = c(0x070e16);
/// Destructive / error. Mirrors web `--destructive` oklch(0.66 0.17 22) → #e85f61.
pub const DANGER: Rgba = c(0xe85f61);
/// Destructive surface tint (tuned).
pub const DANGER_BG: Rgba = c(0x3c1517);
/// Destructive foreground text (tuned).
pub const DANGER_TEXT: Rgba = c(0xf2b4b5);
/// Warning amber. Mirrors oklch(0.80 0.12 75) → #ebb25f.
pub const WARN: Rgba = c(0xebb25f);
/// Success / verified green. Mirrors oklch(0.78 0.15 165) → #37d59f.
pub const SUCCESS: Rgba = c(0x37d59f);
