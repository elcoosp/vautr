//! Subcommand dispatch for the Vautr CLI.

pub mod common;
pub mod create;
pub mod edit;
pub mod get;
pub mod list;
pub mod login;
pub mod machine_account;
pub mod register;
pub mod run;

use crate::api::Api;
use crate::cli::Command;
use crate::error::CliResult;

/// Run the parsed subcommand against a freshly loaded config + API client.
pub async fn dispatch(command: Command, server_override: Option<&str>) -> CliResult<()> {
    let mut cfg = crate::config::Config::load()?;
    if let Some(s) = server_override {
        cfg.server_url = s.to_string();
    }
    let api = Api::new(&cfg.server_url)?;

    match command {
        Command::Login(args) => login::run(&mut cfg, args.username).await,
        Command::Register(args) => register::run(&mut cfg, &args.username).await,
        Command::Logout => logout(),
        Command::MachineAccount(args) => {
            machine_account::run(&mut cfg, &api, &args.name, args.scopes).await
        }
        Command::List(args) => list::run(&cfg, &api, args.project.as_deref()).await,
        Command::Get(args) => get::run(&cfg, &api, &args.key, args.project.as_deref()).await,
        Command::Run(args) => run::run(&cfg, &api, &args.command).await,
        Command::Create { command } => create::run(&mut cfg, &api, command).await,
        Command::Edit { command } => edit::run(&cfg, &api, command).await,
    }
}

/// `logout` — clear the stored session (and machine-account credential).
fn logout() -> CliResult<()> {
    let mut cfg = crate::config::Config::load()?;
    cfg.session_token = None;
    cfg.username = None;
    cfg.machine_account = None;
    cfg.save()?;
    println!("logged out");
    Ok(())
}
