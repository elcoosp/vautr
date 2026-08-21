//! `vautr-cli import` — validate a local `.vautr` backup archive via the
//! server's restore test (decrypt + validate in a scratch DB; never touches
//! the live store).

use std::fs;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::api::Api;
use crate::cli::ImportArgs;
use crate::commands::common::session_token;
use crate::config::Config;
use crate::error::{CliError, CliResult};

/// Read, base64-encode, and submit a local archive for a restore test.
pub async fn run(cfg: &Config, api: &Api, args: ImportArgs) -> CliResult<()> {
    let token = session_token(cfg)?;
    let bytes = fs::read(&args.path).map_err(CliError::Io)?;
    let archive_base64 = B64.encode(&bytes);
    let resp = api.restore_backup(token, &archive_base64).await?;
    println!("status: {}", resp.status);
    println!("test_id: {}", resp.test_id);
    println!("restored_records: {}", resp.restored_records);
    println!("restored_at: {}", resp.restored_at);
    Ok(())
}
