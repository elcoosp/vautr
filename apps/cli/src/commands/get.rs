//! `vautr-cli get <key>` — reveal a secret's plaintext value.
//!
//! Calls the live `/secrets/{uuid}/value` reveal endpoint (gated by
//! `secrets:reveal` on the server) and decrypts the returned AEAD ciphertext
//! with the project key the CLI holds locally.

use crate::api::Api;
use crate::config::Config;
use crate::error::CliResult;

/// Reveal and print a secret's plaintext value.
pub async fn run(cfg: &Config, api: &Api, key: &str, project: Option<&str>) -> CliResult<()> {
    let (proj, secret) = crate::commands::common::resolve_secret(cfg, api, key, project).await?;
    let token = crate::commands::common::session_token(cfg)?;
    let (_revealed_key, ciphertext) = api.reveal_secret(token, &secret.uuid).await?;
    let plaintext = crate::commands::common::decrypt_secret(cfg, &proj.uuid, &ciphertext)?;
    print!("{plaintext}");
    Ok(())
}
