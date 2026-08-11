//! Integration tests for Phases A/B/C of `VautrClient`:
//! - A: reveal_secret / release_secret / perform_action (Safety Reaper + handle table)
//! - B: sync() over a mock Transport (metadata-first pull + selective payload)
//! - C: rotate_key() (crash-safe re-encryption + epoch advance)

use std::collections::HashMap;
use std::sync::Arc;

use sea_orm::{Database, DatabaseConnection, Set, TransactionTrait};
use uuid::Uuid;
use zeroize::Zeroizing;

use vautr_app_state::handles::{CoreAction, PlatformAdapter};
use vautr_app_state::VautrClient;
use vautr_crypto::{aead, key_tree};
use vautr_db::entity::{item_overview, item_payload};
use vautr_db::migrate;
use vautr_domain::DecryptedSecret;
use vautr_sync::engine::{PulledOverview, PushOutcome, Transport, TransportError};

use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

// --- Mock Transport --------------------------------------------------------

#[derive(Clone, Default)]
struct MockState {
    pull: Vec<(Uuid, u64, u64, bool)>, // (uuid, version, enc_key_gen, deleted)
    payloads: HashMap<Uuid, Vec<u8>>,
    min_gen: u64,
    rotated: Vec<(u64, Vec<u8>)>,
}

struct MockTransport {
    state: Arc<Mutex<MockState>>,
}

impl Transport for MockTransport {
    fn pull(
        &self,
        _cursor: u64,
    ) -> Pin<Box<dyn Future<Output = Result<(u64, Vec<PulledOverview>), TransportError>> + Send>>
    {
        let st = self.state.lock().unwrap().clone();
        Box::pin(async move {
            let overviews = st
                .pull
                .into_iter()
                .map(|(uuid, version, enc_key_gen, deleted)| PulledOverview {
                    uuid,
                    version,
                    enc_key_gen,
                    deleted,
                })
                .collect();
            Ok((100, overviews))
        })
    }

