//! Wave C E2E integration tests for `VautrClient`.
//!
//! Exercises the wiring of the standalone feature crates into the orchestrator:
//! - import → seed (data-import-seeding.md)
//! - A-push → B-pull over an in-memory relay transport (save→sync→push path)
//! - 1:1 sharing round-trip over `InMemoryShareRelay` (sharing-pki.md §5)
//! - attachment upload/download over an in-memory file store (file-storage.md)
//! - recovery key onboarding gate + RK unlock → forced MP+RK rotation
//!
//! Everything runs against in-memory SeaORM SQLite databases and in-memory
//! transports; no network or real server is required.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use sea_orm::{Database, DatabaseConnection};
use uuid::Uuid;
use zeroize::Zeroizing;

use vautr_app_state::event_bus::VaultStateUpdate;
use vautr_app_state::file_transfer::FileTransport;
use vautr_app_state::handles::{CoreAction, PlatformAdapter};
use vautr_app_state::sharing::{generate_vault_sharing_keypair, InMemoryShareRelay};
use vautr_app_state::VautrClient;
use vautr_crypto::{aead, key_tree};
use vautr_db::migrate;
use vautr_domain::{DecryptedOverview, DecryptedSecret, DomainModel, ItemMetadata};
use vautr_import::RawImportItem;
use vautr_sync::engine::{PulledOverview, PushOutcome, Transport, TransportError};

// --- Shared relay transport (A push → B pull) ------------------------------

/// A server-like relay shared by two clients: `push_batch` persists, `pull`
/// returns every live item so the peer can download it.
#[derive(Default)]
struct ServerState {
    items: HashMap<Uuid, (u64, u64, Vec<u8>)>, // uuid -> (version, enc_key_gen, payload)
    min_gen: u64,
    svk_blob: Vec<u8>,
}

#[derive(Clone)]
struct RoundTripTransport {
    state: Arc<Mutex<ServerState>>,
}

impl Transport for RoundTripTransport {
    fn pull(
        &self,
        _cursor: u64,
    ) -> Pin<Box<dyn Future<Output = Result<(u64, Vec<PulledOverview>), TransportError>> + Send>>
    {
        let st = self.state.clone();
        Box::pin(async move {
            let g = st.lock().unwrap();
            let overviews = g
                .items
                .iter()
                .map(|(uuid, (version, gen, _))| PulledOverview {
                    uuid: *uuid,
                    version: *version,
                    enc_key_gen: *gen,
                    deleted: false,
                })
                .collect();
            Ok((g.min_gen, overviews))
        })
    }

