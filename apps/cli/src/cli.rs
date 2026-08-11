//! Clap command-line interface definition for `vautr-cli`.
//!
//! The `bws`-style surface (docs/architecture/mlp-scope.md §4): `login`,
//! `register`, `list`, `get`, `run`, `create`, `edit`, `logout`.

use clap::{Args, Parser, Subcommand};

/// The Vautr command-line client.
#[derive(Debug, Parser)]
#[command(
    name = "vautr-cli",
    version,
    about = "Vautr bws-style command-line client (docs/architecture/mlp-scope.md §4)",
    subcommand_required = true,
    arg_required_else_help = true
)]
pub struct Cli {
    /// Vautr server base URL (defaults to the stored config or localhost).
    #[arg(long, global = true)]
    pub server: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

/// Subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Log in a user (OPAQUE password) and store a session.
    Login(LoginArgs),
    /// Register a new user account (OPAQUE).
    Register(RegisterArgs),
    /// Provision a machine account + issue an access token (needs a session).
    MachineAccount(MachineAccountArgs),
    /// Clear the stored session.
    Logout,
    /// List projects and their secrets.
    List(ListArgs),
    /// Reveal a secret's plaintext value by key.
    Get(GetArgs),
    /// Run a command with secrets injected into the environment.
    Run(RunArgs),
    /// Create a project or secret.
    Create {
        #[command(subcommand)]
        command: CreateArgs,
    },
    /// Edit a project or secret.
    Edit {
        #[command(subcommand)]
        command: EditArgs,
    },
}

/// `login` arguments.
#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Username (email). Prompted if omitted.
    pub username: Option<String>,
}

/// `register` arguments.
#[derive(Debug, Args)]
pub struct RegisterArgs {
    /// Username (email) to register.
    pub username: String,
}

/// `machine-account` arguments (provision + issue token).
#[derive(Debug, Args)]
pub struct MachineAccountArgs {
    /// Machine-account display name.
    #[arg(long, default_value = "cli")]
    pub name: String,
    /// Comma-separated scopes. Defaults to the CLI's standard secret/project scopes.
    #[arg(long, value_delimiter = ',')]
    pub scopes: Option<Vec<String>>,
}

/// `list` arguments.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Only list secrets within this project UUID.
    #[arg(long)]
    pub project: Option<String>,
}

/// `get` arguments.
#[derive(Debug, Args)]
pub struct GetArgs {
    /// Secret key (optionally `project-name/key`).
    pub key: String,
    /// Restrict lookup to a project UUID.
    #[arg(long)]
    pub project: Option<String>,
}

/// `run` arguments.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Command to execute with resolved secrets in its environment.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}

/// `create` arguments (nested subcommands).
#[derive(Debug, Subcommand)]
pub enum CreateArgs {
    /// Create a project.
    Project {
        /// Project name.
        name: String,
        /// Optional description.
        #[arg(long)]
        description: Option<String>,
        /// Project type: `personal` (default) or `shared`.
        #[arg(long, value_parser = ["personal", "shared"])]
        proj_type: Option<String>,
    },
    /// Create a secret in a project.
    Secret {
        /// Secret key (the env var name).
        key: String,
        /// Project UUID to place the secret in.
        #[arg(long, required = true)]
        project: String,
        /// Plaintext value. Reads from stdin if omitted.
        #[arg(long)]
        value: Option<String>,
    },
}

/// `edit` arguments (nested subcommands).
#[derive(Debug, Subcommand)]
pub enum EditArgs {
    /// Edit a project's name/description.
    Project {
        /// Project UUID.
        uuid: String,
        /// New name.
        #[arg(long)]
        name: Option<String>,
        /// New description.
        #[arg(long)]
        description: Option<String>,
    },
    /// Edit a secret's key and/or value.
    Secret {
        /// Secret UUID or key (resolved if ambiguous).
        target: String,
        /// New key (renames the secret).
        #[arg(long)]
        key: Option<String>,
        /// New plaintext value. Reads from stdin if the flag is set.
        #[arg(long)]
        value: Option<String>,
    },
}
