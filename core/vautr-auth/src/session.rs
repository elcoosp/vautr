//! OPAQUE session handling. api.md §3 — bearer token returned by login/finish.

#![allow(unused)]

/// Authenticated session token (Bearer) returned after OPAQUE login.
#[derive(Clone, Debug)]
pub struct Session {
    pub token: String,
    pub expires_at: i64,
}
