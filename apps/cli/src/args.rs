//! Command-line argument parsing for the Vautr CLI.
//!
//! Wave 0.3 wires `--version` / `--help` for real and recognizes the planned
//! subcommands (`get`, `list`, `run`, `login`) so their safe stubs can dispatch.
//! A richer parser (flags, IDs, `--` env-injection args) arrives in Wave B4.

use crate::error::{CliError, CliResult};

/// A subcommand recognized by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `vautr-cli get <id>` — reveal one secret.
    Get,
    /// `vautr-cli list` — list secrets/projects.
    List,
    /// `vautr-cli run -- <cmd>` — inject env and run a child process.
    Run,
    /// `vautr-cli login` — machine-account login.
    Login,
}

/// The outcome of parsing the raw argument vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parsed {
    /// `--version` / `-V`.
    Version,
    /// `--help` / `-h`, or no arguments at all.
    Help,
    /// A concrete subcommand to dispatch.
    Command(Command),
}

/// Parse the raw argument list (excluding the program name).
pub fn parse(raw: &[String]) -> CliResult<Parsed> {
    match raw.first().map(String::as_str) {
        None => Ok(Parsed::Help),
        Some("-V" | "--version") => Ok(Parsed::Version),
        Some("-h" | "--help") => Ok(Parsed::Help),
        Some("get") => Ok(Parsed::Command(Command::Get)),
        Some("list") => Ok(Parsed::Command(Command::List)),
        Some("run") => Ok(Parsed::Command(Command::Run)),
        Some("login") => Ok(Parsed::Command(Command::Login)),
        Some(other) => Err(CliError::UnknownCommand(other.to_string())),
    }
}

/// Print the top-level help text to stdout.
pub fn print_help() {
    println!(
        "vautr-cli {} - Vautr command-line client\n\
         \n\
         USAGE:\n\
         \x20   vautr-cli <COMMAND>\n\
         \n\
         COMMANDS:\n\
         \x20   get     Reveal a secret\n\
         \x20   list    List secrets and projects\n\
         \x20   run     Run a command with secrets injected into the environment\n\
         \x20   login   Log in a machine account\n\
         \n\
         OPTIONS:\n\
         \x20   -h, --help     Print help\n\
         \x20   -V, --version  Print version",
        crate::VERSION
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn no_args_yields_help() {
        assert_eq!(parse(&s(&[])), Ok(Parsed::Help));
    }

    #[test]
    fn version_flags_are_recognized() {
        assert_eq!(parse(&s(&["--version"])), Ok(Parsed::Version));
        assert_eq!(parse(&s(&["-V"])), Ok(Parsed::Version));
    }

    #[test]
    fn help_flags_are_recognized() {
        assert_eq!(parse(&s(&["--help"])), Ok(Parsed::Help));
        assert_eq!(parse(&s(&["-h"])), Ok(Parsed::Help));
    }

    #[test]
    fn subcommands_are_recognized() {
        assert_eq!(parse(&s(&["get"])), Ok(Parsed::Command(Command::Get)));
        assert_eq!(parse(&s(&["list"])), Ok(Parsed::Command(Command::List)));
        assert_eq!(parse(&s(&["run"])), Ok(Parsed::Command(Command::Run)));
        assert_eq!(parse(&s(&["login"])), Ok(Parsed::Command(Command::Login)));
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert_eq!(
            parse(&s(&["frobnicate"])),
            Err(CliError::UnknownCommand("frobnicate".to_string()))
        );
    }
}
