//! `vautr-cli register` — OPAQUE registration for a new user.
//!
//! Mirrors `apps/desktop/src/auth_client.rs` exactly: derives the real
//! KDF salt + master key, generates and wraps the SVK (MP-wrapped and
//! Recovery-Key-wrapped), completes the OPAQUE handshake, and persists the
//! wrapped blobs. The earlier hardcoded dummies (`[7u8;16]` salt, `[8u8;48]`
//! wrapped-SVK) produced permanently-broken accounts — see audit.

use uuid::Uuid;

use vautr_auth::state;
use vautr_crypto::{kdf, key_tree, recovery};
use vautr_keyring::wrap;

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

    // ── Local key material (crypto.md §2) ──────────────────────────
    let kdf_salt = kdf::generate_kdf_salt();
    let mk = kdf::derive_master_key(&password, &kdf_salt)
        .map_err(|e| CliError::Crypto(e.to_string()))?;
    let kek = key_tree::derive_kek(&mk).map_err(|e| CliError::Crypto(e.to_string()))?;
    let svk = key_tree::generate_svk();
    let svk_wrapped = wrap::wrap_svk(&kek, &svk);

    // Recovery Key (REQ-RECOVERY-02)
    let mnemonic =
        recovery::generate_recovery_mnemonic().map_err(|e| CliError::Crypto(e.to_string()))?;
    let mnemonic_bytes = recovery::decode_recovery_mnemonic(&mnemonic)
        .map_err(|e| CliError::Crypto(e.to_string()))?;
    let kek_rk =
        recovery::derive_kek_rk(&mnemonic_bytes).map_err(|e| CliError::Crypto(e.to_string()))?;
    let svk_rk_wrapped = recovery::wrap_svk_with_rk(&svk, &kek_rk, &Uuid::nil())
        .map_err(|e| CliError::Crypto(e.to_string()))?;

    // ── OPAQUE registration (api.md §3.1) ──────────────────────────
    let (cstate, creq) = state::registration_start(&password);
    let reg_start_b64 = b64::encode(&creq);
    let reg_response_b64 = api.register_start(username, &reg_start_b64).await?;
    let reg_response = b64::decode(&reg_response_b64)?;

    let (upload, _export, _state) =
        state::registration_finish(&cstate, &reg_response, &password, username.as_bytes());

    // The server ignores the exact server public key on register/finish, so we
    // pass a freshly generated one (matches the wire contract).
    let server_pk = vautr_crypto::opaque::server_setup_public_key()
        .map_err(|e| CliError::Crypto(e.to_string()))?;

    api.register_finish(
        username,
        &b64::encode(&upload),
        &b64::encode(&server_pk),
        &b64::encode(&kdf_salt),
        &b64::encode(&svk_wrapped),
        &b64::encode(&svk_rk_wrapped),
    )
    .await?;

    println!("registered {username}; recovery phrase: {mnemonic}");
    println!("store this recovery phrase safely — it is required to recover your vault");
    Ok(())
}