    fn fetch_payload(
        &self,
        uuid: &Uuid,
        _version: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, TransportError>> + Send>> {
        let st = self.state.lock().unwrap().clone();
        let uuid = *uuid;
        Box::pin(async move {
            st.payloads
                .get(&uuid)
                .cloned()
                .ok_or_else(|| TransportError::Other("no payload".into()))
        })
    }

    fn push_batch(
        &self,
        items: Vec<(Uuid, u64, u64, Option<Vec<u8>>)>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PushOutcome>, TransportError>> + Send>> {
        let _ = items;
        Box::pin(async move { Ok(vec![]) })
    }

    fn account_status(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(u64, Vec<u8>), TransportError>> + Send>> {
        let st = self.state.lock().unwrap().clone();
        Box::pin(async move { Ok((st.min_gen, Vec::new())) })
    }

    fn rotate_key(
        &self,
        new_min_gen: u64,
        new_svk_blob: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64, TransportError>> + Send>> {
        let st = self.state.clone();
        Box::pin(async move {
            st.lock().unwrap().rotated.push((new_min_gen, new_svk_blob));
            Ok(new_min_gen)
        })
    }
}

// --- Mock Platform Adapter ------------------------------------------------

#[derive(Default)]
struct MockAdapter {
    copied: Arc<Mutex<Option<String>>>,
}

impl PlatformAdapter for MockAdapter {
    fn service_action(&self, action: CoreAction, secret: &[u8]) -> Result<(), String> {
        match action {
            CoreAction::CopyToClipboard { .. } | CoreAction::Autofill { .. } => {
                *self.copied.lock().unwrap() = Some(String::from_utf8_lossy(secret).into_owned());
                Ok(())
            }
        }
    }
}

// --- Helpers ---------------------------------------------------------------

async fn fresh_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect in-memory");
    migrate::init(&db).await.expect("migrate");
    db
}

async fn insert_item(
    db: &DatabaseConnection,
    uuid: Uuid,
    gen: u64,
    payload: Vec<u8>,
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
    let payload_am = item_payload::ActiveModel {
        uuid: Set(uuid.to_string()),
        payload: Set(payload),
    };
    vautr_db::txn::save_item_txn(&txn, overview, payload_am, &uuid.to_string())
        .await
        .expect("save");
    txn.commit().await.expect("commit");
}

// --- Phase A: reveal / perform / release -----------------------------------

#[tokio::test]
async fn phase_a_reveal_perform_release() {
    let db = fresh_db().await;
    let client = VautrClient::new(db.clone());

    // Known vault key, derive DEK.
    let svk = key_tree::generate_svk();
    let dek = key_tree::derive_dek(&svk).expect("dek");

    let uuid = Uuid::new_v4();
    let secret = DecryptedSecret {
        password: Zeroizing::new("hunter2".into()),
        totp: None,
        notes: Zeroizing::new(String::new()),
        fields: vec![],
    };
    let pt = serde_json::to_vec(&secret).expect("serialize");
    let env = aead::encrypt(&dek, &uuid, 1, &pt).expect("encrypt");
    insert_item(&db, uuid, 1, env).await;

    client.unlock_with_raw_key(svk, 1).await;
    assert!(!client.is_locked());

    // Reveal -> opaque handle.
    let handle = client.reveal_secret(uuid).await.expect("reveal");
    assert!(handle != 0);

    // Perform a copy via the platform adapter.
    let adapter = Arc::new(MockAdapter::default());
    client.set_platform_adapter(adapter.clone()).await;
    client
        .perform_action(CoreAction::CopyToClipboard { handle })
        .await
        .expect("perform");
    assert_eq!(adapter.copied.lock().unwrap().as_deref(), Some("hunter2"));

    // Release zeroes the handle; a second perform fails (expired).
    client.release_secret(handle);
    let res = client.perform_action(CoreAction::CopyToClipboard { handle }).await;
    assert!(res.is_err(), "expired handle must be rejected");
}

#[tokio::test]
async fn phase_a_reveal_requires_unlock() {
    let db = fresh_db().await;
    let client = VautrClient::new(db);
    let uuid = Uuid::new_v4();
    let res = client.reveal_secret(uuid).await;
    assert!(res.is_err(), "reveal must fail while locked");
}

// --- Phase B: sync over mock transport ------------------------------------

#[tokio::test]
async fn phase_b_sync_pulls_and_persists() {
    let db = fresh_db().await;
    let client = VautrClient::new(db);

    let svk = key_tree::generate_svk();
    client.unlock_with_raw_key(svk, 1).await;

    let remote_uuid = Uuid::new_v4();
    let state = MockState {
        pull: vec![(remote_uuid, 5, 1, false)],
        payloads: {
            let mut m = HashMap::new();
            m.insert(remote_uuid, b"encrypted-blob".to_vec());
            m
        },
        min_gen: 1,
        rotated: vec![],
    };
    let transport = Arc::new(MockTransport {
        state: Arc::new(Mutex::new(state)),
    });
    client
        .connect_sync_with_transport(transport, remote_uuid)
        .await;

    client.sync().await.expect("sync");

    // The remote item should now be queryable locally.
    let overview = client.get_overview(remote_uuid).await;
    assert!(overview.is_ok(), "synced item must be persisted locally");
}

// --- Phase C: rotate_key re-encrypts and advances epoch -------------------

#[tokio::test]
async fn phase_c_rotate_key() {
    let db = fresh_db().await;
    let client = VautrClient::new(db.clone());

    let svk = key_tree::generate_svk();
    let dek = key_tree::derive_dek(&svk).expect("dek");

    // Seed one local item at gen 1.
    let uuid = Uuid::new_v4();
    let secret = DecryptedSecret {
        password: Zeroizing::new("rotate-me".into()),
        totp: None,
        notes: Zeroizing::new(String::new()),
        fields: vec![],
    };
    let pt = serde_json::to_vec(&secret).expect("serialize");
    let env = aead::encrypt(&dek, &uuid, 1, &pt).expect("encrypt");
    insert_item(&db, uuid, 1, env).await;

    client.unlock_with_raw_key(svk, 1).await;

    let state = MockState {
        pull: vec![],
        payloads: HashMap::new(),
        min_gen: 1,
        rotated: vec![],
    };
    let transport = Arc::new(MockTransport {
        state: Arc::new(Mutex::new(state)),
    });
    client
        .connect_sync_with_transport(transport.clone(), uuid)
        .await;

    // Rotate to gen 2.
    client.rotate_key(2).await.expect("rotate");

    // The transport must have received the epoch advance.
    let rotated = &transport.state.lock().unwrap().rotated;
    assert_eq!(rotated.len(), 1);
    assert_eq!(rotated[0].0, 2);

    // The item is now re-encrypted at gen 2 and remains decryptable.
    let material = vautr_db::query::get_secret_material(&db, &uuid.to_string())
        .await
        .expect("query")
        .expect("present");
    assert_eq!(material.0, 2, "enc_key_gen must advance to 2");
    let new_dek = key_tree::derive_dek(&client.current_svk().await.unwrap()).unwrap();
    let plain = aead::decrypt(&new_dek, &uuid, 2, &material.1).expect("decrypt");
    let secret2: DecryptedSecret = serde_json::from_slice(&plain).expect("parse");
    assert_eq!(&*secret2.password, "rotate-me");

    // Local epoch gate updated.
    assert_eq!(client.current_key_gen(), 2);
}
