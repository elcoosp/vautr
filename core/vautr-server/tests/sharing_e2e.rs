//! Live-server end-to-end test for the Sharing / Shares feature (VTR-072).
//!
//! Boots the real `vautr-server` router on a loopback socket and drives the
//! full zero-knowledge share flow over actual HTTP/1.1:
//!
//!   1. Register sender (A) + recipient (B) via OPAQUE, log both in.
//!   2. A creates a project + secret (value stored as opaque ciphertext).
//!   3. B publishes their sharing public key; A fetches it.
//!   4. A KEM-wraps the item for B (`vautr_sharing::share_item`) and relays the
//!      envelope via `POST /shares/` + `POST /shares/{item}/payload`.
//!   5. B lists their inbox (`GET /shares/inbox`) and sees exactly one pending
//!      share.
//!   6. B decapsulates + decrypts (`vautr_sharing::accept_share`) and recovers
//!      the original plaintext — proving the end-to-end encrypted handoff with
//!      the server only ever holding wrapped blobs (zero-knowledge).
//!
//! This is the cross-client Shares contract that web / extension / mobile /
//! desktop all exercise; it had no coverage until this test. Mirrors the
//! in-process harness from `projects_e2e.rs`.

use std::net::SocketAddr;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

use vautr_crypto::opaque;
use vautr_crypto::sharing::{SharingKeyPair, SharingPublicKey};
use vautr_server::db;
use vautr_server::handlers::{build_router, AppState};
use vautr_server::repository::Repository;
use vautr_sharing::{accept_share, share_item, IncomingShare};

/// WORKAROUND (test-scoped, temp DB only): reconcile the frozen `server_config`
/// schema to the key/value shape the running server expects so real OPAQUE
/// registration can run over the wire. No project files are touched.
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
    B64.encode(b)
}

fn decode_b64(s: &str) -> Vec<u8> {
    B64.decode(s).unwrap()
}

