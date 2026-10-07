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

    // VTRFIX-SEC-H18: reject secret names that would enable arbitrary code
    // execution in the child process (LD_PRELOAD, NODE_OPTIONS, PATH, ...).
    for (k, _) in &injected {
        validate_env_key(k)?;
    }

    // VTRFIX-BUG-M11: guard the empty command list to avoid a panic.
    let Some(prog) = command.first() else {
        return Err(crate::error::CliError::MissingArgument("command"));
    };
    let mut cmd = Command::new(prog);
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

/// VTRFIX-SEC-H18: a secret key must be a valid POSIX env name and must not
/// be on the well-known "loads code on process start" denylist.
fn validate_env_key(k: &str) -> Result<(), crate::error::CliError> {
    const FORBIDDEN_PREFIXES: [&str; 3] = ["LD_", "DYLD_", "SHELL"];
    const FORBIDDEN_NAMES: [&str; 12] = [
        "PATH",
        "NODE_OPTIONS",
        "BASH_ENV",
        "ENV",
        "IFS",
        "PYTHONSTARTUP",
        "PYTHONPATH",
        "PERL5OPT",
        "RUBYOPT",
        "GEM_HOME",
        "GEM_PATH",
        "JAVA_TOOL_OPTIONS",
    ];
    let valid = !k.is_empty()
        && k.chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !FORBIDDEN_NAMES.contains(&k)
        && !FORBIDDEN_PREFIXES.iter().any(|p| k.starts_with(p));
    if !valid {
        return Err(crate::error::CliError::InvalidSecretName(k.to_string()));
    }
    Ok(())
}
