//! # vautr-app-state
//!
//! Top-level client orchestrator. Owns the event bus (`VaultStateUpdate` stream,
//! data.md §6.3), the `PersistenceWorker` (save/delete commands with epoch
//! gating, REQ-SYNC-06), and epoch/Read-Only-Gate management (REQ-AUTH-05).
//!
//! Implemented surfaces:
//! - `orchestrator` — wires crypto+auth+keyring+sync+db into one `VautrClient`.
//! - `event_bus`    — `VaultStateUpdate` broadcast (data.md §6.3).
//! - `worker`       — `PersistenceWorker` (SaveCommand/DeleteCommand + sync_epoch).
//! - `epoch`        — Read-Only Gate logic when `local_gen < min_enc_key_gen`.

pub mod epoch;
pub mod event_bus;
pub mod orchestrator;
pub mod worker;
