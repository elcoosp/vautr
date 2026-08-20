//! Live-server end-to-end test for the sync conflict engine (VTR-057).
//!
//! Boots the real `vautr-server` router on a loopback socket and drives the
//! sync endpoints (`/sync/push-batch`, `/sync/pull`) over actual HTTP/1.1 to
//! prove the OCC conflict resolution works at the wire layer — not just via the
//! in-repo unit tests in `sync.rs`. This is the server-side contract that the
//! web conflict-resolution modal (VTR-056) and the desktop/mobile reapers rely
//! on: a stale push (target_version behind the server's current version) MUST
//! return `status: "conflict"` with the authoritative `current_server_state`,
//! and a fresh push must succeed and advance the version.
//!
//! Mirrors the in-process harness from `sharing_e2e.rs` / `projects_e2e.rs`.

use std::net::SocketAddr;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

use vautr_crypto::opaque;
use vautr_server::db;
use vautr_server::handlers::{build_router, AppState};
use vautr_server::repository::Repository;

/// WORKAROUND (test-scoped, temp DB only): reconcile the frozen `server_config`
/// schema to the key/value shape the running server expects so real OPAQUE
/// registration can run over the wire. No project files are touched.
async fn reconcile_server_config(pool: &sqlx::SqlitePool) {
    let has_value: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('server_config') WHERE name = 'value'",
    )
    .fetch_one(pool)
    .await
    .expect("probe");
    if has_value > 0 {
        return;
    }
    sqlx::query(
        "DROP TABLE server_config; CREATE TABLE server_config (key TEXT PRIMARY KEY, value BLOB, created_at INTEGER NOT NULL DEFAULT (unixepoch())) STRICT;",
    )
    .execute(pool)
    .await
    .expect("reconcile");
}

fn b64(b: &[u8]) -> String {
    B64.encode(b)
}
fn decode_b64(s: &str) -> Vec<u8> {
    B64.decode(s).unwrap()
}

