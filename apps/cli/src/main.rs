//! Binary entry point for the Vautr CLI.
//!
//! Thin wrapper: collects args and delegates to [`vautr_cli::run`], which owns all
//! parsing and dispatch so the logic stays unit-testable in the library crate.

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let code = vautr_cli::run(&raw);
    std::process::exit(code);
}
