//! `vautr-cli edit project|secret` — edit projects and secrets.

use crate::api::Api;
use crate::cli::EditArgs;
use crate::config::Config;
use crate::crypto;
use crate::error::{CliError, CliResult};

/// Edit a project or secret against the live server.
pub async fn run(cfg: &Config, api: &Api, args: EditArgs) -> CliResult<()> {
    match args {
        EditArgs::Project {
            uuid,
            name,
            description,
        } => edit_project(api, cfg, &uuid, name.as_deref(), description.as_deref()).await,
        EditArgs::Secret { target, key, value } => {
            edit_secret(cfg, api, &target, key.as_deref(), value).await
        }
    }
}

/// `edit project` — PATCH /projects/{uuid}.
async fn edit_project(
    api: &Api,
    cfg: &Config,
    uuid: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> CliResult<()> {
    let token = crate::commands::common::session_token(cfg)?;
    let project = api.update_project(token, uuid, name, description).await?;
    println!("updated project {} ({})", project.name, project.uuid);
    Ok(())
}

/// `edit secret` — resolve the target secret, encrypt a new value if given, and
/// PATCH /secrets/{uuid}.
async fn edit_secret(
    cfg: &Config,
    api: &Api,
    target: &str,
    new_key: Option<&str>,
    value: Option<String>,
) -> CliResult<()> {
    let token = crate::commands::common::session_token(cfg)?;

    // Resolve the target secret (by key, or by UUID if it looks like one).
    let secret = if is_uuid(target) {
        find_secret_by_uuid(api, token, target).await?
    } else {
        let (_, s) = crate::commands::common::resolve_secret(cfg, api, target, None).await?;
        s
    };

    let key_b64 = cfg
        .project_keys
        .get(&secret.project_uuid)
        .cloned()
        .ok_or_else(|| CliError::MissingProjectKey(secret.project_uuid.clone()))?;

    let value_ct = match value {
        Some(v) => Some(crypto::encrypt_value(&key_b64, v.as_bytes())?),
        None => None,
    };

    let updated = api
        .update_secret(token, &secret.uuid, new_key, value_ct.as_deref())
        .await?;
    println!("updated secret {} ({})", updated.key, updated.uuid);
    Ok(())
}

fn is_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

async fn find_secret_by_uuid(api: &Api, token: &str, uuid: &str) -> CliResult<crate::api::Secret> {
    let projects = api.list_projects(token).await?;
    for p in &projects {
        let secrets = api.list_secrets(token, &p.uuid).await?;
        if let Some(s) = secrets.into_iter().find(|s| s.uuid == uuid) {
            return Ok(s);
        }
    }
    Err(CliError::SecretNotFound(uuid.to_string()))
}
