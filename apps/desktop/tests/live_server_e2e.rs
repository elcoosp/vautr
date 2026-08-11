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

use uuid::Uuid;
use zeroize::Zeroizing;

use rand::RngCore;
use vautr_crypto::{aead, kdf, key_tree};
use vautr_desktop::api_client::{self, ApiClient};
use vautr_desktop::auth_client::AuthClient;
use vautr_desktop::state;
use vautr_domain::{DecryptedOverview, DecryptedSecret, DomainModel, ItemMetadata};

/// Resolve the server base URL from the environment or default to localhost.
fn base_url() -> String {
    std::env::var("VAUTR_API_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

/// Generate a unique database path so concurrent test runs do not collide.
fn temp_db_path() -> String {
    let dir = std::env::temp_dir().join("vautr-desktop-e2e");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("test-{}.db", Uuid::new_v4()))
        .to_string_lossy()
        .into()
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
    assert!(!reg.recovery_mnemonic.is_empty(), "mnemonic must not be empty");

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
        .unlock_with_password(mp, &reg.kdf_salt, &login.wrapped_svk, Uuid::nil(), local_gen)
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
fn derive_dek(
    password: &str,
    kdf_salt: &[u8; 32],
    wrapped_svk: &[u8],
) -> Zeroizing<[u8; 32]> {
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
        .create_secret(
            &token,
            &project.uuid,
            key,
            &api_client::b64_encode(&ct),
        )
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
