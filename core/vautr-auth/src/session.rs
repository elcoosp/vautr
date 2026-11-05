//! OPAQUE session handling. api.md §3 — bearer token returned by login/finish.

#![allow(unused)]

/// Authenticated session token (Bearer) returned after OPAQUE login.
///
/// `Debug` is implemented manually so the bearer token never leaks into logs,
/// crash reports, or `{:?}` dumps of session state.
#[derive(Clone)]
pub struct Session {
    pub token: String,
    pub expires_at: i64,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("token", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_token() {
        let s = Session {
            token: "super-secret-bearer-token".to_string(),
            expires_at: 1234567890,
        };
        let dbg = format!("{s:?}");
        assert!(dbg.contains("[REDACTED]"), "debug output: {dbg}");
        assert!(
            !dbg.contains("super-secret"),
            "token leaked in debug: {dbg}"
        );
    }
}
