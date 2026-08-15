//! Desktop feature-tour (VTR-078) — native Rust port with true element
//! anchoring. Unlike the VTR-077 centered-card version, the tour is rendered as
//! an overlay *on top of* the live app (the underlying view stays mounted), and
//! the spotlight is anchored to the relevant surface: each step names the
//! `Section` it points at, the tour switches to that section so the feature is
//! visible behind the dimmed scrim, and the highlight ring + card are positioned
//! from the measured window/content bounds (GPUI `window().bounds()`). This is
//! the GPUI analogue of the web `getBoundingClientRect()` spotlight.

pub struct TourStep {
    pub id: &'static str,
    /// Section the tour navigates to so the anchored feature is on-screen.
    pub section: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

impl TourStep {
    /// Resolve the step's target section to a `Section` (mirrors the desktop
    /// `Section` enum; kept as a string here to avoid a cross-module enum dep).
    pub fn resolve_section(&self) -> crate::desktop_view::Section {
        crate::desktop_view::Section::from_tour_id(self.section)
    }
}

pub const STEPS: &[TourStep] = &[
    TourStep {
        id: "vault",
        section: "vault",
        title: "Your Vault",
        body: "This is where your encrypted secrets live. Everything is decrypted only on your device.",
    },
    TourStep {
        id: "add-secret",
        section: "vault",
        title: "Add a secret",
        body: "Open your vault and add a login, note, or card. It is encrypted before it leaves your device.",
    },
    TourStep {
        id: "emergency-kit",
        section: "mfa",
        title: "Emergency Kit",
        body: "Generate and store your Emergency Kit from MFA & security — your only way to recover the account if you forget your master password.",
    },
    TourStep {
        id: "audit",
        section: "mfa",
        title: "Audit log",
        body: "See who accessed what and when. The audit log is your security trail.",
    },
];
