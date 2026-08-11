//! `vautr-cli machine-account` — provision a machine account and issue a
//! scoped access token (Wave A2), using the active user session.
//!
//! This is the "machine-account login (issue/use an access token)" flow: the
//! CLI creates a non-human identity and returns the one-time access-token
//! secret that CI/CD or agents present as a Bearer credential.

use crate::api::{Api, MACHINE_SCOPES};
use crate::config::{Config, MachineAccountInfo};
use crate::error::CliResult;

/// Provision a machine account + access token and print the token once.
pub async fn run(
    cfg: &mut Config,
    api: &Api,
    name: &str,
    scopes: Option<Vec<String>>,
) -> CliResult<()> {
    let token = crate::commands::common::session_token(cfg)?;
    let scopes: Vec<&str> = match &scopes {
        Some(s) if !s.is_empty() => s.iter().map(String::as_str).collect(),
        _ => MACHINE_SCOPES.to_vec(),
    };

    let ma = api.create_machine_account(token, name, &scopes).await?;
    let issued = api.issue_token(token, name, &ma.uuid, &scopes).await?;

    cfg.machine_account = Some(MachineAccountInfo {
        uuid: ma.uuid.clone(),
        name: ma.name.clone(),
        token_prefix: issued.token.chars().take(8).collect(),
    });
    cfg.save()?;

    println!("machine account: {} ({})", ma.name, ma.uuid);
    println!("access token (shown once): {}", issued.token);
    println!("scopes: {}", scopes.join(", "));
    Ok(())
}
