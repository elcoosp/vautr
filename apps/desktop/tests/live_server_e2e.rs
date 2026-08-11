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

use vautr_crypto::{aead, kdf, key_tree};
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
