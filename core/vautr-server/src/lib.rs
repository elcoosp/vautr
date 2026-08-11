//! # vautr-server
//!
//! Untrusted, zero-knowledge blob store. Axum HTTP API enforcing OCC (ADR-004)
//! and epoch gating (REQ-API-01/02/03). SQLite WAL (ADR-001). Handlers mirror
//! [`docs/architecture/api.md`].
//!
//! Implemented surfaces:
//! - `main`      — binary entrypoint (Axum + middleware stack, arch-design §3.3).
//! - `handlers`  — /auth, /sync, /items, /account.
//! - `repository`— sqlx queries with strict user_id scoping (⚠ server DDL TBD).
//! - `middleware`— tracing, rate limiting, auth extractor, CORS.

pub mod db;
pub mod handlers;
pub mod repository;
pub mod middleware;