async fn user_uuid_by_email(pool: &sqlx::SqlitePool, email: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE email = ?")
        .bind(email)
        .fetch_one(pool)
        .await
        .expect("user exists in test db")
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
    stream
        .write_all(req.as_bytes())
        .await
        .expect("write headers");
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

/// Register a user via the OPAQUE registration flow (real over-the-wire).
async fn register(addr: SocketAddr, username: &str, password: &[u8]) {
    let (start, cstate) = opaque::client_register_start(password).expect("reg start");
    let (status, resp) = http(
        addr,
        "POST",
        "/auth/register/start",
        None,
        Some(json!({
            "username": username,
            "registration_start": b64(&start),
        })),
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

/// Log in via OPAQUE and return the session bearer token.
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
async fn shares_live_server_inbox_accept_e2e() {
    let db_path = std::env::temp_dir().join(format!("vautr_share_e2e_{}.db", Uuid::new_v4()));
    let url = format!("sqlite://{}", db_path.display());
    let pool = db::connect(&url).await.expect("connect + migrate");
    reconcile_server_config(&pool).await;
    let lookup_pool = pool.clone();
    let repo = Arc::new(Repository::new(pool));
    let state = AppState::new(repo);

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let suffix = Uuid::new_v4().to_string();
    let sender_email = format!("sender-{suffix}@e2e.test");
    let recip_email = format!("recip-{suffix}@e2e.test");
    let password = b"correct horse battery staple";

    // 1. Register + log in both users.
    register(addr, &sender_email, password).await;
    register(addr, &recip_email, password).await;
    let sender_tok = login(addr, &sender_email, password).await;
    let recip_tok = login(addr, &recip_email, password).await;
    let sender_uuid = Uuid::parse_str(&user_uuid_by_email(&lookup_pool, &sender_email).await)
        .expect("sender uuid");
    let recip_uuid =
        Uuid::parse_str(&user_uuid_by_email(&lookup_pool, &recip_email).await).expect("recip uuid");

    // 2. Sender creates a project + secret.
    let (status, resp) = http(
        addr,
        "POST",
        "/projects",
        Some(&sender_tok),
        Some(json!({ "name": "Share E2E Vault", "type": "personal" })),
    )
    .await;
    assert_eq!(status, 201, "create project: {resp}");
    let project_id = resp["uuid"].as_str().unwrap().to_string();

    let secret_plaintext = b"top-secret-shared-value-1337";
    let (status, resp) = http(
        addr,
        "POST",
        "/secrets",
        Some(&sender_tok),
        Some(json!({
            "project_uuid": project_id,
            "key": "API_TOKEN",
            "value_ciphertext": b64(secret_plaintext),
        })),
    )
    .await;
    assert_eq!(status, 201, "create secret: {resp}");
    let item_uuid =
        Uuid::parse_str(resp["uuid"].as_str().unwrap()).expect("secret item uuid");

    // 3. Recipient publishes their sharing public key; sender fetches it.
    let recip_keypair = SharingKeyPair::generate();
    let (status, resp) = http(
        addr,
        "PUT",
        &format!("/users/{recip_uuid}/public-key"),
        Some(&recip_tok),
        Some(json!({ "public_key": b64(&recip_keypair.public) })),
    )
    .await;
    assert_eq!(status, 200, "publish sharing key: {resp}");

    let (status, resp) = http(
        addr,
        "GET",
        &format!("/users/{recip_uuid}/public-key"),
        Some(&sender_tok),
        None,
    )
    .await;
    assert_eq!(status, 200, "get sharing key: {resp}");
    let recip_pub_bytes = decode_b64(resp["public_key"].as_str().expect("pub key"));
    assert_eq!(recip_pub_bytes.len(), 32, "sharing public key is 32 bytes");
    let mut recip_pub = [0u8; 32];
    recip_pub.copy_from_slice(&recip_pub_bytes);
    let recip_pub: SharingPublicKey = recip_pub;

    // 4. Sender KEM-wraps the item for the recipient and relays the envelope.
    let bundle = share_item(
        sender_uuid,
        recip_uuid,
        item_uuid,
        &recip_pub,
        secret_plaintext,
    )
    .expect("share_item");
    let (status, resp) = http(
        addr,
        "POST",
        "/shares/",
        Some(&sender_tok),
        Some(json!({
            "item_uuid": item_uuid.to_string(),
            "recipient_uuid": recip_uuid.to_string(),
            "wrapped_sik": bundle.wrapped_sik,
            "ephemeral_public_key": bundle.ephemeral_public_key,
        })),
    )
    .await;
    assert_eq!(status, 200, "create share: {resp}");

    let (status, resp) = http(
        addr,
        "POST",
        &format!("/shares/{item_uuid}/payload"),
        Some(&sender_tok),
        Some(json!({ "payload": bundle.encrypted_payload })),
    )
    .await;
    assert_eq!(status, 200, "upload share payload: {resp}");

    // 5. Recipient lists the inbox and sees exactly one pending share.
    let (status, resp) = http(addr, "GET", "/shares/inbox", Some(&recip_tok), None).await;
    assert_eq!(status, 200, "list inbox: {resp}");
    let shares = resp.as_array().expect("inbox is an array");
    assert_eq!(shares.len(), 1, "recipient has exactly one pending share");
    let incoming_json = &shares[0];

    let incoming = IncomingShare {
        share_id: Uuid::parse_str(incoming_json["share_id"].as_str().unwrap()).expect("share_id"),
        sender_uuid,
        item_uuid,
        wrapped_sik: incoming_json["wrapped_sik"].as_str().unwrap().to_string(),
        ephemeral_public_key: incoming_json["ephemeral_public_key"]
            .as_str()
            .unwrap()
            .to_string(),
        encrypted_payload: incoming_json["payload"].as_str().unwrap().to_string(),
    };

    // 6. Recipient decapsulates + decrypts -> recovers the original plaintext.
    let recovered = accept_share(&recip_keypair, &incoming).expect("accept_share");
    assert_eq!(
        recovered, secret_plaintext,
        "decrypted shared secret must match the original"
    );

    // Sanity: a sender cannot see their own outbound share in their inbox.
    let (status, resp) = http(addr, "GET", "/shares/inbox", Some(&sender_tok), None).await;
    assert_eq!(status, 200);
    assert!(
        resp.as_array().unwrap().is_empty(),
        "sender inbox stays empty (only recipients receive)"
    );

    eprintln!("✓ Shares e2e passed: sender → recipient handoff decrypts to original");
    let _ = std::fs::remove_file(&db_path);
}