    fn fetch_payload(
        &self,
        uuid: &Uuid,
        _version: u64,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, TransportError>> + Send>> {
        let st = self.state.clone();
        let uuid = *uuid;
        Box::pin(async move {
            st.lock()
                .unwrap()
                .items
                .get(&uuid)
                .map(|(_, _, p)| p.clone())
                .ok_or_else(|| TransportError::Other("no payload".into()))
        })
    }

    fn push_batch(
        &self,
        items: Vec<(Uuid, u64, u64, Option<Vec<u8>>)>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PushOutcome>, TransportError>> + Send>> {
        let st = self.state.clone();
        Box::pin(async move {
            let mut g = st.lock().unwrap();
            let mut outcomes = Vec::with_capacity(items.len());
            for (uuid, version, gen, payload) in items {
                if let Some(p) = payload {
                    g.items.insert(uuid, (version, gen, p));
                }
                outcomes.push(PushOutcome::Applied);
            }
            Ok(outcomes)
        })
    }

    fn account_status(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<(u64, Vec<u8>), TransportError>> + Send>> {
        let st = self.state.clone();
        Box::pin(async move {
            let g = st.lock().unwrap();
            Ok((g.min_gen, g.svk_blob.clone()))
        })
    }

    fn rotate_key(
        &self,
        new_min_gen: u64,
        new_svk_blob: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64, TransportError>> + Send>> {
        let st = self.state.clone();
        Box::pin(async move {
            let mut g = st.lock().unwrap();
            g.min_gen = new_min_gen;
            g.svk_blob = new_svk_blob;
            Ok(new_min_gen)
        })
    }
}

// --- In-memory file store ---------------------------------------------------

#[derive(Default)]
struct FileStoreState {
    chunks: HashMap<(Uuid, u32), Vec<u8>>,
    sessions: HashSet<Uuid>,
    completed: HashSet<Uuid>,
}

#[derive(Clone, Default)]
struct InMemoryFileStore {
    state: Arc<Mutex<FileStoreState>>,
}

impl FileTransport for InMemoryFileStore {
    fn initiate_upload(
        &self,
        manifest: &vautr_files::manifest::FileManifest,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let state = self.state.clone();
        let id = manifest.file_uuid;
        Box::pin(async move {
            state.lock().unwrap().sessions.insert(id);
            Ok(())
        })
    }

    fn upload_chunk(
        &self,
        file_uuid: Uuid,
        index: u32,
        ciphertext: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move {
            state.lock().unwrap().chunks.insert((file_uuid, index), ciphertext);
            Ok(())
        })
    }

    fn complete_upload(
        &self,
        file_uuid: Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move {
            state.lock().unwrap().completed.insert(file_uuid);
            Ok(())
        })
    }

    fn fetch_chunk(
        &self,
        file_uuid: Uuid,
        index: u32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send>> {
        let state = self.state.clone();
        Box::pin(async move {
            state
                .lock()
                .unwrap()
                .chunks
                .get(&(file_uuid, index))
                .cloned()
                .ok_or_else(|| format!("missing chunk {file_uuid}/{index}"))
        })
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

fn sample_item(uuid: Uuid, password: &str) -> DomainModel {
    DomainModel {
        uuid,
        enc_key_gen: 1,
        overview: DecryptedOverview {
            uuid,
            title: "Acme".into(),
            subtitle: "login".into(),
            icon_key: "web".into(),
            urls: vec![],
            updated_at: 0,
        },
        secret: DecryptedSecret {
            password: Zeroizing::new(password.to_string()),
            totp: None,
            notes: Zeroizing::new(String::new()),
            fields: vec![],
        },
        metadata: ItemMetadata {
            created_at: 0,
            updated_at: 0,
            trashed: false,
        },
    }
}

/// Captures whatever the client asks the platform adapter to copy.
#[derive(Default)]
struct MockAdapter {
    copied: Arc<Mutex<Option<String>>>,
}

impl PlatformAdapter for MockAdapter {
    fn service_action(&self, action: CoreAction, secret: &[u8]) -> Result<(), String> {
        match action {
            CoreAction::CopyToClipboard { .. } | CoreAction::Autofill { .. } => {
                *self.copied.lock().unwrap() =
                    Some(String::from_utf8_lossy(secret).into_owned());
                Ok(())
            }
        }
    }
}

// --- E2E: A saves → pushes → B pulls and decrypts --------------------------

#[tokio::test]
async fn e2e_push_pull_roundtrip() {
    let shared = Arc::new(Mutex::new(ServerState::default()));
    let a = VautrClient::new(fresh_db().await);
    let b = VautrClient::new(fresh_db().await);
    let svk = key_tree::generate_svk();

    a.connect_sync_with_transport(
        Arc::new(RoundTripTransport { state: shared.clone() }),
        Uuid::new_v4(),
    )
    .await;
    b.connect_sync_with_transport(
        Arc::new(RoundTripTransport { state: shared.clone() }),
        Uuid::new_v4(),
    )
    .await;
    a.unlock_with_raw_key(svk.clone(), 1).await;
    b.unlock_with_raw_key(svk.clone(), 1).await;

    // A saves a new item (save → sync → push path).
    let uuid = Uuid::new_v4();
    let item = sample_item(uuid, "roundtrip-secret");
    let dek = key_tree::derive_dek(&svk).unwrap();
    let pt = serde_json::to_vec(&item.secret).unwrap();
    let env = aead::encrypt(&dek, &uuid, 1, &pt).unwrap();
    a.save_item(item, env).await;

    // A pushes all local changes to the relay.
    a.push_local_changes().await.expect("A push");
    assert_eq!(shared.lock().unwrap().items.len(), 1, "server has A's item");

    // B pulls and must be able to reveal A's secret.
    b.sync().await.expect("B sync");
    b.get_overview(uuid).await.expect("B overview persisted");
    let adapter = Arc::new(MockAdapter::default());
    b.set_platform_adapter(adapter.clone()).await;
    let handle = b.reveal_secret(uuid).await.expect("B reveals secret");
    b.perform_action(CoreAction::CopyToClipboard { handle })
        .await
        .expect("B copies");
    assert_eq!(
        adapter.copied.lock().unwrap().as_deref(),
        Some("roundtrip-secret"),
        "B recovers the exact secret A saved"
    );
}

// --- E2E: import → seed (data-import-seeding.md §4) ------------------------

#[tokio::test]
async fn e2e_import_then_seed() {
    let shared = Arc::new(Mutex::new(ServerState::default()));
    let client = VautrClient::new(fresh_db().await);
    client
        .connect_sync_with_transport(
            Arc::new(RoundTripTransport { state: shared.clone() }),
            Uuid::new_v4(),
        )
        .await;
    let svk = key_tree::generate_svk();
    client.unlock_with_raw_key(svk, 1).await;

    let mut rx = client.watch_state();
    let items = vec![RawImportItem {
        source_id: Some("bitwarden".into()),
        title: "Imported Login".into(),
        url: Some("https://import.example.com".into()),
        fields: serde_json::json!({ "username": "u1", "password": "pw1" }),
    }];

    let report = client.import_items(items, |_| {}).await.expect("import");
    assert_eq!(report.success_count, 1);
    assert!(report.errors.is_empty());

    // A single ImportCompleted event is emitted (plus nothing else during seed).
    assert!(
        matches!(rx.recv().await, Ok(VaultStateUpdate::ImportCompleted(_))),
        "expected a single ImportCompleted event"
    );

    // The imported item was seeded to the server.
    assert_eq!(shared.lock().unwrap().items.len(), 1);
}

// --- E2E: 1:1 share → accept round-trip (sharing-pki.md §5) ----------------

#[tokio::test]
async fn e2e_share_accept_roundtrip() {
    let relay = InMemoryShareRelay::default();
    let share_t: std::sync::Arc<dyn vautr_app_state::sharing::ShareTransport> =
        Arc::new(relay.clone());

    let a_user = Uuid::new_v4();
    let b_user = Uuid::new_v4();
    let a = VautrClient::new(fresh_db().await);
    let b = VautrClient::new(fresh_db().await);
    a.connect_sharing(share_t.clone()).await;
    b.connect_sharing(share_t.clone()).await;
    a.set_server_user_id(a_user).await;
    b.set_server_user_id(b_user).await;

    let kp_a = generate_vault_sharing_keypair();
    let kp_b = generate_vault_sharing_keypair();
    a.set_sharing_keypair(kp_a).await;
    b.set_sharing_keypair(kp_b.clone()).await;
    // B publishes its public key; A will look it up.
    relay.publish_public_key(b_user, kp_b.public);

    let item_uuid = Uuid::new_v4();
    let plaintext = b"shared-plaintext-secret";
    let bundle = a
        .share_item(b_user, item_uuid, plaintext)
        .await
        .expect("A shares to B");

    let inbox = b.fetch_shares().await.expect("B fetch inbox");
    assert!(
        inbox.iter().any(|s| s.share_id == bundle.share_id),
        "B must see the share in its inbox"
    );

    let incoming = inbox.into_iter().find(|s| s.share_id == bundle.share_id).unwrap();
    let decrypted = b.accept_share(&incoming).await.expect("B accepts");
    assert_eq!(decrypted, plaintext, "B recovers the shared plaintext");

    // Revoking removes it from the inbox.
    b.revoke_share(bundle.share_id).await.expect("revoke");
    let after = b.fetch_shares().await.expect("fetch again");
    assert!(!after.iter().any(|s| s.share_id == bundle.share_id));
}

// --- E2E: attachment upload → download (file-storage.md) -------------------

#[tokio::test]
async fn e2e_file_upload_download() {
    let store = Arc::new(InMemoryFileStore::default());
    let client = VautrClient::new(fresh_db().await);
    client.connect_files(store.clone()).await;

    let svk = key_tree::generate_svk();
    client.unlock_with_raw_key(svk.clone(), 1).await;

    let payload = b"the quick brown fox jumps over the lazy dog".repeat(200); // >1 chunk
    let manifest = client
        .upload_file(&payload, "text/plain", 1234)
        .await
        .expect("upload");
    assert_eq!(manifest.total_size, payload.len() as u64);
    assert!(store.state.lock().unwrap().completed.contains(&manifest.file_uuid));

    let roundtrip = client.download_file(&manifest).await.expect("download");
    assert_eq!(roundtrip, payload, "downloaded bytes match upload");
}

// --- E2E: recovery key gate + RK unlock → forced rotation ------------------

#[tokio::test]
async fn e2e_recovery_rotate() {
    // Set up a vault, generate an RK and wrap the SVK for it.
    let svk = key_tree::generate_svk();
    let setup = VautrClient::new(fresh_db().await);
    setup.unlock_with_raw_key(svk.clone(), 1).await;
    let rk = setup.generate_recovery_key();
    assert_eq!(rk.split_whitespace().count(), 24, "24-word recovery key");

    // Onboarding proof-of-possession gate (§3.2): words 4, 12, 20.
    let words: Vec<&str> = rk.split_whitespace().collect();
    assert!(setup.verify_recovery_key_possession(
        &rk,
        &[words[3], words[11], words[19]]
    ));
    assert!(!setup.verify_recovery_key_possession(
        &rk,
        &["incorrect", "incorrect", "incorrect"]
    ));

    // Wrap the SVK under the RK (what onboarding would persist server-side).
    let mnemonic = vautr_crypto::recovery::decode_recovery_mnemonic(&rk).unwrap();
    let kek_rk = vautr_crypto::recovery::derive_kek_rk(&mnemonic).unwrap();
    let user_id = Uuid::new_v4();
    let wrapped = vautr_crypto::recovery::wrap_svk_with_rk(&svk, &kek_rk, &user_id).unwrap();

    // A fresh device recovers via the RK.
    let recovered = VautrClient::new(fresh_db().await);
    recovered
        .recover_with_key(&rk, &wrapped, user_id)
        .await
        .expect("recover via RK");
    assert!(!recovered.is_locked());
    assert!(recovered.recovery_pending(), "forced rotation is pending");

    // RK proof-of-possession signature for the server challenge.
    let sig = recovered.recovery_sign_challenge(b"server-nonce").await.expect("sign");
    assert!(!sig.is_empty());

    // Complete the forced MP+RK rotation; gate clears and a new RK is issued.
    let new_rk = recovered
        .complete_recovery(Zeroizing::new("new-master-password".to_string()), &[0u8; 32])
        .await
        .expect("complete recovery");
    assert!(!new_rk.is_empty());
    assert!(!recovered.recovery_pending());
}

// --- E2E: offline queue buffers then flushes -------------------------------

#[tokio::test]
async fn e2e_offline_queue_flush() {
    let shared = Arc::new(Mutex::new(ServerState::default()));
    let client = VautrClient::new(fresh_db().await);
    client
        .connect_sync_with_transport(
            Arc::new(RoundTripTransport { state: shared.clone() }),
            Uuid::new_v4(),
        )
        .await;
    let svk = key_tree::generate_svk();
    client.unlock_with_raw_key(svk.clone(), 1).await;

    let uuid = Uuid::new_v4();
    let item = sample_item(uuid, "offline-secret");
    let dek = key_tree::derive_dek(&svk).unwrap();
    let pt = serde_json::to_vec(&item.secret).unwrap();
    let env = aead::encrypt(&dek, &uuid, 1, &pt).unwrap();

    // While "offline" (no flush), queue a mutation.
    let n = client.offline_save(item, env).await;
    assert_eq!(n, 1);
    assert_eq!(client.offline_queue_len(), 1);
    assert!(shared.lock().unwrap().items.is_empty(), "nothing pushed yet");

    // Flush applies + pushes.
    let flushed = client.flush_offline_queue().await.expect("flush");
    assert_eq!(flushed, 1);
    assert_eq!(client.offline_queue_len(), 0);
    assert_eq!(shared.lock().unwrap().items.len(), 1);
}
