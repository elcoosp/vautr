//! Live-server end-to-end test for Machine Accounts + Access Tokens (Wave A2).
//!
//! Boots the real `vautr-server` router on a loopback TCP socket and drives the
//! full flow over actual HTTP/1.1:
//!
//!   1. Register + log in an admin user (OPAQUE).
//!   2. Create a machine account (scopes: `secrets:read`).
//!   3. Issue a scoped access token bound to it (returns the full secret once).
//!   4. Verify the token authenticates with the right scopes.
//!   5. Revoke the token -> verify now denies.
//!   6. Issue another token, force its expiry in the past -> verify denies.
//!   7. Scope over-request beyond the machine account is rejected (403).
//!
//! Uses UUID-suffixed names/emails so parallel test runs never collide.
//! A minimal HTTP/1.1 client over `tokio::net::TcpStream` is used (no extra deps).

use std::net::SocketAddr;
use std::sync::Arc;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use vautr_crypto::opaque;
use vautr_server::db;
use vautr_server::handlers::{AppState, build_router};
use vautr_server::repository::Repository;

/// Workaround mirroring `projects_e2e.rs`: the committed `server_config` schema
/// (0001_init.sql) differs from the key/value shape `repository/config.rs` reads.
/// Reconciled on the throwaway test DB only; no project files touched.
async fn reconcile_server_config(pool: &sqlx::SqlitePool) {
    let has_value: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('server_config') WHERE name = 'value'",
    )
    .fetch_one(pool)
    .await
    .expect("probe server_config schema");
    if has_value > 0 {
        return;
    }
    sqlx::query(
        "DROP TABLE server_config;
         CREATE TABLE server_config (
             key         TEXT PRIMARY KEY,
             value       BLOB,
             created_at  INTEGER NOT NULL DEFAULT (unixepoch())
         ) STRICT;",
    )
    .execute(pool)
    .await
    .expect("reconcile server_config to key/value shape");
}

fn b64(b: &[u8]) -> String {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    B64.encode(b)
}

fn decode_b64(s: &str) -> Vec<u8> {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    B64.decode(s).unwrap()
}

/// Minimal HTTP/1.1 request over a fresh TCP connection (`Connection: close`).
async fn http(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (u16, Value) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let body_bytes = body
        .map(|b| serde_json::to_vec(&b).unwrap())
        .unwrap_or_default();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
    if let Some(t) = token {
        req.push_str(&format!("Authorization: Bearer {t}\r\n"));
    }
    if !body_bytes.is_empty() {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body_bytes.len()
        ));
    }
    req.push_str("Connection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await.expect("write headers");
    if !body_bytes.is_empty() {
        stream.write_all(&body_bytes).await.expect("write body");
    }
    stream.flush().await.ok();

    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        let n = stream.read(&mut tmp).await.expect("read");
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let text = String::from_utf8_lossy(&buf);
    let status_line = text.split("\r\n").next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body_text = match text.split_once("\r\n\r\n") {
        Some((_, b)) => b.trim(),
        None => "",
    };
    let value = serde_json::from_str(body_text).unwrap_or(Value::Null);
    (status, value)
}

async fn register(addr: SocketAddr, username: &str, password: &[u8]) {
    let (start, cstate) = opaque::client_register_start(password).expect("reg start");
    let (status, resp) = http(
        addr,
        "POST",
        "/auth/register/start",
        None,
        Some(json!({ "username": username, "registration_start": b64(&start) })),
    )
    .await;
    assert_eq!(status, 200, "register/start: {resp}");
    let sresp = decode_b64(resp["registration_response"].as_str().unwrap());

    let (upload, _export) =
        opaque::client_register_finish(&cstate, &sresp, password, username.as_bytes())
            .expect("reg finish");
    let server_pk = opaque::server_setup_public_key().expect("setup pk");
    let (status, resp) = http(
        addr,
        "POST",
        "/auth/register/finish",
        None,
        Some(json!({
            "username": username,
            "registration_finish": b64(&upload),
            "server_public_key": b64(&server_pk),
            "kdf_salt": b64(&[7u8; 16]),
            "svk_ciphertext_blob": b64(&[8u8; 48]),
            "svk_ciphertext_blob_rk": b64(&[9u8; 48]),
        })),
    )
    .await;
    assert_eq!(status, 200, "register/finish: {resp}");
}

