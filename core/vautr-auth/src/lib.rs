//! # vautr-auth
//!
//! Client-side OPAQUE registration and login state machines. The server never
//! receives the Master Password or any password equivalent (REQ-AUTH-02).
//!
//! Implemented surfaces:
//! - `state` — registration/login client state machine (api.md §3).
//! - `error` — `AuthError` (data.md §8.4).
//! - `session` — bearer token handling after successful login.

pub mod state;
pub mod error;
pub mod session;
