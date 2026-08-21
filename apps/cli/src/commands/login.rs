//! `vautr-cli login` — OPAQUE password login for a user, storing a session token.
//!
//! The zero-knowledge OPAQUE handshake runs against the live `/auth/login/*`
//! endpoints; on success the CLI persists the session token locally so later
//! commands authenticate without re-entering the password.

use vautr_crypto::opaque;

use crate::api::Api;
use crate::b64;
use crate::config::Config;
use crate::error::{CliError, CliResult};

/// Perform a full OPAQUE login and persist the session.
pub async fn run(cfg: &mut Config, username: Option<String>) -> CliResult<()> {
    let username = match username {
        Some(u) => u,
        None => {
            eprint!("Username: ");
            std::io::Write::flush(&mut std::io::stderr()).map_err(CliError::Io)?;
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .map_err(CliError::Io)?;
            line.trim().to_string()
        }
    };
    if username.is_empty() {
        return Err(CliError::MissingArgument("username"));
    }
    let password = crate::commands::common::prompt_password()?;

    let api = Api::new(&cfg.server_url)?;

    // OPAQUE client login.
    let (lreq, cstate) = opaque::client_login_start(password.as_bytes())
        .map_err(|e| CliError::Crypto(e.to_string()))?;
    let login_start_b64 = b64::encode(&lreq);
    let login_response_b64 = api.login_start(&username, &login_start_b64).await?;
    let login_response = b64::decode(&login_response_b64)?;

    let (finish, _session_key) = opaque::client_login_finish(
        &cstate,
        &login_response,
        password.as_bytes(),
        username.as_bytes(),
    )
    .map_err(|e| CliError::Crypto(e.to_string()))?;

    let session_token = api.login_finish(&username, &b64::encode(&finish)).await?;

    cfg.username = Some(username.clone());
    cfg.session_token = Some(session_token.clone());
    cfg.save()?;

    // The raw token is sensitive; print a truncated hint only.
    println!("logged in as {username}");
    println!(
        "session stored (token {}…)",
        &session_token[..session_token.len().min(12)]
    );
    Ok(())
}
