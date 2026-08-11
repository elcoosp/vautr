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

/// Deep graphite-blue canvas. Mirrors `--background` (oklch 0.16 0.02 255).
pub const BG: Rgba = c(0x12151c);
/// Raised card / surface. Mirrors `--card`.
pub const SURFACE: Rgba = c(0x181c25);
/// Further-raised surface (hover/selection wells). Mirrors `--accent`/raised.
pub const SURFACE_RAISED: Rgba = c(0x20242e);
/// Hairline structure. Mirrors `--border`.
pub const BORDER: Rgba = c(0x2a3140);
/// Primary ink. Mirrors `--foreground`.
pub const TEXT: Rgba = c(0xe7eaf0);
/// Muted ink. Mirrors `--muted-foreground`.
pub const TEXT_MUTED: Rgba = c(0x9aa3b4);
/// Dimmer secondary ink.
pub const TEXT_DIM: Rgba = c(0x7b8494);
/// Emerald-teal accent (brand primary). Mirrors `--primary` (oklch 0.70 0.11 175).
pub const ACCENT: Rgba = c(0x2bbca0);
/// Dark ink placed on the accent (primary-foreground).
pub const ACCENT_INK: Rgba = c(0x0c1713);
/// Destructive / error. Mirrors `--destructive`.
pub const DANGER: Rgba = c(0xe5484d);
/// Destructive surface tint.
pub const DANGER_BG: Rgba = c(0x3a1416);
/// Destructive foreground text.
pub const DANGER_TEXT: Rgba = c(0xf8a3a3);
/// Warning amber.
pub const WARN: Rgba = c(0xf0b429);
/// Success / verified green.
pub const SUCCESS: Rgba = c(0x3dd68c);
