//! End-to-end integration test against a live Vautr server.
//!
//! Requires the server to be running at `VAUTR_API_URL` (or `http://localhost:8080`).
//! Run with: `cargo test -p vautr-desktop --test live_server_e2e`
//!
//! This test proves the full round-trip:
//!   OPAQUE register → login → build VautrClient → unlock → add item →
//!   search → reveal_secret → read_secret (desktop-api gated).
//!
//! An `#[ignore]` variant exists so it is skipped during `cargo test --workspace`.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use base64::Engine;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use uuid::Uuid;
use zeroize::Zeroizing;

use rand::RngCore;
use vautr_app_state::VautrClient;
use vautr_app_state::sharing::{ShareTransport, generate_vault_sharing_keypair};
use vautr_crypto::{aead, kdf, key_tree};
use vautr_desktop::api_client::{self, ApiClient};
use vautr_desktop::auth_client::AuthClient;
use vautr_desktop::state;
use vautr_domain::{DecryptedOverview, DecryptedSecret, DomainModel, ItemMetadata};
use vautr_sharing::{IncomingShare, ShareBundle};

/// Resolve the server base URL from the environment or default to localhost.
fn base_url() -> String {
    std::env::var("VAUTR_API_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

/// Supervisor DB path used by the reaper-proof Rust server runner. The sharing
/// test needs each user's server-assigned UUID (the server generates a random
/// one at registration), so we read it back from this DB by email.
fn server_db_path() -> String {
    std::env::var("VAUTR_E2E_DB").unwrap_or_else(|_| "/tmp/vautr-webauthn-e2e.db".into())
}

/// Look up a registered user's server UUID by email (read-only DB query).
async fn user_id_by_email(email: &str) -> Uuid {
    let url = format!("sqlite://{}?mode=ro", server_db_path());
    let db = Database::connect(&url)
        .await
        .expect("open server db (mode=ro)");
    let stmt = Statement::from_string(
        DbBackend::Sqlite,
        format!("SELECT id FROM users WHERE email = '{email}'"),
    );
    let res = db
        .query_one_raw(stmt)
        .await
        .expect("user lookup")
        .expect("user row must exist");
    let id: String = res.try_get("", "id").expect("id column");
    Uuid::parse_str(&id).expect("valid uuid")
}

/// Generate a unique database path so concurrent test runs do not collide.
fn temp_db_path() -> String {
    let dir = std::env::temp_dir().join("vautr-desktop-e2e");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("test-{}.db", Uuid::new_v4()))
        .to_string_lossy()
        .into()
}

/// A minimal HTTP [`ShareTransport`] that speaks the live server's sharing
/// endpoints (sharing-pki.md §5). Each call carries the bearer token of the
/// *acting* user (sender for post/share, recipient for inbox/revoke).
struct HttpShareTransport {
    base: String,
    /// Token used when the sender acts (post_share / post_share_payload).
    sender_token: String,
    /// Token used when the recipient acts (fetch_inbox / revoke_share).
    recipient_token: String,
    /// Cached recipient public key (set once at construction).
    recipient_pk: std::sync::Mutex<Option<vautr_crypto::sharing::SharingPublicKey>>,
}

impl HttpShareTransport {
    fn bearer(&self, token: &str) -> String {
        format!("Bearer {}", token)
    }
}

impl ShareTransport for HttpShareTransport {
    fn fetch_public_key(
        &self,
        user_uuid: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<vautr_crypto::sharing::SharingPublicKey, String>> + Send>>
    {
        let cached = self.recipient_pk.lock().unwrap().clone();
        if let Some(pk) = cached {
            return Box::pin(async move { Ok(pk) });
        }
        let base = self.base.clone();
        let token = self.recipient_token.clone();
        Box::pin(async move {
            let resp = reqwest::Client::new()
                .get(format!("{base}/users/{user_uuid}/public-key"))
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .map_err(|e| format!("fetch_public_key: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("fetch_public_key: HTTP {}", resp.status()));
            }
            let body: serde_json::Value = resp.json().await.map_err(|e| format!("json: {e}"))?;
            let pk = body
                .get("public_key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing public_key".to_string())?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(pk)
                .map_err(|e| format!("decode pk: {e}"))?;
            if bytes.len() != 32 {
                return Err("public key must be 32 bytes".into());
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            Ok(arr)
        })
    }

    fn post_share(
        &self,
        bundle: &ShareBundle,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let base = self.base.clone();
        let token = self.sender_token.clone();
        let bundle = bundle.clone();
        Box::pin(async move {
            let body = serde_json::json!({
                "item_uuid": bundle.item_uuid.to_string(),
                "recipient_uuid": bundle.recipient_uuid.to_string(),
                "wrapped_sik": bundle.wrapped_sik,
                "ephemeral_public_key": bundle.ephemeral_public_key,
            });
            let resp = reqwest::Client::new()
                .post(format!("{base}/shares/"))
                .header("Authorization", format!("Bearer {token}"))
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("post_share: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("post_share: HTTP {}", resp.status()));
            }
            Ok(())
        })
    }

    fn post_share_payload(
        &self,
        share_id: Uuid,
        payload: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let base = self.base.clone();
        let token = self.sender_token.clone();
        Box::pin(async move {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&payload);
            let body = serde_json::json!({ "payload": b64 });
            let resp = reqwest::Client::new()
                .post(format!("{base}/shares/{share_id}/payload"))
                .header("Authorization", format!("Bearer {token}"))
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("post_share_payload: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("post_share_payload: HTTP {}", resp.status()));
            }
            Ok(())
        })
    }

    fn fetch_inbox(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<IncomingShare>, String>> + Send>> {
        let base = self.base.clone();
        let token = self.recipient_token.clone();
        Box::pin(async move {
            let resp = reqwest::Client::new()
                .get(format!("{base}/shares/inbox"))
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .map_err(|e| format!("fetch_inbox: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("fetch_inbox: HTTP {}", resp.status()));
            }
            let items: Vec<IncomingShare> = resp.json().await.map_err(|e| format!("json: {e}"))?;
            Ok(items)
        })
    }

    fn revoke_share(
        &self,
        share_id: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let base = self.base.clone();
        let token = self.sender_token.clone();
        Box::pin(async move {
            let resp = reqwest::Client::new()
                .delete(format!("{base}/shares/{share_id}"))
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .map_err(|e| format!("revoke_share: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("revoke_share: HTTP {}", resp.status()));
            }
            Ok(())
        })
    }

    fn store_group_wrapped_key(
        &self,
        _wrapped: &vautr_sharing::WrappedGroupKey,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    fn replace_group_wrapped_keys(
        &self,
        _wrapped: Vec<vautr_sharing::WrappedGroupKey>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}

/// Spin up a fresh registered user + unlocked VautrClient (sync-connected).
async fn fresh_user(pw: &str) -> (String, String, Arc<VautrClient>) {
    let base = base_url();
    let auth = AuthClient::new(&base);
    let user = format!("e2e-desktop-{}-{}", Uuid::new_v4(), std::process::id());
    let reg = auth
        .register(&user, pw)
        .await
        .expect("registration should succeed");
    let login = auth
        .login(&user, pw, &reg.kdf_salt)
        .await
        .expect("login should succeed");

    let db_path = temp_db_path();
    let client = state::build_client(&db_path, &base, &login.session_token)
        .await
        .expect("build_client should succeed");

    let mp = Zeroizing::new(pw.to_string());
    let mk = kdf::derive_master_key(&mp, &reg.kdf_salt).expect("MK derive");
    let kek = key_tree::derive_kek(&mk).expect("KEK derive");
    let svk_bytes = aead::decrypt(&kek, &Uuid::nil(), 0, &login.wrapped_svk)
        .expect("SVK unwrap should succeed with correct password");
    let mut svk = Zeroizing::new([0u8; 32]);
    svk.copy_from_slice(&svk_bytes);
    let _dek = key_tree::derive_dek(&svk).expect("DEK derive");
    let local_gen = login.min_enc_key_gen.max(1);
    client
        .unlock_with_password(
            mp,
            &reg.kdf_salt,
            &login.wrapped_svk,
            Uuid::nil(),
            local_gen,
        )
        .await
        .expect("unlock should succeed");
    (user, login.session_token, client)
}

/// Full end-to-end: register → login → unlock → add → search → reveal.
#[tokio::test]
#[ignore = "requires live Vautr server at VAUTR_API_URL or http://localhost:8080"]
async fn full_auth_add_reveal_roundtrip() {
    let base = base_url();
    let auth = AuthClient::new(&base);

    // ── Register a fresh user ──────────────────────────────────────────
    let user = format!("e2e-desktop-{}", Uuid::new_v4());
    let pw = "correct horse battery staple";

    let reg = auth
        .register(&user, pw)
        .await
        .expect("registration should succeed");
    assert_eq!(reg.kdf_salt.len(), 32);
    assert!(
        !reg.recovery_mnemonic.is_empty(),
        "mnemonic must not be empty"
    );

    // ── Login (OPAQUE handshake + fetch wrapped SVK) ───────────────────
    let login = auth
        .login(&user, pw, &reg.kdf_salt)
        .await
        .expect("login should succeed");
    assert!(!login.session_token.is_empty(), "must have a session token");
    assert!(!login.wrapped_svk.is_empty(), "must have a wrapped SVK");
    assert!(
        login.min_enc_key_gen >= 1,
        "server must report a non-zero key generation"
    );

    // ── Build real VautrClient with SQLite ─────────────────────────────
    let db_path = temp_db_path();
    let client = state::build_client(&db_path, &base, &login.session_token)
        .await
        .expect("build_client should succeed");

    // ── Unlock the orchestrator (derive MK/KEK/SVK/DEK) ────────────────
    let mp = Zeroizing::new(pw.to_string());
    let mk = kdf::derive_master_key(&mp, &reg.kdf_salt).expect("MK derive");
    let kek = key_tree::derive_kek(&mk).expect("KEK derive");

    // Derive DEK locally for payload encryption.
    let svk_bytes = aead::decrypt(&kek, &Uuid::nil(), 0, &login.wrapped_svk)
        .expect("SVK unwrap should succeed with correct password");
    assert_eq!(svk_bytes.len(), 32, "SVK must be 32 bytes");
    let mut svk = Zeroizing::new([0u8; 32]);
    svk.copy_from_slice(&svk_bytes);
    let dek = key_tree::derive_dek(&svk).expect("DEK derive");

    let local_gen = login.min_enc_key_gen.max(1);
    client
        .unlock_with_password(
            mp,
            &reg.kdf_salt,
            &login.wrapped_svk,
            Uuid::nil(),
            local_gen,
        )
        .await
        .expect("unlock_with_password should succeed");

    // ── Add an item ────────────────────────────────────────────────────
    let item_uuid = Uuid::new_v4();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let overview = DecryptedOverview {
        uuid: item_uuid,
        title: "E2E Test Item".into(),
        subtitle: "testuser@example.com".into(),
        icon_key: "key".into(),
        urls: vec!["https://example.com".into()],
        updated_at: now,
    };

    let secret = DecryptedSecret {
        password: Zeroizing::new("my-secret-password-123!".to_string()),
        totp: None,
        notes: Zeroizing::new("integration test note".to_string()),
        fields: vec![],
    };

    let secret_json = serde_json::to_vec(&secret).expect("secret serialization");
    let enc_key_gen = client.current_key_gen();
    let payload = aead::encrypt(&dek, &item_uuid, enc_key_gen, &secret_json)
        .expect("payload encryption should succeed");

    let model = DomainModel {
        uuid: item_uuid,
        enc_key_gen,
        overview,
        secret,
        metadata: ItemMetadata {
            created_at: now,
            updated_at: now,
            trashed: false,
        },
    };

    let outcome = client.save_item(model, payload).await;
    match outcome {
        vautr_app_state::worker::TaskOutcome::Committed(_) => {}
        other => panic!("save_item should return Committed, got {other:?}"),
    }

    // ── Search: the item must appear ───────────────────────────────────
    let items = client.search("").await.expect("search should succeed");
    let found = items.iter().find(|i| i.uuid == item_uuid);
    assert!(
        found.is_some(),
        "added item must be in search results, got {:?}",
        items.iter().map(|i| &i.title).collect::<Vec<_>>()
    );
    let found = found.unwrap();
    assert_eq!(found.title, "E2E Test Item");
    assert_eq!(found.subtitle, "testuser@example.com");

    // ── Reveal + read_secret (desktop-api gated) ───────────────────────
    let handle = client
        .reveal_secret(item_uuid)
        .await
        .expect("reveal_secret should succeed");
    let secret_str = client
        .read_secret(handle)
        .expect("read_secret should succeed (desktop-api)");
    assert_eq!(
        secret_str.as_str(),
        "my-secret-password-123!",
        "revealed secret must match original"
    );

    // ── Cleanup ────────────────────────────────────────────────────────
    let _ = std::fs::remove_file(&db_path);
    eprintln!("✓ E2E round-trip passed for user '{user}'");
}

/// Derive the local DEK from the login material, matching the desktop unlock.
fn derive_dek(password: &str, kdf_salt: &[u8; 32], wrapped_svk: &[u8]) -> Zeroizing<[u8; 32]> {
    let mp = Zeroizing::new(password.to_string());
    let mk = kdf::derive_master_key(&mp, kdf_salt).expect("MK derive");
    let kek = key_tree::derive_kek(&mk).expect("KEK derive");
    let svk_bytes = aead::decrypt(&kek, &Uuid::nil(), 0, wrapped_svk).expect("SVK unwrap");
    let mut svk = Zeroizing::new([0u8; 32]);
    svk.copy_from_slice(&svk_bytes);
    key_tree::derive_dek(&svk).expect("DEK derive")
}

/// Projects/Secrets E2E: register → login → create project → create secret →
/// reveal secret (decrypt locally) → assert.
#[tokio::test]
#[ignore = "requires live Vautr server at VAUTR_API_URL or http://localhost:8080"]
async fn projects_and_secrets_roundtrip() {
    let base = base_url();
    let auth = AuthClient::new(&base);

    // ── Register a fresh user ─────────────────────────────────────────
    let user = format!("e2e-proj-{}", Uuid::new_v4());
    let pw = "correct horse battery staple";
    let reg = auth
        .register(&user, pw)
        .await
        .expect("registration should succeed");
    let login = auth
        .login(&user, pw, &reg.kdf_salt)
        .await
        .expect("login should succeed");
    let token = login.session_token.clone();
    let dek = derive_dek(pw, &reg.kdf_salt, &login.wrapped_svk);

    let api = ApiClient::new(&base);

    // ── Create a project ──────────────────────────────────────────────
    let project = api
        .create_project(&token, "E2E Project", "shared", Some("desktop e2e"))
        .await
        .expect("create project should succeed");
    assert_eq!(project.name, "E2E Project");
    assert_eq!(project.kind, "shared");

    let projects = api
        .list_projects(&token)
        .await
        .expect("list projects should succeed");
    assert!(
        projects.iter().any(|p| p.uuid == project.uuid),
        "created project must appear in list"
    );

    // Members endpoint must respond (a fresh project lists no explicit grants).
    let _members = api
        .list_members(&token, &project.uuid)
        .await
        .expect("list members should succeed");

    // ── Create a secret (value encrypted client-side, zero-knowledge) ─
    let key = "DATABASE_URL";
    let plaintext = "postgres://secret-db:5432/vault";
    let ad = api_client::secret_ad(&project.uuid, key);
    let mut nonce = [0u8; aead::NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = aead::encrypt_with_nonce(&dek, &nonce, &ad, plaintext.as_bytes())
        .expect("encrypt secret value");
    let secret = api
        .create_secret(&token, &project.uuid, key, &api_client::b64_encode(&ct))
        .await
        .expect("create secret should succeed");
    assert_eq!(secret.key, key);

    let secrets = api
        .list_secrets(&token, &project.uuid)
        .await
        .expect("list secrets should succeed");
    assert!(
        secrets.iter().any(|s| s.uuid == secret.uuid),
        "created secret must appear in project list"
    );

    // ── Reveal + decrypt locally, assert the plaintext round-trips ────
    let value = api
        .get_secret_value(&token, &secret.uuid)
        .await
        .expect("get secret value should succeed");
    assert_eq!(value.key, key, "reveal must return the same key");

    let ct2 = api_client::b64_decode(&value.value_ciphertext).expect("decode ciphertext");
    let plain = aead::decrypt_with_ad(&dek, &ad, &ct2).expect("decrypt secret value");
    let revealed = String::from_utf8(plain).expect("plaintext is UTF-8");
    assert_eq!(revealed, plaintext, "revealed secret must match original");

    eprintln!("✓ Projects/Secrets E2E passed for user '{user}'");
}

// ── Lock / unlock (VTR-104 surface) ──────────────────────────────────────

#[tokio::test]
#[ignore = "requires live Vautr server at VAUTR_API_URL or http://localhost:8080"]
async fn lock_then_reveal_is_refused() {
    let pw = "correct horse battery staple";
    let (_user, _token, client) = fresh_user(pw).await;

    // Add + reveal while unlocked.
    let item_uuid = Uuid::new_v4();
    let now = 0i64;
    let overview = DecryptedOverview {
        uuid: item_uuid,
        title: "Lock Test Item".into(),
        subtitle: "lock@example.com".into(),
        icon_key: "key".into(),
        urls: vec![],
        updated_at: now,
    };
    let secret = DecryptedSecret {
        password: Zeroizing::new("lock-test-pw-123!".to_string()),
        totp: None,
        notes: Zeroizing::new("n".to_string()),
        fields: vec![],
    };
    let secret_json = serde_json::to_vec(&secret).expect("serialize");
    let enc_key_gen = client.current_key_gen();
    let svk = client.current_svk().await.expect("svk present");
    let dek = key_tree::derive_dek(&svk).expect("dek derive");
    let payload = aead::encrypt(&dek, &item_uuid, enc_key_gen, &secret_json).expect("encrypt");

    let model = DomainModel {
        uuid: item_uuid,
        enc_key_gen,
        overview,
        secret,
        metadata: ItemMetadata {
            created_at: now,
            updated_at: now,
            trashed: false,
        },
    };
    let outcome = client.save_item(model, payload).await;
    match outcome {
        vautr_app_state::worker::TaskOutcome::Committed(_) => {}
        other => panic!("save_item should Commit, got {other:?}"),
    }

    let handle = client
        .reveal_secret(item_uuid)
        .await
        .expect("reveal works while unlocked");
    let s = client
        .read_secret(handle)
        .expect("read works while unlocked");
    assert_eq!(s.as_str(), "lock-test-pw-123!", "unlocked reveal matches");

    // Now lock: reveal + read_secret must be refused.
    client.lock().await;
    assert!(client.is_locked(), "client reports locked");
    let reveal = client.reveal_secret(item_uuid).await;
    assert!(reveal.is_err(), "reveal must fail while locked");
    assert_eq!(reveal.unwrap_err(), "vault locked");

    eprintln!("✓ Lock/Unlock E2E passed");
}

// ── Master-key rotation (VTR-104 surface) ──────────────────────────────────

#[tokio::test]
#[ignore = "requires live Vautr server at VAUTR_API_URL or http://localhost:8080"]
async fn rotate_key_advances_epoch() {
    let pw = "correct horse battery staple";
    let (_user, _token, client) = fresh_user(pw).await;

    let before = client.current_key_gen();
    // Rotate to a strictly higher epoch; the orchestrator re-wraps the SVK
    // internally under the freshly-derived KEK before pushing to the server.
    let new_gen = before + 1;
    client
        .rotate_key(new_gen)
        .await
        .expect("rotate_key should succeed against live server");

    assert!(
        client.current_key_gen() >= new_gen,
        "epoch must advance after rotation (was {before}, now {})",
        client.current_key_gen()
    );

    eprintln!(
        "✓ Rotate-key E2E passed (epoch {before} → {})",
        client.current_key_gen()
    );
}

// ── 1:1 share → accept round-trip against the live server (VTR-104) ────────

#[tokio::test]
#[ignore = "requires live Vautr server at VAUTR_API_URL or http://localhost:8080"]
async fn share_accept_roundtrip_live() {
    let base = base_url();
    let pw = "correct horse battery staple";

    // Two distinct users (sender + recipient) on the live server.
    let (sender_user, sender_token, sender) = fresh_user(pw).await;
    let (recip_user, recip_token, recipient) = fresh_user(pw).await;

    // Resolve server UUIDs (server assigns random ids at registration).
    let sender_id = user_id_by_email(&sender_user).await;
    let recip_id = user_id_by_email(&recip_user).await;

    // Each client must know its own server user id (AD binding).
    sender.set_server_user_id(sender_id).await;
    recipient.set_server_user_id(recip_id).await;

    // Generate + install sharing keypairs. The recipient's public key is
    // cached in the transport; the sender looks it up via the live directory.
    let kp_s = generate_vault_sharing_keypair();
    let kp_r = generate_vault_sharing_keypair();
    sender.set_sharing_keypair(kp_s).await;
    recipient.set_sharing_keypair(kp_r.clone()).await;

    // Wire an HTTP sharing transport (speaks the live server's §5 endpoints).
    let transport = Arc::new(HttpShareTransport {
        base: base.clone(),
        sender_token: sender_token.clone(),
        recipient_token: recip_token.clone(),
        recipient_pk: std::sync::Mutex::new(Some(kp_r.public)),
    });
    sender.connect_sharing(transport.clone()).await;
    recipient.connect_sharing(transport.clone()).await;

    // Sender shares an item's plaintext to the recipient.
    let item_uuid = Uuid::new_v4();
    let plaintext = b"shared-live-secret-payload";
    let bundle = sender
        .share_item(recip_id, item_uuid, plaintext)
        .await
        .expect("sender shares to recipient");

    // Recipient sees it in the inbox.
    let inbox = recipient
        .fetch_shares()
        .await
        .expect("recipient fetches inbox");
    assert!(
        inbox.iter().any(|s| s.share_id == bundle.share_id),
        "recipient inbox must contain the share"
    );

    // Recipient decrypts it locally (SIK decapsulation under its keypair).
    let incoming = inbox
        .into_iter()
        .find(|s| s.share_id == bundle.share_id)
        .unwrap();
    let decrypted = recipient
        .accept_share(&incoming)
        .await
        .expect("recipient accepts + decrypts");
    assert_eq!(
        decrypted, plaintext,
        "recipient recovers the shared plaintext"
    );

    // Revoking removes it from the inbox.
    recipient
        .revoke_share(bundle.share_id)
        .await
        .expect("revoke");
    let after = recipient.fetch_shares().await.expect("fetch again");
    assert!(
        !after.iter().any(|s| s.share_id == bundle.share_id),
        "revoked share must leave the inbox"
    );

    eprintln!("✓ Share/Accept (live server) E2E passed");
}
