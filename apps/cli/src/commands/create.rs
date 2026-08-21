//! `vautr-cli create project|secret` — create projects and secrets.

use std::io::Read;

use crate::api::Api;
use crate::cli::CreateArgs;
use crate::config::Config;
use crate::crypto;
use crate::error::{CliError, CliResult};

/// Create a project or secret against the live server.
pub async fn run(cfg: &mut Config, api: &Api, args: CreateArgs) -> CliResult<()> {
    match args {
        CreateArgs::Project {
            name,
            description,
            proj_type,
        } => {
            create_project(
                cfg,
                api,
                &name,
                description.as_deref(),
                proj_type.as_deref(),
            )
            .await
        }
        CreateArgs::Secret {
            key,
            project,
            value,
        } => create_secret(cfg, api, &project, &key, value).await,
    }
}

/// `create project` — POST /projects and store a fresh local project key.
async fn create_project(
    cfg: &mut Config,
    api: &Api,
    name: &str,
    description: Option<&str>,
    proj_type: Option<&str>,
) -> CliResult<()> {
    let token = crate::commands::common::session_token(cfg)?;
    let project = api
        .create_project(token, name, description, proj_type)
        .await?;

    // Generate + persist the project key so the CLI can encrypt/decrypt its
    // secrets (the zero-knowledge server never holds or returns this key).
    if !cfg.project_keys.contains_key(&project.uuid) {
        cfg.project_keys
            .insert(project.uuid.clone(), crypto::generate_project_key_b64());
        cfg.save()?;
    }

    println!("created project {} ({})", project.name, project.uuid);
    Ok(())
}

/// `create secret` — encrypt the value and POST /secrets.
async fn create_secret(
    cfg: &Config,
    api: &Api,
    project_uuid: &str,
    key: &str,
    value: Option<String>,
) -> CliResult<()> {
    let token = crate::commands::common::session_token(cfg)?;
    let key_b64 = cfg
        .project_keys
        .get(project_uuid)
        .cloned()
        .ok_or_else(|| CliError::MissingProjectKey(project_uuid.to_string()))?;

    let plaintext = match value {
        Some(v) => v.into_bytes(),
        None => {
            let mut buf = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buf)
                .map_err(CliError::Io)?;
            buf
        }
    };

    let ciphertext = crypto::encrypt_value(&key_b64, &plaintext)?;
    let secret = api
        .create_secret(token, project_uuid, key, &ciphertext)
        .await?;
    println!("created secret {key} ({})", secret.uuid);
    Ok(())
}
