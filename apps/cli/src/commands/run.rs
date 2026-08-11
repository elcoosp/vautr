//! `vautr-cli run -- <cmd>` — inject secrets as env vars and run a command.
//!
//! Resolves every secret the caller can see, decrypts each value, exports it as
//! an environment variable keyed by the secret's key, and spawns the child
//! command. The child inherits the CLI's environment plus the injected secrets,
//! then the CLI exits with the child's exit code.

use std::process::Command;

use crate::api::Api;
use crate::config::Config;
use crate::error::CliResult;

/// Inject secrets into the environment and run `command`.
pub async fn run(cfg: &Config, api: &Api, command: &[String]) -> CliResult<()> {
    let token = crate::commands::common::session_token(cfg)?;
    let projects = api.list_projects(token).await?;

    let mut injected: Vec<(String, String)> = Vec::new();
    for p in &projects {
        let secrets = api.list_secrets(token, &p.uuid).await?;
        for s in &secrets {
            let (_key, ciphertext) = api.reveal_secret(token, &s.uuid).await?;
            let plaintext = crate::commands::common::decrypt_secret(cfg, &p.uuid, &ciphertext)?;
            injected.push((s.key.clone(), plaintext));
        }
    }

    if injected.is_empty() {
        eprintln!("warning: no secrets to inject");
    }

    let mut cmd = Command::new(&command[0]);
    cmd.args(&command[1..]);
    for (k, v) in &injected {
        cmd.env(k, v);
    }

    let status = cmd.status().map_err(crate::error::CliError::Io)?;
    if !status.success() {
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }
    Ok(())
}
