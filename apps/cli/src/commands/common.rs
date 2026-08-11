//! Shared helpers for CLI subcommands: authenticated API access, secret
//! resolution, and password input.

use std::io::{BufRead, Write};
use std::process::exit;

use crate::api::{Api, Project, Secret};
use crate::config::Config;
use crate::error::{CliError, CliResult};

/// Load configuration and build an authenticated [`Api`] client.
///
/// Applies the `--server` override on top of any stored config, then requires a
/// session credential for data-plane operations.
pub fn authed_api(server_override: Option<&str>) -> CliResult<(Config, Api)> {
    let mut cfg = Config::load()?;
    if let Some(s) = server_override {
        cfg.server_url = s.to_string();
    }
    let api = Api::new(&cfg.server_url)?;
    if cfg.credential().is_none() {
        return Err(CliError::NotLoggedIn);
    }
    Ok((cfg, api))
}

/// The active session token, or an error if none is stored.
pub fn session_token(cfg: &Config) -> CliResult<&str> {
    cfg.credential().ok_or(CliError::NotLoggedIn)
}

/// Read a password from `VAUTR_PASSWORD` or, failing that, from stdin.
pub fn read_password() -> CliResult<String> {
    if let Ok(p) = std::env::var("VAUTR_PASSWORD") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).map_err(CliError::Io)?;
    let pw = line.trim_end_matches(['\n', '\r']).to_string();
    if pw.is_empty() {
        return Err(CliError::NoPassword);
    }
    Ok(pw)
}

/// Prompt for a password on stderr (no terminal-echo control; avoids extra deps).
pub fn prompt_password() -> CliResult<String> {
    eprint!("Password: ");
    std::io::stderr().flush().map_err(CliError::Io)?;
    read_password()
}

/// Fetch all projects for the caller.
pub async fn all_projects(cfg: &Config, api: &Api) -> CliResult<Vec<Project>> {
    let token = session_token(cfg)?;
    api.list_projects(token).await
}

/// Find a secret by key, optionally scoped to a project UUID.
///
/// When `project` is `Some`, searches only that project. Otherwise searches
/// every project the caller can see, preferring a unique key match. The `key`
/// may be `name/key` where `name` is a project name.
pub async fn resolve_secret(
    cfg: &Config,
    api: &Api,
    key: &str,
    project: Option<&str>,
) -> CliResult<(Project, Secret)> {
    let token = session_token(cfg)?;
    let projects = all_projects(cfg, api).await?;

    let scope_name: Option<String> = key.split_once('/').map(|(pname, _)| pname.to_string());
    let bare_key: String = key
        .split_once('/')
        .map(|(_, k)| k.to_string())
        .unwrap_or_else(|| key.to_string());

    let candidates: Vec<(&Project, Secret)> = {
        let mut out = Vec::new();
        for p in &projects {
            if let Some(su) = project {
                if p.uuid != su {
                    continue;
                }
            }
            if let Some(sn) = &scope_name {
                if p.name != *sn {
                    continue;
                }
            }
            let secrets = api.list_secrets(token, &p.uuid).await?;
            for s in secrets {
                if s.key == bare_key {
                    out.push((p, s));
                }
            }
        }
        out
    };

    if candidates.is_empty() {
        return Err(CliError::SecretNotFound(key.to_string()));
    }
    if candidates.len() > 1 {
        let names: Vec<String> = candidates.iter().map(|(p, _)| format!("{}/{}", p.name, bare_key)).collect();
        return Err(CliError::Api {
            status: 0,
            message: format!(
                "ambiguous secret '{}' ({}); qualify with --project",
                bare_key,
                names.join(", ")
            ),
        });
    }
    let (p, s) = candidates.into_iter().next().unwrap();
    Ok((p.clone(), s))
}

/// Decrypt a secret's revealed ciphertext with the stored project key.
pub fn decrypt_secret(cfg: &Config, project_uuid: &str, ciphertext_b64: &str) -> CliResult<String> {
    let key_b64 = cfg
        .project_keys
        .get(project_uuid)
        .cloned()
        .ok_or_else(|| CliError::MissingProjectKey(project_uuid.to_string()))?;
    let plaintext = crate::crypto::decrypt_value(&key_b64, ciphertext_b64)?;
    Ok(String::from_utf8_lossy(&plaintext).into_owned())
}

/// Terminate with an error message on stderr.
pub fn bail(err: CliError) -> ! {
    eprintln!("error: {err}");
    exit(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_password_prefers_env() {
        std::env::set_var("VAUTR_PASSWORD", "hunter2");
        assert_eq!(read_password().unwrap(), "hunter2");
        std::env::remove_var("VAUTR_PASSWORD");
    }

    #[test]
    fn split_project_scoped_key() {
        let (scope, bare) = match "proj/DB_PASS".split_once('/') {
            Some((p, k)) => (Some(p.to_string()), k.to_string()),
            None => (None, "DB_PASS".to_string()),
        };
        assert_eq!(scope.as_deref(), Some("proj"));
        assert_eq!(bare, "DB_PASS");
    }
}
