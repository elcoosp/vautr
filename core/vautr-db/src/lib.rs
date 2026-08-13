//! # vautr-db
//!
//! Client-side SQLite persistence: SeaORM 2.0 entities, FTS5 index, and the
//! transaction boundaries defined in [`docs/architecture/db-contract.md`].
//!
//! The local DB is the single source of offline truth (hot/cold split, WAL,
//! `synchronous=NORMAL`, STRICT tables). Implemented surfaces:
//! - `entity`  — SeaORM entities (item_overview, item_payload, sync_meta,
//!               local_blacklist, quarantine, DashMapState enum).
//! - `migrate` — schema bootstrap + FTS5 triggers (db-contract §3–4).
//! - `txn`     — save / apply_sync_batch / persist_dashmap / reaper_reset_ttl
//!               (db-contract §5).
//! - `query`   — FTS5/recent/overview read queries (db-contract §4).
//! - `search`  — FTS5 query preparation (stopwords, wildcards, VTR-055).

pub mod entity;
pub mod migrate;
pub mod query;
pub mod search;
pub mod txn;