async fn login(addr: SocketAddr, username: &str, password: &[u8]) -> String {
    let (start, cstate) = opaque::client_login_start(password).expect("login start");
    let (status, resp) = http(
        addr,
        "POST",
        "/auth/login/start",
        None,
        Some(json!({ "username": username, "login_start": b64(&start) })),
    )
    .await;
    assert_eq!(status, 200, "login/start: {resp}");
    let sresp = decode_b64(resp["login_response"].as_str().unwrap());
    let (finish, _key) =
        opaque::client_login_finish(&cstate, &sresp, password, username.as_bytes())
            .expect("login finish");
    let (status, resp) = http(
        addr,
        "POST",
        "/auth/login/finish",
        None,
        Some(json!({ "username": username, "login_finish": b64(&finish) })),
    )
    .await;
    assert_eq!(status, 200, "login/finish: {resp}");
    resp["session_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn machine_accounts_live_server_e2e() {
    // Unique temp DB + fresh loopback listener.
    let db_path = std::env::temp_dir().join(format!("vautr_ma_e2e_{}.db", uuid::Uuid::new_v4()));
    let url = format!("sqlite://{}", db_path.display());
    let pool = db::connect(&url).await.expect("connect + migrate");
    reconcile_server_config(&pool).await;
    let repo = Arc::new(Repository::new(pool));
    let state = AppState::new(repo.clone());

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let suffix = uuid::Uuid::new_v4().to_string();
    let email = format!("admin-{suffix}@e2e.test");
    let password = b"correct horse battery staple";
    let future = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64 + 3600_000)
        .unwrap();

    register(addr, &email, password).await;
    let tok = login(addr, &email, password).await;

    // 2. Create a machine account (UUID-suffixed name).
    let (status, resp) = http(
        addr,
        "POST",
        "/machine-accounts",
        Some(&tok),
        Some(json!({
            "name": format!("ci-runner-{suffix}"),
            "description": "E2E CI runner",
            "scopes": ["secrets:read"],
            "expires_at": future,
        })),
    )
    .await;
    assert_eq!(status, 201, "create machine account: {resp}");
    let ma_uuid = resp["uuid"].as_str().unwrap().to_string();
    assert_eq!(resp["status"], "active");
    assert_eq!(resp["scopes"][0], "secrets:read");

    // Wave C audit seam: machine-account creation is recorded as an org event.
    let audit = repo
        .query_audit_events(
            &vautr_server::repository::audit::AuditFilter {
                resource_type: Some("machine_account"),
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .expect("query audit events");
    assert!(
        audit.iter().any(|r| r.action == "machine_account_create"),
        "expected machine_account_create audit event"
    );

    // List shows it.
    let (status, resp) = http(addr, "GET", "/machine-accounts", Some(&tok), None).await;
    assert_eq!(status, 200);
    assert_eq!(resp["machine_accounts"].as_array().unwrap().len(), 1);

    // 3. Issue a scoped token bound to the machine account.
    let (status, resp) = http(
        addr,
        "POST",
        "/tokens",
        Some(&tok),
        Some(json!({
            "name": "deploy-token",
            "machine_account_uuid": ma_uuid,
            "scopes": ["secrets:read"],
            "expires_at": future,
        })),
    )
    .await;
    assert_eq!(status, 201, "issue token: {resp}");
    let secret = resp["token"].as_str().unwrap().to_string();
    let token_id = resp["token_id"].as_str().unwrap().to_string();
    assert!(!secret.is_empty());
    assert_eq!(resp["expires_at"], json!(future));

    // 4. Verify the token authenticates with the right scopes (the "scoped call").
    let vt = vautr_server::handlers::tokens::verify_access_token(&repo, &secret)
        .await
        .expect("token verifies");
    assert_eq!(vt.machine_account_uuid.as_deref(), Some(ma_uuid.as_str()));
    assert!(vt.scopes.contains(&"secrets:read".to_string()));
    assert!(!vt.scopes.contains(&"secrets:write".to_string()));
    // A wrong secret is denied.
    assert!(vautr_server::handlers::tokens::verify_access_token(&repo, "bogus-secret")
        .await
        .is_err());

    // List token metadata: the secret must never appear.
    let (status, resp) = http(addr, "GET", "/tokens", Some(&tok), None).await;
    assert_eq!(status, 200);
    let arr = resp["tokens"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert!(arr[0].get("token").is_none(), "secret never returned in list");
    assert_eq!(arr[0]["uuid"], token_id);

    // 5. Revoke via DELETE, then the token is denied.
    let (status, resp) = http(
        addr,
        "DELETE",
        &format!("/tokens/{token_id}"),
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(status, 200, "revoke: {resp}");
    assert_eq!(resp["status"], "revoked");
    assert!(vautr_server::handlers::tokens::verify_access_token(&repo, &secret)
        .await
        .is_err());

    // Wave C audit seam: token create + revoke are recorded as org events.
    let token_audit = repo
        .query_audit_events(
            &vautr_server::repository::audit::AuditFilter {
                resource_type: Some("access_token"),
                ..Default::default()
            },
            100,
            0,
        )
        .await
        .expect("query token audit");
    assert!(
        token_audit.iter().any(|r| r.action == "token_create"),
        "expected token_create audit event"
    );
    assert!(
        token_audit.iter().any(|r| r.action == "token_revoke"),
        "expected token_revoke audit event"
    );

    // 6. Expiry: issue a token, force its expiry into the past, then it is denied.
    let (status, resp) = http(
        addr,
        "POST",
        "/tokens",
        Some(&tok),
        Some(json!({
            "name": "short-lived",
            "machine_account_uuid": ma_uuid,
            "scopes": ["secrets:read"],
            "expires_at": future,
        })),
    )
    .await;
    assert_eq!(status, 201, "issue second token: {resp}");
    let secret2 = resp["token"].as_str().unwrap().to_string();
    let past = 1_000_000_000_000i64;
    sqlx::query("UPDATE access_tokens SET expires_at = ?")
        .bind(past)
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(
        vautr_server::handlers::tokens::verify_access_token(&repo, &secret2)
            .await
            .is_err(),
        "expired token denied"
    );

    // 7. Scope over-request beyond the machine account's scopes is rejected (403).
    let (status, resp) = http(
        addr,
        "POST",
        "/tokens",
        Some(&tok),
        Some(json!({
            "name": "over-scoped",
            "machine_account_uuid": ma_uuid,
            "scopes": ["secrets:read", "projects:write"],
        })),
    )
    .await;
    assert_eq!(status, 403, "scope over-request denied: {resp}");
    assert_eq!(resp["error"], "scope_not_allowed");

    let _ = std::fs::remove_file(&db_path);
}
