//! `vautr-cli export` — trigger a server-side encrypted vault backup.

use crate::api::Api;
use crate::commands::common::session_token;
use crate::config::Config;
use crate::error::CliResult;

/// Trigger a backup export and print the resulting archive metadata.
pub async fn run(cfg: &Config, api: &Api) -> CliResult<()> {
    let token = session_token(cfg)?;
    let resp = api.export_backup(token, true).await?;
    println!("backup_id: {}", resp.backup_id);
    if let Some(url) = resp.download_url {
        println!("download_url: {url}");
    }
    println!("size_bytes: {}", resp.size_bytes);
    println!("checksum: {}", resp.checksum);
    println!("created_at: {}", resp.created_at);
    Ok(())
}
