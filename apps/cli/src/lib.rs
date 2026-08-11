//! # vautr-cli
//!
//! The Vautr `bws`-style command-line client (docs/architecture/mlp-scope.md §4).
//!
//! Planned surface (Wave B4, `docs/architecture/mlp-wave-plan.md` §B4):
//! - `vautr-cli get <id>` — reveal a single secret to stdout.
//! - `vautr-cli list` — list secrets/projects for the authenticated machine account.
//! - `vautr-cli run -- <command>…` — inject resolved secrets into a child process
//!   environment.
//! - `vautr-cli login` — machine-account login (access-token based).
//!
//! **Scaffold note (Wave 0.3):** only `--version` / `--help` are wired for real.
//! The subcommand handlers are safe, non-panicking stubs that resolve to
//! [`CliError::NotYetImplemented`]; the real logic lands in Wave B4.
//!
//! No `todo!()` / `panic!` in compiled code.

pub mod args;
pub mod commands;
pub mod config;
pub mod error;

pub use error::{CliError, CliResult};

/// The crate's compiled version, sourced from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Process raw CLI arguments and run the requested command.
///
/// Returns the process exit code: `0` on success, `1` on error.
pub fn run(raw: &[String]) -> i32 {
    match args::parse(raw) {
        Ok(args::Parsed::Version) => {
            println!("vautr-cli {VERSION}");
            0
        }
        Ok(args::Parsed::Help) => {
            args::print_help();
            0
        }
        Ok(args::Parsed::Command(command)) => match commands::dispatch(command) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("error: {err}");
                1
            }
        },
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}
