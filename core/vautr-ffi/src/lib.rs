//! # vautr-ffi
//!
//! UniFFI 0.31 bindings exposing `VautrClient` to Swift/Kotlin (React Native
//! mobile, build-env-deploy §3.1). The opaque-handle pattern (ADR-003) means
//! secrets are never passed as strings across the bridge;
//! `perform_action(handle)` / `read_secret(handle)` (desktop only) service
//! copy/autofill natively.
//!
//! Mobile boot surface: `MobileClient::initialize` (links the core + migrations),
//! `unlock` / `unlock_with_password`, `list_overviews`, `reveal_secret`, `lock`,
//! `sync`, plus a `SecureEnclaveBridge` for OS-keystore biometric storage of the
//! SVK.
//!
//! `read_secret` is gated behind the `desktop-api` feature (data.md §1 rule 4,
//! build-env-deploy §2.5) so it never compiles into the mobile artifact.

uniffi::setup_scaffolding!();

pub mod client;
pub mod sharing;

pub use client::*;

/// Run the in-crate integration tests:
/// `cargo test -p vautr-ffi -- --nocapture`
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::{Database, Set, TransactionTrait};
    use uuid::Uuid;
    use zeroize::Zeroizing;

    use super::client::{
        CoreAction, FfiError, MobileClient, PlatformActionHandler, SecureEnclaveBridge,
    };
    use vautr_crypto::{aead, key_tree};
    use vautr_db::entity::{item_overview, item_payload};
    use vautr_db::migrate;
    use vautr_domain::DecryptedSecret;

    /// A test-only Secure Enclave bridge backed by an in-memory `Vec<u8>`,
    /// exercising the native-keystore trait surface from Rust.
    struct MockEnclave {
        svk: std::sync::RwLock<Option<Vec<u8>>>,
    }

    impl SecureEnclaveBridge for MockEnclave {
        fn save_svk(&self, svk: Vec<u8>) -> Result<(), FfiError> {
            *self.svk.write().unwrap() = Some(svk);
            Ok(())
        }
        fn load_svk(&self) -> Result<Option<Vec<u8>>, FfiError> {
            Ok(self.svk.read().unwrap().clone())
        }
        fn delete_svk(&self) -> Result<(), FfiError> {
            *self.svk.write().unwrap() = None;
            Ok(())
        }
        fn has_svk(&self) -> Result<bool, FfiError> {
            Ok(self.svk.read().unwrap().is_some())
        }
    }

    /// Seed one encrypted item directly into the DB (mirrors phase_abc.rs).
    async fn seed_item(
        db: &sea_orm::DatabaseConnection,
        uuid: Uuid,
        gen: u64,
        password: &str,
        dek: &[u8; 32],
    ) {
        let txn = db.begin().await.expect("begin");
        let overview = item_overview::ActiveModel {
            uuid: Set(uuid.to_string()),
            version: Set(1),
            enc_key_gen: Set(gen as i64),
            deleted_date: Set(None),
            overview_title: Set("Example".into()),
            overview_subtitle: Set("login".into()),
            overview_icon_key: Set("web".into()),
            overview_urls: Set("[\"https://example.com\"]".into()),
            created_at: Set(0),
            updated_at: Set(0),
        };
        let secret = DecryptedSecret {
            password: Zeroizing::new(password.to_string()),
            totp: None,
            notes: Zeroizing::new(String::new()),
            fields: vec![],
        };
        let pt = serde_json::to_vec(&secret).expect("serialize");
        let env = aead::encrypt(dek, &uuid, gen, &pt).expect("encrypt");
        let payload = item_payload::ActiveModel {
            uuid: Set(uuid.to_string()),
            payload: Set(env),
        };
        vautr_db::txn::save_item_txn(&txn, overview, payload, &uuid.to_string())
            .await
            .expect("save");
        txn.commit().await.expect("commit");
    }

    /// Full mobile flow: initialize -> unlock -> list_overviews -> reveal_secret
    /// -> (release on detail unmount) -> lock. Confirms `release_secret` zeroizes
    /// the handle so a post-release action is rejected, mirroring the Detail
    /// screen's unmount cleanup (ui-state-charts §3).
    #[tokio::test]
    async fn mobile_unlock_list_reveal_release_lock() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir
            .path()
            .join("vault.sqlite3")
            .to_string_lossy()
            .into_owned();

        // initialize links the core and runs migrations on the fresh vault DB.
        let client = MobileClient::initialize(db_path.clone())
            .await
            .expect("initialize");
        assert!(client.is_locked(), "fresh vault must start locked");

        // Register a Secure Enclave bridge and persist the SVK.
        let enclave = Arc::new(MockEnclave {
            svk: std::sync::RwLock::new(None),
        });
        client.set_secure_enclave_bridge(enclave.clone());
        assert!(client.secure_enclave_bridge().is_some());

        let svk = key_tree::generate_svk();
        enclave.save_svk(svk.to_vec()).expect("enclave save");
        assert!(enclave.has_svk().expect("has_svk"));

        // Seed a secret at key generation 1, decryptable under the SVK.
        let dek = key_tree::derive_dek(&svk).expect("dek");
        let uuid = Uuid::new_v4();
        let db = Database::connect(&format!("sqlite://{db_path}"))
            .await
            .expect("connect");
        seed_item(&db, uuid, 1, "hunter2", &dek).await;

        // Unlock via the recovered SVK (biometric path).
        let svk_vec = enclave.load_svk().expect("load").expect("present");
        client.unlock(svk_vec.clone(), 1).await.expect("unlock");

        // list_overviews returns the seeded item as a JSON array.
        let list = client.list_overviews().await.expect("list");
        let parsed: Vec<vautr_domain::DecryptedOverview> =
            serde_json::from_str(&list).expect("parse list");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].uuid, uuid);

        // reveal -> opaque handle (u64 surfaced to TS as a string).
        let handle = client
            .reveal_secret(uuid.to_string())
            .await
            .expect("reveal");
        assert_ne!(handle, 0);

        // "Unmount" the Detail screen: the component's cleanup calls
        // release_secret (ui-state-charts §3). After release the handle is
        // zeroized and a subsequent action is rejected.
        client.release_secret(handle).expect("release");
        let after_release = client
            .perform_action(CoreAction::CopyToClipboard { handle })
            .await;
        assert!(after_release.is_err(), "released handle must be rejected");

        // lock wipes in-memory keys.
        client.lock().await;
        assert!(client.is_locked());
        let db2 = Database::connect(&format!("sqlite://{db_path}"))
            .await
            .expect("connect2");
        migrate::init(&db2).await.expect("migrate idempotent");
    }

    /// A locked vault must reject list/reveal (reads require unlock).
    #[tokio::test]
    async fn mobile_locked_rejects_reads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir
            .path()
            .join("vault2.sqlite3")
            .to_string_lossy()
            .into_owned();
        let client = MobileClient::initialize(db_path).await.expect("initialize");
        assert!(client
            .reveal_secret(Uuid::new_v4().to_string())
            .await
            .is_err());
    }

    /// VTR-048 (TDD1/2 in Rust): a revealed secret rendered into the native
    /// overlay is delivered as plaintext ONLY to the `PlatformActionHandler`
    /// (the native overlay component), and never crosses into JS. The test
    /// stands in for the Kotlin/Swift `on_action` implementation: it captures
    /// the `(action, secret)` the core delegates and asserts the plaintext
    /// reached native code, with the opaque handle (not the string) being the
    /// only thing JS would have held.
    #[tokio::test]
    async fn overlay_renders_plaintext_only_in_native_handler() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir
            .path()
            .join("vault3.sqlite3")
            .to_string_lossy()
            .into_owned();
        let client = MobileClient::initialize(db_path.clone())
            .await
            .expect("initialize");

        let enclave = Arc::new(MockEnclave {
            svk: std::sync::RwLock::new(None),
        });
        client.set_secure_enclave_bridge(enclave.clone());
        let svk = key_tree::generate_svk();
        enclave.save_svk(svk.to_vec()).expect("enclave save");
        let dek = key_tree::derive_dek(&svk).expect("dek");

        let db = Database::connect(&format!("sqlite://{db_path}"))
            .await
            .expect("connect");
        let uuid = Uuid::new_v4();
        seed_item(&db, uuid, 1, "s3cr3t-overlay", &dek).await;
        let svk_vec = enclave.load_svk().expect("load").expect("present");
        client.unlock(svk_vec, 1).await.expect("unlock");

        // Capture the native action+secret delegated by the overlay render.
        let captured: Arc<std::sync::RwLock<Option<(CoreAction, String)>>> =
            Arc::new(std::sync::RwLock::new(None));
        let handler = Arc::new(MockActionHandler {
            captured: captured.clone(),
        });
        client.set_platform_handler(handler).await;

        let handle = client
            .reveal_secret(uuid.to_string())
            .await
            .expect("reveal");
        // JS holds only the opaque handle (u64 as string) — never the secret.
        assert_ne!(handle, 0);

        client
            .render_secret_in_overlay(handle)
            .await
            .expect("overlay render");

        let got = captured.read().unwrap().clone().expect("handler called");
        match got.0 {
            CoreAction::RenderInOverlay { handle: h } => assert_eq!(h, handle),
            other => panic!("expected RenderInOverlay, got {other:?}"),
        }
        assert_eq!(got.1, "s3cr3t-overlay");

        // Unmounting the overlay releases the handle (zeroizes the secret).
        client.release_secret(handle).expect("release");
        let after_release = client.render_secret_in_overlay(handle).await;
        assert!(after_release.is_err(), "released handle must be rejected");
    }

    /// Mock native `PlatformActionHandler`: records the `(action, secret)` the
    /// core delegates for the overlay (or clipboard/autofill) so tests prove the
    /// plaintext reached native code and not JS.
    struct MockActionHandler {
        captured: Arc<std::sync::RwLock<Option<(CoreAction, String)>>>,
    }

    impl PlatformActionHandler for MockActionHandler {
        fn on_action(&self, action: CoreAction, secret: String) {
            *self.captured.write().unwrap() = Some((action, secret));
        }
    }

    /// FFI sharing roundtrip: a sender shares an item to a recipient's public
    /// key via `ffi_share_item`, the recipient accepts via `ffi_accept_share`
    /// using their persisted sharing secret, and recovers the plaintext. This
    /// exercises the exact uniffi boundary the React Native layer uses.
    #[test]
    fn ffi_share_accept_roundtrip() {
        use crate::sharing::{ffi_accept_share, ffi_share_item};
        use base64::Engine;
        use vautr_crypto::sharing::SharingKeyPair;

        let sender = SharingKeyPair::generate();
        let recipient = SharingKeyPair::generate();
        let recipient_pub_b64 = base64::engine::general_purpose::STANDARD.encode(recipient.public);
        let recipient_secret_b64 =
            base64::engine::general_purpose::STANDARD.encode(recipient.secret_bytes());

        let sender_uuid = Uuid::new_v4().to_string();
        let recipient_uuid = Uuid::new_v4().to_string();
        let item_uuid = Uuid::new_v4().to_string();
        let plaintext = b"mobile shared secret".to_vec();

        let bundle = ffi_share_item(
            sender_uuid,
            recipient_uuid,
            item_uuid.clone(),
            recipient_pub_b64,
            plaintext.clone(),
        )
        .expect("share_item");

        // The relay delivers an IncomingShare to the recipient.
        let incoming = serde_json::json!({
            "share_id": bundle.share_id,
            "sender_uuid": bundle.sender_uuid,
            "item_uuid": bundle.item_uuid,
            "wrapped_sik": bundle.wrapped_sik,
            "ephemeral_public_key": bundle.ephemeral_public_key,
            "encrypted_payload": bundle.encrypted_payload,
        });
        let incoming_json = serde_json::to_string(&incoming).unwrap();

        let recovered = ffi_accept_share(incoming_json, recipient_secret_b64).expect("accept_share");
        assert_eq!(recovered, plaintext);
        let _ = sender;
    }

    /// Group sharing roundtrip at the FFI boundary: admin creates a group, adds
    /// a member (wraps the Group SIK for their public key), the member unwraps
    /// it, and both encrypt/decrypt a group item under the Group SIK.
    #[test]
    fn ffi_group_share_roundtrip() {
        use crate::sharing::{
            ffi_add_group_member, ffi_create_group, ffi_decrypt_group_item, ffi_encrypt_group_item,
            ffi_unwrap_group_key,
        };
        use base64::Engine;
        use vautr_crypto::sharing::SharingKeyPair;

        let admin = SharingKeyPair::generate();
        let member = SharingKeyPair::generate();
        let member_pub_b64 = base64::engine::general_purpose::STANDARD.encode(member.public);
        let member_secret_b64 =
            base64::engine::general_purpose::STANDARD.encode(member.secret_bytes());

        let group = ffi_create_group("Mobile Team".into(), Uuid::new_v4().to_string())
            .expect("create_group");

        let wrapped = ffi_add_group_member(
            group_json(&group),
            Uuid::new_v4().to_string(),
            member_pub_b64,
        )
        .expect("add_group_member");

        // Member unwraps the Group SIK into their own persisted key.
        let member_key_json = ffi_unwrap_group_key(
            serde_json::to_string(&serde_json::json!({
                "group_id": wrapped.group_id,
                "member_uuid": wrapped.member_uuid,
                "wrapped_sik": wrapped.wrapped_sik,
                "ephemeral_public_key": wrapped.ephemeral_public_key,
            }))
            .unwrap(),
            member_secret_b64,
        )
        .expect("unwrap_group_key");

        let item_uuid = Uuid::new_v4().to_string();
        let plaintext = b"group item payload".to_vec();

        // Admin encrypts under the Group SIK.
        let ct = ffi_encrypt_group_item(group_json(&group), item_uuid.clone(), plaintext.clone())
            .expect("encrypt");
        // Member decrypts using their unwrapped key.
        let recovered = ffi_decrypt_group_item(member_key_json, item_uuid, ct).expect("decrypt");
        assert_eq!(recovered, plaintext);
        let _ = admin;
    }

    /// Helper: serialize an `FfiGroupKey` into the `{ group, secret_b64 }` JSON
    /// the FFI functions accept.
    fn group_json(g: &crate::sharing::FfiGroupKey) -> String {
        serde_json::to_string(&serde_json::json!({
            "group_id": g.group_id,
            "name": g.name,
            "admin_uuid": g.admin_uuid,
            "secret_b64": g.secret_b64,
        }))
        .unwrap()
    }
}