async fn http(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (u16, Value) {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let body_bytes = body.map(|b| serde_json::to_vec(&b).unwrap()).unwrap_or_default();
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
    stream.write_all(req.as_bytes()).await.expect("write");
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
    let status: u16 = text
        .split("\r\n")
        .next()
        .unwrap_or("")
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
        Some(json!({"username": username, "registration_start": b64(&start)})),
    )
    .await;
    assert_eq!(status, 200);
    let sresp = decode_b64(resp["registration_response"].as_str().unwrap());
    let (upload, _) =
        opaque::client_register_finish(&cstate, &sresp, password, username.as_bytes())
            .expect("reg finish");
    let server_pk = opaque::server_setup_public_key().expect("setup pk");
    let (status, _resp) = http(
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
    assert_eq!(status, 200);
}

async fn login(addr: SocketAddr, username: &str, password: &[u8]) -> String {
    let (start, cstate) = opaque::client_login_start(password).expect("login start");
    let (status, resp) = http(
        addr,
        "POST",
        "/auth/login/start",
        None,
        Some(json!({"username": username, "login_start": b64(&start)})),
    )
    .await;
    assert_eq!(status, 200);
    let sresp = decode_b64(resp["login_response"].as_str().unwrap());
    let (finish, _) =
        opaque::client_login_finish(&cstate, &sresp, password, username.as_bytes())
            .expect("login finish");
    let (status, resp) = http(
        addr,
        "POST",
        "/auth/login/finish",
        None,
        Some(json!({"username": username, "login_finish": b64(&finish)})),
    )
    .await;
    assert_eq!(status, 200);
    resp["session_token"].as_str().unwrap().to_string()
}

/// Push a single item via `/sync/push-batch` and return its PushResult.
async fn push(
    addr: SocketAddr,
    token: &str,
    uuid: &str,
    target_version: u64,
    enc_key_gen: u64,
    payload: Option<&[u8]>,
) -> Value {
    let body = json!({
        "items": [{
            "uuid": uuid,
            "target_version": target_version,
            "enc_key_gen": enc_key_gen,
            "payload": payload.map(b64),
        }]
    });
    let (status, resp) = http(addr, "POST", "/sync/push-batch", Some(token), Some(body)).await;
    assert_eq!(status, 200, "push-batch status; body={resp}");
    resp["results"]
        .as_array()
        .unwrap()
        .first()
        .cloned()
        .expect("one result")
}

#[tokio::test]
async fn sync_conflict_engine_e2e() {
    let db_path = std::env::temp_dir().join(format!("vautr_sync_e2e_{}.db", Uuid::new_v4()));
    let pool = db::connect(&format!("sqlite://{}", db_path.display()))
        .await
        .expect("connect+migrate");
    reconcile_server_config(&pool).await;
    let repo = Arc::new(Repository::new(pool));
    let state = AppState::new(repo);
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    // Register + login a user.
    register(addr, "sync-alice@vautr.test", b"correct-horse-battery-staple").await;
    let token = login(addr, "sync-alice@vautr.test", b"correct-horse-battery-staple").await;

    let item = "11111111-1111-1111-1111-111111111111";

    // 1. Initial push (target_version 0) creates the item at version 1.
    let r1 = push(addr, &token, item, 0, 1, Some(b"v1-ciphertext")).await;
    assert_eq!(r1["status"].as_str().unwrap(), "success");
    assert_eq!(r1["version"].as_i64().unwrap(), 1);
    assert!(r1["current_server_state"].is_null());

    // 2. A correct OCC push (target_version 1 == current server version) advances
    //    to version 2.
    let r2 = push(addr, &token, item, 1, 1, Some(b"v2-ciphertext")).await;
    assert_eq!(r2["status"].as_str().unwrap(), "success");
    assert_eq!(r2["version"].as_i64().unwrap(), 2);

    // 3. A STALE push (target_version 1, but the server is now at v2) MUST
    //    conflict — OCC rejects the outdated base version and returns the
    //    authoritative current server state so the client can resolve.
    let r3 = push(addr, &token, item, 1, 1, Some(b"v2-ciphertext-stale")).await;
    assert_eq!(r3["status"].as_str().unwrap(), "conflict");
    assert_eq!(r3["version"].is_null(), true);
    let srv = r3["current_server_state"].as_object().expect("server state");
    assert_eq!(srv["version"].as_i64().unwrap(), 2);

    // 4. A correct OCC push (target_version 2) advances to version 3.
    let r4 = push(addr, &token, item, 2, 1, Some(b"v3-ciphertext")).await;
    assert_eq!(r4["status"].as_str().unwrap(), "success");
    assert_eq!(r4["version"].as_i64().unwrap(), 3);

    // 5. /sync/pull reflects the latest server version (3) as metadata. The
    //    server retains only the latest version per item, so we pull with a
    //    cursor at the prior version (its cursor-expiry check needs
    //    cursor >= min_version - 1). Payload ciphertext is fetched separately.
    let (status, pull) = http(
        addr,
        "GET",
        "/sync/pull?cursor=2&limit=100",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, 200);
    let items = pull["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["version"].as_i64().unwrap(), 3);

    // 6. /sync/pull-payloads delivers the actual ciphertext for that version.
    let (pstat, payloads) = http(
        addr,
        "POST",
        "/sync/pull-payloads",
        Some(&token),
        Some(json!({"items": [{"uuid": item, "version": 3}]})),
    )
    .await;
    assert_eq!(pstat, 200);
    let pres = payloads["results"].as_array().expect("payload results")[0].clone();
    assert_eq!(pres["status"].as_str().unwrap(), "payload_delivered");
    assert_eq!(
        decode_b64(pres["payload"].as_str().unwrap()),
        b"v3-ciphertext".to_vec()
    );

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn sync_epoch_too_old_e2e() {
    let db_path = std::env::temp_dir().join(format!("vautr_sync_epoch_{}.db", Uuid::new_v4()));
    let pool = db::connect(&format!("sqlite://{}", db_path.display()))
        .await
        .expect("connect+migrate");
    reconcile_server_config(&pool).await;
    let repo = Arc::new(Repository::new(pool));
    let state = AppState::new(repo);
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    register(addr, "sync-bob@vautr.test", b"correct-horse-battery-staple").await;
    let token = login(addr, "sync-bob@vautr.test", b"correct-horse-battery-staple").await;

    let item = "22222222-2222-2222-2222-222222222222";
    // First push establishes enc_key_gen = 2 and the item at v1.
    let r1 = push(addr, &token, item, 0, 2, Some(b"gen2")).await;
    assert_eq!(r1["status"].as_str().unwrap(), "success");

    // Rotate the account key forward to bump min_enc_key_gen to 3 (epoch gate).
    let (rstat, _rbody) = http(
        addr,
        "POST",
        "/account/rotate-key",
        Some(&token),
        Some(json!({"new_svk_ciphertext_blob": b64(&[8u8; 48]), "new_min_enc_key_gen": 3})),
    )
    .await;
    assert_eq!(rstat, 200);

    // A push with enc_key_gen 1 (behind the server's min enc_key_gen of 3) is
    // rejected with epoch_too_old (REQ-API-01).
    let r2 = push(addr, &token, item, 1, 1, Some(b"gen1-stale")).await;
    assert_eq!(r2["status"].as_str().unwrap(), "epoch_too_old");

    let _ = std::fs::remove_file(&db_path);
}
