//! Desktop feature-tour (VTR-077) — native Rust port. GPUI has no
//! element-measurement primitive, so this is a centered-card sequence (same as
//! the first-run onboarding overlay), not pixel-anchored. Steps mirror
//! `docs/onboarding/tour.md`. Anchored tours on GPUI would need `bounds()` and
//! are a follow-up.

pub struct TourStep {
    pub id: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

pub const STEPS: &[TourStep] = &[
    TourStep {
        id: "vault",
        title: "Your Vault",
        body: "This is where your encrypted secrets live. Everything is decrypted only on your device.",
    },
    TourStep {
        id: "add-secret",
        title: "Add a secret",
        body: "Open your vault and add a login, note, or card. It is encrypted before it leaves your device.",
    },
    TourStep {
        id: "emergency-kit",
        title: "Emergency Kit",
        body: "Generate and store your Emergency Kit from MFA & security — your only way to recover the account if you forget your master password.",
    },
    TourStep {
        id: "audit",
        title: "Audit log",
        body: "See who accessed what and when. The audit log is your security trail.",
    },
];
