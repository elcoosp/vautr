//! UniFFI `VautrClient` surface for mobile (Swift/Kotlin). data.md, ADR-003.
//!
//! Secrets cross the bridge ONLY as opaque `u64` handles. `perform_action`
//! delegates the copy/autofill to a native `PlatformActionHandler` (registered
//! by the Turbo Module) so the secret string never enters the JS/Kotlin heap.
//! `read_secret` is desktop-gated (data.md §1 rule 4) and is compiled out of
//! the mobile artifact — asserted by `scripts/check-restricted-api.sh`.
//!
//! ## Mobile boot surface (build-env-deploy.md §3.1)
//! `initialize` links the core into the app (runs migrations on a fresh vault
//! DB); `unlock` recovers the 32-byte SVK from the OS keystore (biometrics) or
//! `unlock_with_password` re-derives it from the master password; `list_overviews`
//! / `reveal_secret` / `lock` / `sync` drive the Lock/List/Detail screens.
//! `SecureEnclaveBridge` is the native Keychain/Android-Keystore adapter that
//! persists the SVK under biometric protection so unlock never re-prompts for a
//! master password. `read_secret` remains desktop-only (build-env-deploy.md §2.5
//! keeps `read_secret` out of mobile via the `desktop-api` gate).

use std::sync::{Arc, RwLock};

use base64::Engine;
use uniffi::{Enum, Object};
use uuid::Uuid;
use zeroize::Zeroizing;

use vautr_app_state::handles::{CoreAction as CoreCoreAction, PlatformAdapter};
use vautr_app_state::worker::TaskOutcome;
use vautr_app_state::VautrClient;

/// Opaque handle to a decrypted secret (u64). ADR-003.
pub type SecretHandle = u64;

/// FFI-safe action enum mirroring `vautr_app_state::handles::CoreAction`.
#[derive(Clone, Debug, Enum)]
pub enum CoreAction {
    CopyToClipboard {
        handle: SecretHandle,
    },
    Autofill {
        handle: SecretHandle,
    },
    /// Render the secret in the native overlay view. The plaintext is delivered
    /// only to the native `PlatformActionHandler` (never JS); the overlay calls
    /// `release_secret` on unmount (VTR-048, ADR-003).
    RenderInOverlay {
        handle: SecretHandle,
    },
}

impl From<CoreAction> for CoreCoreAction {
    fn from(a: CoreAction) -> Self {
        match a {
            CoreAction::CopyToClipboard { handle } => CoreCoreAction::CopyToClipboard { handle },
            CoreAction::Autofill { handle } => CoreCoreAction::Autofill { handle },
            CoreAction::RenderInOverlay { handle } => CoreCoreAction::RenderInOverlay { handle },
        }
    }
}

impl From<CoreCoreAction> for CoreAction {
    fn from(a: CoreCoreAction) -> Self {
        match a {
            CoreCoreAction::CopyToClipboard { handle } => CoreAction::CopyToClipboard { handle },
            CoreCoreAction::Autofill { handle } => CoreAction::Autofill { handle },
            CoreCoreAction::RenderInOverlay { handle } => CoreAction::RenderInOverlay { handle },
        }
    }
}

/// Error surfaced to the mobile layer.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    #[error("core error: {0}")]
    Core(String),
    #[error("invalid uuid: {0}")]
    Uuid(String),
    #[error("mutation rejected (read-only gate or conflict)")]
    Rejected,
}

/// Native platform handler. The Turbo Module implements this and registers it
/// so the Core can invoke the OS clipboard / autofill natively (the secret
/// string never crosses into JS).
#[uniffi::export(with_foreign)]
pub trait PlatformActionHandler: Send + Sync {
    /// `secret` is the plaintext; the implementer writes it to the OS clipboard
    /// (or autofills) and must zeroize it immediately after.
    fn on_action(&self, action: CoreAction, secret: String);
}

/// Bridges the UniFFI foreign handler to the core `PlatformAdapter` trait.
struct MobilePlatformAdapter {
    handler: Arc<dyn PlatformActionHandler>,
}

impl PlatformAdapter for MobilePlatformAdapter {
    fn service_action(&self, action: CoreCoreAction, secret: &[u8]) {
        let secret = String::from_utf8_lossy(secret).into_owned();
        self.handler.on_action(action.into(), secret);
    }
}

