//! # User-controlled telemetry consent gate.
//!
//! Telemetry is strictly **opt-in** (spec §4). All telemetry collection and
//! heartbeat transmission must be gated behind this consent. The gate is
//! purely an in-memory decision; callers are responsible for persisting the
//! user's choice (e.g. in app settings) and restoring it on startup.
//!
//! The default state is **disabled**: no telemetry is collected until the user
//! explicitly grants consent.

use serde::{Deserialize, Serialize};

/// The user's telemetry consent state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Consent {
    /// Telemetry is disabled. This is the default and safe state.
    Denied,
    /// The user explicitly opted in to anonymous telemetry.
    Granted,
}

impl Default for Consent {
    fn default() -> Self {
        Consent::Denied
    }
}

/// A user-controlled opt-in/opt-out gate for telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ConsentGate {
    state: Consent,
}

impl ConsentGate {
    /// Create a gate with the safe default: telemetry disabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a gate from an explicit consent state.
    pub fn from_consent(state: Consent) -> Self {
        Self { state }
    }

    /// Whether telemetry is currently enabled.
    ///
    /// All telemetry recording and heartbeat sending MUST be guarded by this
    /// returning `true`.
    pub fn enabled(&self) -> bool {
        self.state == Consent::Granted
    }

    /// Whether telemetry is currently disabled.
    pub fn denied(&self) -> bool {
        !self.enabled()
    }

    /// The current consent state.
    pub fn state(&self) -> Consent {
        self.state
    }

    /// Opt the user in. Returns the new state.
    pub fn grant(&mut self) -> Consent {
        tracing::info!("telemetry consent granted");
        self.state = Consent::Granted;
        self.state
    }

    /// Opt the user out (also the default on first launch). Returns the new state.
    pub fn revoke(&mut self) -> Consent {
        tracing::info!("telemetry consent revoked");
        self.state = Consent::Denied;
        self.state
    }

    /// Clear any telemetry that was collected while consent was granted.
    ///
    /// Callers pass a closure that discards the in-memory metrics buffer, so a
    /// revoked user leaves no residual data to be transmitted later.
    pub fn reset<F>(&mut self, clear_buffer: F)
    where
        F: FnOnce(),
    {
        clear_buffer();
        self.state = Consent::Denied;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_denied() {
        let gate = ConsentGate::new();
        assert!(gate.denied());
        assert!(!gate.enabled());
        assert_eq!(gate.state(), Consent::Denied);
    }

    #[test]
    fn grant_enables_and_revoke_disables() {
        let mut gate = ConsentGate::new();
        assert_eq!(gate.grant(), Consent::Granted);
        assert!(gate.enabled());
        assert!(!gate.denied());

        assert_eq!(gate.revoke(), Consent::Denied);
        assert!(gate.denied());
        assert!(!gate.enabled());
    }

    #[test]
    fn reset_discards_buffer_and_denies() {
        let mut gate = ConsentGate::new();
        gate.grant();

        let mut cleared = false;
        gate.reset(|| cleared = true);

        assert!(cleared, "buffer was discarded");
        assert!(gate.denied());
    }

    #[test]
    fn serde_roundtrip() {
        let gate = ConsentGate::from_consent(Consent::Granted);
        let json = serde_json::to_string(&gate).expect("serializes");
        let back: ConsentGate = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, gate);
        assert!(back.enabled());
    }
}
