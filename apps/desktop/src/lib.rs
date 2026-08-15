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

pub mod api_client;
pub mod app;
pub mod auth_client;
pub mod desktop_view;
pub mod onboarding;
pub mod project_state;
pub mod runtime;
pub mod state;
pub mod theme;
pub mod ui_states;
pub mod updater;

// Deprecated aliases kept for existing imports.
pub use desktop_view::DesktopView;
pub use state::VaultManagerState;