/// OS-keystore biometrics storage of the 32-byte SVK (build-env-deploy §2.5,
/// crypto.md §6). Implemented natively (Keychain on iOS, Android Keystore via
/// the Turbo Module) and registered with `MobileClient::set_secure_enclave_bridge`
/// so a biometric unlock can recover the SVK without re-prompting for a master
/// password. The SVK bytes are handed back as `Vec<u8>`; they are never logged
/// and the implementer must zeroize any copy after unlock.
#[uniffi::export(with_foreign)]
pub trait SecureEnclaveBridge: Send + Sync {
    /// Persist the 32-byte SVK under biometric (or device-passcode) protection.
    fn save_svk(&self, svk: Vec<u8>) -> Result<(), FfiError>;

    /// Load the stored SVK. Returns `None` if absent or if biometric auth was
    /// cancelled/denied by the user.
    fn load_svk(&self) -> Result<Option<Vec<u8>>, FfiError>;

    /// Delete the stored SVK (e.g. on explicit lock or vault removal).
    fn delete_svk(&self) -> Result<(), FfiError>;

    /// Whether an SVK is currently stored and available for a biometric unlock.
    fn has_svk(&self) -> Result<bool, FfiError>;
}

/// Map a core `TaskOutcome` to an FFI result.
fn map_outcome(o: TaskOutcome) -> Result<(), FfiError> {
    match o {
        TaskOutcome::Committed(_) => Ok(()),
        TaskOutcome::Rejected { .. } => Err(FfiError::Rejected),
    }
}

/// Mobile-facing client. Wraps `vautr_app_state::VautrClient`.
#[derive(Object)]
pub struct MobileClient {
    inner: Arc<VautrClient>,
    /// Native Keychain/Keystore SVK adapter (biometric unlock). Registered via
    /// `set_secure_enclave_bridge`; read by the app to recover the SVK.
    enclave: RwLock<Option<Arc<dyn SecureEnclaveBridge>>>,
    /// Persisted sharing secret key (base64), used by the sharing PKI surface.
    sharing_secret: RwLock<Option<String>>,
}

#[uniffi::export(async_runtime = "tokio")]
impl MobileClient {
    /// Open (or create) the vault at `db_path` (sqlite file). The vault starts
    /// locked; call an unlock method before accessing secrets.
    #[uniffi::constructor]
    pub fn new(db_path: String) -> Result<Arc<Self>, FfiError> {
        let url = if db_path.starts_with("sqlite://") {
            db_path
        } else {
            format!("sqlite://{db_path}?mode=rwc")
        };
        let db = tokio::runtime::Handle::current()
            .block_on(sea_orm::Database::connect(&url))
            .map_err(|e| FfiError::Core(format!("db connect: {e}")))?;
        let client = VautrClient::new(db);
        Ok(Arc::new(Self {
            inner: Arc::new(client),
            enclave: RwLock::new(None),
            sharing_secret: RwLock::new(None),
        }))
    }

    /// Link the core into the app (build-env-deploy §3.1 / skill matrix boot
    /// pattern). Opens the vault DB and runs schema migrations so a fresh vault
    /// is usable on first launch. The vault starts locked; call `unlock` (or
    /// `unlock_with_password`) before accessing secrets.
    #[uniffi::constructor]
    pub async fn initialize(db_path: String) -> Result<Arc<Self>, FfiError> {
        let url = if db_path.starts_with("sqlite://") {
            db_path
        } else {
            format!("sqlite://{db_path}?mode=rwc")
        };
        let db = sea_orm::Database::connect(&url)
            .await
            .map_err(|e| FfiError::Core(format!("db connect: {e}")))?;
        vautr_db::migrate::init(&db)
            .await
            .map_err(|e| FfiError::Core(format!("migrate: {e}")))?;
        let client = VautrClient::new(db);
        Ok(Arc::new(Self {
            inner: Arc::new(client),
            enclave: RwLock::new(None),
            sharing_secret: RwLock::new(None),
        }))
    }

    /// Register the native OS-keystore SVK bridge (biometric unlock).
    pub fn set_secure_enclave_bridge(&self, bridge: Arc<dyn SecureEnclaveBridge>) {
        *self.enclave.write().unwrap() = Some(bridge);
    }

    /// The currently-registered Secure Enclave bridge, if any.
    pub fn secure_enclave_bridge(&self) -> Option<Arc<dyn SecureEnclaveBridge>> {
        self.enclave.read().unwrap().clone()
    }

    /// Register the native platform handler (clipboard / autofill).
    pub async fn set_platform_handler(&self, handler: Arc<dyn PlatformActionHandler>) {
        let adapter: Arc<dyn PlatformAdapter> = Arc::new(MobilePlatformAdapter { handler });
        self.inner.set_platform_adapter(adapter).await;
    }

