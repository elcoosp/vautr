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

pub use client::*;

/// Run the in-crate integration tests:
/// `cargo test -p vautr-ffi -- --nocapture`
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::{Database, Set, TransactionTrait};
    use uuid::Uuid;
    use zeroize::Zeroizing;

    use super::client::{CoreAction, FfiError, MobileClient, SecureEnclaveBridge};
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
        let db_path = dir.path().join("vault.sqlite3").to_string_lossy().into_owned();

        // initialize links the core and runs migrations on the fresh vault DB.
        let client = MobileClient::initialize(db_path.clone()).await.expect("initialize");
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
        let handle = client.reveal_secret(uuid.to_string()).await.expect("reveal");
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
        let db_path = dir.path().join("vault2.sqlite3").to_string_lossy().into_owned();
        let client = MobileClient::initialize(db_path).await.expect("initialize");
        assert!(client.reveal_secret(Uuid::new_v4().to_string()).await.is_err());
    }
}

