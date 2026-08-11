//! Emergency Kit PDF rendering.
//!
//! Spec: docs/architecture/emergency-recovery-account.md §3.1.
//!
//! The Emergency Kit is generated **locally on the client device** and contains
//! the Vautr logo reference (wordmark), clear physical-storage instructions, the
//! user's email address, and the 24-word BIP-39 Recovery Key in readable text.
//!
//! # BR-7 — No QR code / no machine-scannable representation
//! This module renders the Recovery Key **only** as human-readable words. It must
//! never emit a QR code, barcode, or any machine-scannable encoding of the key.
//! BIP-39 words are the deliberate anti-exfiltration design: a QR code would let
//! an attacker photograph the key from a distance in seconds (§3.1).

use std::fmt;

use printpdf::{BuiltinFont, Color, Mm, PdfDocument, Rgb};

/// An error raised while rendering the Emergency Kit PDF.
#[derive(Debug)]
pub enum PdfError {
    /// The printpdf backend failed while building the document.
    Render(String),
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdfError::Render(msg) => write!(f, "pdf render error: {msg}"),
        }
    }
}

impl std::error::Error for PdfError {}

impl From<printpdf::Error> for PdfError {
    fn from(e: printpdf::Error) -> Self {
        PdfError::Render(e.to_string())
    }
}

/// Render an Emergency Kit PDF for a user's account.
///
/// * `email` — the account's email address.
/// * `mnemonic` — the 24-word BIP-39 Recovery Key (space-separated). Rendered in
///   readable text only (BR-7: no QR / machine-scannable representation).
///
/// Returns the raw PDF bytes.
pub fn render_emergency_kit_pdf(email: &str, mnemonic: &str) -> Result<Vec<u8>, PdfError> {
    // A4 in millimetres (printpdf origin is the bottom-left corner).
    let (page_w, page_h) = (Mm(210.0), Mm(297.0));
    let (doc, page, layer) =
        PdfDocument::new("Vautr Emergency Kit", page_w, page_h, "emergency-kit");

    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
    let font_reg = doc.add_builtin_font(BuiltinFont::Helvetica)?;

    let layer = doc.get_page(page).get_layer(layer);

    let brand = Rgb::new(0.08, 0.35, 0.62, None); // deep blue wordmark
    let ink = Rgb::new(0.12, 0.12, 0.14, None);

    // --- Logo reference (wordmark) ---
    layer.set_fill_color(Color::Rgb(brand.clone()));
    layer.use_text("Vautr", 34.0, Mm(20.0), Mm(268.0), &font_bold);

    layer.set_fill_color(Color::Rgb(ink.clone()));
    layer.use_text("Emergency Kit", 18.0, Mm(20.0), Mm(256.0), &font_bold);

    // --- Storage instructions (BR-7 §3.1) ---
    layer.use_text(
        "Store this page in a safe, offline location. This is the ONLY way to",
        10.0,
        Mm(20.0),
        Mm(236.0),
        &font_reg,
    );
    layer.use_text(
        "recover your Vautr vault if you ever forget your Master Password.",
        10.0,
        Mm(20.0),
        Mm(227.0),
        &font_reg,
    );
    layer.use_text(
        "Never photograph, scan, or store this Recovery Key digitally. Keep it",
        10.0,
        Mm(20.0),
        Mm(218.0),
        &font_reg,
    );
    layer.use_text(
        "separate from your devices. Anyone with these words can unlock your vault.",
        10.0,
        Mm(20.0),
        Mm(209.0),
        &font_reg,
    );

    // --- Account email ---
    layer.set_fill_color(Color::Rgb(brand.clone()));
    layer.use_text("Account", 12.0, Mm(20.0), Mm(192.0), &font_bold);
    layer.set_fill_color(Color::Rgb(ink.clone()));
    layer.use_text(email, 12.0, Mm(20.0), Mm(183.0), &font_reg);

    // --- Recovery Key (readable words only) ---
    layer.set_fill_color(Color::Rgb(brand.clone()));
    layer.use_text("Recovery Key (24 words)", 13.0, Mm(20.0), Mm(166.0), &font_bold);

    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    // Render as a readable numbered grid (4 columns x 6 rows).
    const COLS: usize = 4;
    const ROW_H: f32 = 9.0;
    const COL_W: f32 = 42.0;
    for (idx, word) in words.iter().enumerate() {
        let col = idx % COLS;
        let row = idx / COLS;
        let x = 20.0 + col as f32 * COL_W;
        let y = 150.0 - row as f32 * ROW_H;
        layer.set_fill_color(Color::Rgb(ink.clone()));
        layer.use_text(
            format!("{}. {}", idx + 1, word),
            10.0,
            Mm(x),
            Mm(y),
            &font_reg,
        );
    }

    // --- Footer ---
    layer.set_fill_color(Color::Rgb(Rgb::new(0.5, 0.5, 0.55, None)));
    layer.use_text(
        "Vautr — the password manager that belongs to you.",
        9.0,
        Mm(20.0),
        Mm(20.0),
        &font_reg,
    );

    Ok(doc.save_to_bytes()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MNEMONIC: &str = "legal winner thank year wave sausage worth useful legal winner thank yellow \
        letter advice cage absurd amount doctor acoustic avoid letter advice cage above \
        fruit mother list laundry dawn region above exhibit leaf garden require margin";

    #[test]
    fn emergency_kit_pdf_generates_bytes() {
        let bytes = render_emergency_kit_pdf("alice@example.com", MNEMONIC)
            .expect("pdf should render");
        assert!(!bytes.is_empty(), "PDF must produce non-empty bytes");
        // PDF magic header.
        assert!(bytes.starts_with(b"%PDF"), "must be a valid PDF (got {:?})", &bytes[..5]);
    }

    #[test]
    fn emergency_kit_contains_readable_words() {
        let bytes = render_emergency_kit_pdf("alice@example.com", MNEMONIC)
            .expect("pdf should render");
        // Text is emitted as hex-encoded content streams (`<...>` tokens), which
        // proves the key is rendered as real, selectable text (not an image or a
        // machine-scannable blob). Lowercase both sides and search for the hex
        // encoding of each word.
        let text = String::from_utf8_lossy(&bytes).to_lowercase();
        let hex = |s: &str| s.bytes().map(|b| format!("{b:02x}")).collect::<String>();
        for word in MNEMONIC.split_whitespace() {
            assert!(
                text.contains(&hex(word)),
                "word {word:?} must appear as readable text"
            );
        }
        assert!(
            text.contains(&hex("alice@example.com")),
            "email must be present"
        );
    }

    /// BR-7: the Emergency Kit must NOT contain a QR code or any machine-
    /// scannable representation of the Recovery Key.
    #[test]
    fn emergency_kit_emits_no_qr_or_barcode() {
        let bytes = render_emergency_kit_pdf("alice@example.com", MNEMONIC)
            .expect("pdf should render");
        let text = String::from_utf8_lossy(&bytes).to_lowercase();
        // No QR/barcode operators or resource names.
        for marker in [
            "qrcode",
            "qr code",
            "barcode",
            "/dqfont",
            "matrix",
            "pdf417",
        ] {
            assert!(
                !text.contains(marker),
                "must not emit machine-scannable marker {marker:?}"
            );
        }
    }
}
