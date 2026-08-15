//! Desktop first-run onboarding — native Rust re-implementation of the OnboardJS
//! flow used on web/extension/mobile. The shared step contract lives in
//! `docs/onboarding/spec.md`. OnboardJS cannot run in Rust, so this is a faithful
//! port: same 5 steps, copy, skippable behaviour, first-run-once + replay.
//!
//! Rendering happens in `desktop_view.rs` (it needs GPUI widgets + theme); this
//! module owns the step data and the first-run persistence file.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One onboarding step. `skippable` mirrors the spec (steps 3 & 4 only).
pub struct OnboardingStep {
    pub id: &'static str,
    pub title: &'static str,
    pub body: &'static str,
    pub skippable: bool,
}

/// The 5 steps, in order. Copy is the contract in `docs/onboarding/spec.md`.
pub const STEPS: &[OnboardingStep] = &[
    OnboardingStep {
        id: "welcome",
        title: "Welcome to Vautr",
        body: "Vautr is a zero-knowledge vault: your secrets are encrypted on your device \
               and the server never sees them. Let's set up the essentials in about a minute.",
        skippable: false,
    },
    OnboardingStep {
        id: "create-vault",
        title: "Create your first vault",
        body: "A vault (project) groups your secrets. You can create more later.",
        skippable: false,
    },
    OnboardingStep {
        id: "emergency-kit",
        title: "Save your Emergency Kit",
        body: "The Emergency Kit lets you recover your account if you forget your master \
               password. It is generated and encrypted locally — store it somewhere safe \
               (password manager, printed copy).",
        skippable: true,
    },
    OnboardingStep {
        id: "add-secret",
        title: "Add your first secret",
        body: "Open your vault and add a login, note, or card. Everything is encrypted \
               before it leaves your device.",
        skippable: true,
    },
    OnboardingStep {
        id: "done",
        title: "You're all set",
        body: "That's the core loop: vault → Emergency Kit → secrets. You can replay this \
               tour anytime from Settings.",
        skippable: false,
    },
];

#[derive(Serialize, Deserialize, Default)]
struct OnboardingFlag {
    seen_v1: bool,
}

fn flag_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let mut p = PathBuf::from(home);
    p.push(".config");
    p.push("vautr");
    p.push("onboarding.json");
    Some(p)
}

/// Whether the first-run flow has already been completed.
pub fn load_seen() -> bool {
    let Some(path) = flag_path() else {
        return false;
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str::<OnboardingFlag>(&s)
            .map(|f| f.seen_v1)
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Persist that the first-run flow has been completed.
pub fn mark_seen() {
    let Some(path) = flag_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let data = OnboardingFlag { seen_v1: true };
    if let Ok(s) = serde_json::to_string_pretty(&data) {
        let _ = std::fs::write(&path, s);
    }
}
