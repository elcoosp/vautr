//! `vautr-cli list` — list projects and their secrets.

use crate::api::Api;
use crate::config::Config;
use crate::error::CliResult;

/// List projects (and, when `project` is given, its secrets).
pub async fn run(cfg: &Config, api: &Api, project: Option<&str>) -> CliResult<()> {
    let token = crate::commands::common::session_token(cfg)?;
    let projects = api.list_projects(token).await?;

    if projects.is_empty() {
        println!("no projects");
        return Ok(());
    }

    for p in &projects {
        if let Some(su) = project {
            if p.uuid != su {
                continue;
            }
        }
        let proj_type = if p.proj_type == "shared" {
            "shared"
        } else {
            "personal"
        };
        println!("{}  ({proj_type}, {})", p.name, p.uuid);
        if let Some(d) = &p.description {
            if !d.is_empty() {
                println!("    {d}");
            }
        }
        let secrets = api.list_secrets(token, &p.uuid).await?;
        for s in &secrets {
            println!("    {}/{}", p.name, s.key);
        }
    }
    Ok(())
}
