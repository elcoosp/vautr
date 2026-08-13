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
//! - `sharing`      — sharing PKI transport + in-memory relay + group store.
//! - `file_transfer`— `FileTransferWorker` (throttled multipart upload/download).
//! - `project_transport` — projects + project-scoped secret metadata transport.
//! - `recovery`     — Recovery Key auth gate + proof-of-possession (§2-4).
//! - `offline`      — offline mutation queue (VTR-047).

pub mod epoch;
pub mod event_bus;
pub mod file_transfer;
pub mod handles;
pub mod offline;
pub mod orchestrator;
pub mod project_transport;
pub mod recovery;
pub mod sharing;
pub mod sync_transport;
pub mod worker;

pub use orchestrator::SecondFactorMethod;
pub use orchestrator::VautrClient;
pub use project_transport::{ProjectSecretSummary, ProjectSummary, ProjectTransportHandle};
