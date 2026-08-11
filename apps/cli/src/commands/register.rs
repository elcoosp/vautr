//! `vautr-cli register` — OPAQUE registration for a new user.

use vautr_crypto::opaque;

use crate::api::Api;
use crate::b64;
use crate::config::Config;
use crate::error::{CliError, CliResult};

/// Register a new user via the OPAQUE `/auth/register/*` flow.
pub async fn run(cfg: &mut Config, username: &str) -> CliResult<()> {
    if username.is_empty() {
        return Err(CliError::MissingArgument("username"));
    }
    let password = crate::commands::common::prompt_password()?;

    let api = Api::new(&cfg.server_url)?;

    // OPAQUE client registration.
    let (creq, cstate) =
        opaque::client_register_start(password.as_bytes()).map_err(|e| CliError::Crypto(e.to_string()))?;
    let reg_start_b64 = b64::encode(&creq);
    let reg_response_b64 = api.register_start(username, &reg_start_b64).await?;
    let reg_response = b64::decode(&reg_response_b64)?;

    let (upload, _export) = opaque::client_register_finish(
        &cstate,
        &reg_response,
        password.as_bytes(),
        username.as_bytes(),
    )
    .map_err(|e| CliError::Crypto(e.to_string()))?;

    // The server ignores the exact server public key on register/finish, so we
    // pass a freshly generated one (matches the wire contract).
    let server_pk = opaque::server_setup_public_key()
        .map_err(|e| CliError::Crypto(e.to_string()))?;

    api.register_finish(
        username,
        &b64::encode(&upload),
        &b64::encode(&server_pk),
        &b64::encode(&[7u8; 16]),   // kdf_salt (stored, unused server-side)
        &b64::encode(&[8u8; 48]),   // svk_ciphertext_blob (opaque)
        &b64::encode(&[9u8; 48]),   // svk_ciphertext_blob_rk (opaque)
    )
    .await?;

    println!("registered {username}; run `vautr-cli login {username}` to sign in");
    Ok(())
}
