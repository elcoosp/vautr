//! # vautr-desktop
//!
//! The native desktop client, built with GPUI (Zed's GPU-accelerated UI
//! framework). This is the **only** package in the monorepo that may hold
//! `read_secret` — it enables the `desktop-api` feature on `vautr-app-state`.
//!
//! Security contract (build-env-deploy.md §2.5):
//! - `read_secret` compiles only here (feature-gated in `vautr-app-state`).
//! - Revealed secrets are held in `zeroize::Zeroizing` buffers and wiped on
//!   lock via `VautrClient::lock` + `release_secret`.
//! - Local File Transfer hooks consume `vautr-app-state`'s `FileTransferWorker`
//!   (file-storage.md §5.4 desktop auto-sync).

pub mod app;
pub mod state;
pub mod vault_manager_view;

pub use state::VaultManagerState;
pub use vault_manager_view::VaultManagerView;