    /// Unlock with a raw 32-byte SVK recovered from the OS keystore (biometric
    /// unlock, crypto.md §6). `local_gen` is the local vault key generation.
    pub async fn unlock(&self, raw_key: Vec<u8>, local_gen: u64) -> Result<(), FfiError> {
        if raw_key.len() != 32 {
            return Err(FfiError::Core("raw key must be 32 bytes".into()));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&raw_key);
        self.inner
            .unlock_with_raw_key(Zeroizing::new(key), local_gen)
            .await;
        Ok(())
    }

    /// List all overviews (most-recently-used first) as a JSON array of
    /// `DecryptedOverview`. Mirrors the web worker's empty-query search.
    pub async fn list_overviews(&self) -> Result<String, FfiError> {
        let items = self.inner.search("").await.map_err(FfiError::Core)?;
        serde_json::to_string(&items).map_err(|e| FfiError::Core(format!("serialize: {e}")))
    }

    /// Unlock with the master password. `kdf_salt_b64` / `wrapped_svk_b64` come
    /// from the server auth bootstrap; `user_id` is the server user uuid.
    pub async fn unlock_with_password(
        &self,
        mp: String,
        kdf_salt_b64: String,
        wrapped_svk_b64: String,
        user_id: String,
        local_gen: u64,
    ) -> Result<(), FfiError> {
        let kdf_salt = base64::engine::general_purpose::STANDARD
            .decode(&kdf_salt_b64)
            .map_err(|e| FfiError::Core(format!("kdf_salt b64: {e}")))?;
        let wrapped_svk = base64::engine::general_purpose::STANDARD
            .decode(&wrapped_svk_b64)
            .map_err(|e| FfiError::Core(format!("wrapped_svk b64: {e}")))?;
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&kdf_salt);
        let uid = Uuid::parse_str(&user_id).map_err(|e| FfiError::Uuid(e.to_string()))?;
        self.inner
            .unlock_with_password(Zeroizing::new(mp), &salt, &wrapped_svk, uid, local_gen)
            .await
            .map_err(FfiError::Core)
    }

    /// Unlock directly with a raw 32-byte vault key + the local key generation.
    pub async fn unlock_with_raw_key(
        &self,
        raw_key: Vec<u8>,
        local_gen: u64,
    ) -> Result<(), FfiError> {
        if raw_key.len() != 32 {
            return Err(FfiError::Core("raw key must be 32 bytes".into()));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&raw_key);
        self.inner
            .unlock_with_raw_key(Zeroizing::new(key), local_gen)
            .await;
        Ok(())
    }

    /// Lock the vault (zeroizes keys + in-memory secrets).
    pub async fn lock(&self) {
        self.inner.lock().await;
    }

    /// Whether the vault is currently locked.
    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }

    /// FTS5 search. Returns JSON-encoded `Vec<DecryptedOverview>`.
    pub async fn search(&self, query: String) -> Result<String, FfiError> {
        let items = self.inner.search(&query).await.map_err(FfiError::Core)?;
        serde_json::to_string(&items).map_err(|e| FfiError::Core(format!("serialize: {e}")))
    }

    /// Get a single overview by uuid string. Returns JSON `DecryptedOverview`.
    pub async fn get_overview(&self, uuid: String) -> Result<String, FfiError> {
        let uuid = Uuid::parse_str(&uuid).map_err(|e| FfiError::Uuid(e.to_string()))?;
        let ov = self
            .inner
            .get_overview(uuid)
            .await
            .map_err(FfiError::Core)?;
        serde_json::to_string(&ov).map_err(|e| FfiError::Core(format!("serialize: {e}")))
    }

    /// Reveal a secret, returning an opaque handle (secret stays in Rust).
    pub async fn reveal_secret(&self, uuid: String) -> Result<SecretHandle, FfiError> {
        let uuid = Uuid::parse_str(&uuid).map_err(|e| FfiError::Uuid(e.to_string()))?;
        self.inner.reveal_secret(uuid).await.map_err(FfiError::Core)
    }

    /// Delegate copy/autofill to the native platform handler.
    pub async fn perform_action(&self, action: CoreAction) -> Result<(), FfiError> {
        self.inner
            .perform_action(action.into())
            .await
            .map_err(FfiError::Core)
    }

    /// Explicitly release a handle (zeroizes the in-memory secret).
    pub fn release_secret(&self, handle: SecretHandle) -> Result<(), FfiError> {
        self.inner.release_secret(handle);
        Ok(())
    }

    /// Render a revealed secret in the native overlay view (VTR-048, ADR-003).
    ///
    /// Delegates to `PlatformActionHandler::on_action(RenderInOverlay, secret)`
    /// so the plaintext is delivered **only to native code** (the overlay view)
    /// and never crosses into the JS heap. The overlay component is responsible
    /// for calling `release_secret` when it unmounts.
    pub async fn render_secret_in_overlay(&self, handle: SecretHandle) -> Result<(), FfiError> {
        self.inner
            .perform_action(CoreCoreAction::RenderInOverlay { handle })
            .await
            .map_err(FfiError::Core)
    }

    /// Save an item. `payload` is the pre-encrypted ciphertext blob; `enc_key_gen`
    /// is the vault key generation that encrypted it.
    pub async fn save_item(
        &self,
        uuid: String,
        enc_key_gen: u64,
        payload: Vec<u8>,
    ) -> Result<(), FfiError> {
        let uuid = Uuid::parse_str(&uuid).map_err(|e| FfiError::Uuid(e.to_string()))?;
        let item = vautr_domain::DomainModel {
            uuid,
            enc_key_gen,
            overview: vautr_domain::DecryptedOverview {
                uuid,
                title: String::new(),
                subtitle: String::new(),
                icon_key: String::new(),
                urls: vec![],
                updated_at: 0,
            },
            secret: vautr_domain::DecryptedSecret {
                password: Zeroizing::new(String::new()),
                totp: None,
                notes: Zeroizing::new(String::new()),
                fields: vec![],
            },
            metadata: vautr_domain::ItemMetadata {
                created_at: 0,
                updated_at: 0,
                trashed: false,
            },
        };
        map_outcome(self.inner.save_item(item, payload).await)
    }

    /// Delete an item by uuid.
    pub async fn delete_item(&self, uuid: String) -> Result<(), FfiError> {
        let uuid = Uuid::parse_str(&uuid).map_err(|e| FfiError::Uuid(e.to_string()))?;
        map_outcome(self.inner.delete_item(uuid).await)
    }

    /// Connect sync transport to the server.
    pub async fn connect_sync(
        &self,
        base_url: String,
        token: String,
        user_id: String,
    ) -> Result<(), FfiError> {
        let user_id = Uuid::parse_str(&user_id).map_err(|e| FfiError::Uuid(e.to_string()))?;
        self.inner.connect_sync(&base_url, &token, user_id).await;
        Ok(())
    }

    /// Run a metadata-first sync pull + selective payload download.
    pub async fn sync(&self) -> Result<(), FfiError> {
        self.inner.sync().await.map_err(FfiError::Core)
    }

    /// Rotate the vault key to `new_gen`.
    pub async fn rotate_key(&self, new_gen: u64) -> Result<(), FfiError> {
        self.inner.rotate_key(new_gen).await.map_err(FfiError::Core)
    }

    // ── Sharing PKI (ADR-007 / sharing-pki.md §6) ───────────────────────
    // These delegate to the uniffi sharing surface in `crate::sharing`. Plaintext
    // secret bytes are produced only inside Rust and handed back to the native
    // caller; they never sit in the JS heap (zero-knowledge invariant).

    /// Ensure a sharing keypair exists; generates + persists one on first use,
    /// returning the public key (base64) for publishing to the server PKI.
    pub fn ensure_sharing_key(&self) -> Result<String, FfiError> {
        let mut guard = self.sharing_secret.write().unwrap();
        if let Some(existing) = guard.as_ref() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(existing)
                .map_err(|e| FfiError::Core(format!("stored sharing secret: {e}")))?;
            if bytes.len() != 32 {
                return Err(FfiError::Core("stored sharing secret not 32 bytes".into()));
            }
            let mut sk = [0u8; 32];
            sk.copy_from_slice(&bytes);
            return Ok(base64::engine::general_purpose::STANDARD
                .encode(vautr_crypto::sharing::SharingKeyPair::from_secret(sk).public));
        }
        let kp = vautr_crypto::sharing::SharingKeyPair::generate();
        let secret_b64 = base64::engine::general_purpose::STANDARD.encode(kp.secret_bytes());
        let public_b64 = base64::engine::general_purpose::STANDARD.encode(kp.public);
        *guard = Some(secret_b64);
        Ok(public_b64)
    }

    /// Persist the sharing secret key (base64) loaded from the OS secure store.
    pub fn set_sharing_secret(&self, secret_b64: Option<String>) -> Result<(), FfiError> {
        if let Some(s) = secret_b64.as_ref() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|e| FfiError::Core(format!("sharing secret b64: {e}")))?;
            if bytes.len() != 32 {
                return Err(FfiError::Core("sharing secret must be 32 bytes".into()));
            }
        }
        *self.sharing_secret.write().unwrap() = secret_b64;
        Ok(())
    }

    /// The persisted sharing secret key (base64), if any.
    pub fn sharing_secret(&self) -> Option<String> {
        self.sharing_secret.read().unwrap().clone()
    }

    /// Build a 1:1 share bundle for `recipient_pubkey_b64`.
    pub fn share_item(
        &self,
        sender_uuid: String,
        recipient_uuid: String,
        item_uuid: String,
        recipient_pubkey_b64: String,
        plaintext: Vec<u8>,
    ) -> Result<crate::sharing::FfiShareBundle, FfiError> {
        crate::sharing::ffi_share_item(
            sender_uuid,
            recipient_uuid,
            item_uuid,
            recipient_pubkey_b64,
            plaintext,
        )
    }

    /// Decrypt an incoming 1:1 share using the persisted sharing secret.
    pub fn accept_share(&self, incoming_json: String) -> Result<Vec<u8>, FfiError> {
        let secret =
            self.sharing_secret.read().unwrap().clone().ok_or_else(|| {
                FfiError::Core("no sharing secret; call ensure_sharing_key".into())
            })?;
        crate::sharing::ffi_accept_share(incoming_json, secret)
    }

    /// Create a sharing group (admin). Returns the admin's `{ group, secret }`.
    pub fn create_group(
        &self,
        name: String,
        admin_uuid: String,
    ) -> Result<crate::sharing::FfiGroupKey, FfiError> {
        crate::sharing::ffi_create_group(name, admin_uuid)
    }

    /// Wrap the Group SIK for a new member.
    pub fn add_group_member(
        &self,
        group_json: String,
        member_uuid: String,
        member_pubkey_b64: String,
    ) -> Result<crate::sharing::FfiWrappedGroupKey, FfiError> {
        crate::sharing::ffi_add_group_member(group_json, member_uuid, member_pubkey_b64)
    }

    /// Member-side: decapsulate the Group SIK from an inbox entry.
    pub fn unwrap_group_key(&self, inbox_json: String) -> Result<String, FfiError> {
        let secret =
            self.sharing_secret.read().unwrap().clone().ok_or_else(|| {
                FfiError::Core("no sharing secret; call ensure_sharing_key".into())
            })?;
        crate::sharing::ffi_unwrap_group_key(inbox_json, secret)
    }

    /// Encrypt a vault item's payload for a group.
    pub fn encrypt_group_item(
        &self,
        group_json: String,
        item_uuid: String,
        plaintext: Vec<u8>,
    ) -> Result<String, FfiError> {
        crate::sharing::ffi_encrypt_group_item(group_json, item_uuid, plaintext)
    }

    /// Decrypt a group item's payload.
    pub fn decrypt_group_item(
        &self,
        group_json: String,
        item_uuid: String,
        ct_b64: String,
    ) -> Result<Vec<u8>, FfiError> {
        crate::sharing::ffi_decrypt_group_item(group_json, item_uuid, ct_b64)
    }
}

/// Restricted API — Desktop only. Defined in a non-exported `impl` block so it
/// is compiled out of the mobile artifact entirely (data.md §1 rule 4 / client.md
/// §2). The desktop (GPUI) build enables `desktop-api` and calls this directly;
/// the UniFFI Turbo Module never sees it (CI symbol-check asserts absence).
#[cfg(feature = "desktop-api")]
impl MobileClient {
    /// Read the secret string for `handle`. ONLY available on Desktop.
    pub fn read_secret(&self, handle: SecretHandle) -> Result<String, FfiError> {
        self.inner
            .read_secret(handle)
            .map(|s| s.to_string())
            .map_err(FfiError::Core)
    }

    /// Export the entire vault as decrypted JSON (ZK boundary: the plaintext
    /// bytes are returned to the desktop process only — never to JS/web). The
    /// caller is responsible for writing the bytes to disk and zeroizing them.
    /// ONLY available on Desktop (`desktop-api` feature).
    pub async fn export_vault(&self) -> Result<Vec<u8>, FfiError> {
        self.inner.export_to_json().await.map_err(FfiError::Core)
    }
}
