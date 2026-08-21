//! # vautr-cli
//!
//! The Vautr `bws`-style command-line client (docs/architecture/mlp-scope.md §4).
//!
//! Surface:
//! - `login [USERNAME]` — OPAQUE password login, stores a session token.
//! - `register USERNAME` — OPAQUE registration.
//! - `machine-account [--name N]` — provision a machine account + access token.
//! - `list [--project UUID]` — list projects and secrets.
//! - `get <key> [--project UUID]` — reveal a secret's plaintext value.
//! - `run -- <cmd>…` — inject resolved secrets as env vars and run a command.
//! - `create project|secret …` — create projects and secrets.
//! - `edit project|secret …` — edit projects and secrets.
//! - `logout` — clear the stored session.
//!
//! Auth model: the zero-knowledge server authenticates data-plane endpoints
//! with **session** tokens (minted by the OPAQUE `/auth/login/*` flow), so the
//! CLI stores that session and uses it for project/secret operations.
//! `machine-account` provisions a Wave-A2 non-human identity and issues a
//! one-time access token (the machine credential for CI/agents).
//!
//! No `todo!()` / `panic!()` in compiled code.

pub mod api;
pub mod b64;
pub mod cli;
pub mod commands;
pub mod config;
pub mod crypto;
pub mod error;

pub use error::{CliError, CliResult};

/// The crate's compiled version, sourced from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Process raw CLI arguments and run the requested command.
///
/// Returns the process exit code: `0` on success, `1` on error.
pub fn run(raw: &[String]) -> i32 {
    use clap::Parser;

    let cli = match cli::Cli::try_parse_from(
        std::iter::once("vautr-cli".to_string()).chain(raw.iter().cloned()),
    ) {
        Ok(c) => c,
        Err(e) => {
            // clap prints help/usage/errors to the appropriate stream.
            let _ = e.print();
            return if e.use_stderr() { 1 } else { 0 };
        }
    };

    let server_override = cli.server.clone().or_else(|| {
        config::Config::load()
            .ok()
            .map(|c| c.server_url)
            .filter(|s| !s.is_empty())
    });

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    let result = rt.block_on(commands::dispatch(cli.command, server_override.as_deref()));

    match result {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}
