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

/// The canonical OpenAPI 3 spec (api.md), embedded at compile time so the
/// server can serve it at `GET /openapi.json` (VTR-010 TDD #2) and the SPA /
/// external tooling can fetch the contract directly from a running instance.
///
/// Source of truth: `packages/api-contract/openapi.json`. Re-run the contract
/// generator after editing request/response shapes so this stays in sync.
pub const OPENAPI_SPEC: &str = include_str!("../../../packages/api-contract/openapi.json");
