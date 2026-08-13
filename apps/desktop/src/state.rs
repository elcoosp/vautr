//! Testable, GPUI-agnostic view state for the desktop client.
//!
//! Three state machines:
//! 1. `LoginScreenState` — login/register form. Pure data + an `on_unlock`
//!    callback fired on successful login.
//! 2. `VaultManagerState` — item list, selection, error dialog, lock.
//! 3. `VaultConfig` — locally-persisted config (KDF salt, username) so the
//!    desktop client can re-derive the MK on subsequent logins.
//!
//! The GPUI layer applies transitions inside `cx.update_mut(...)` followed by
//! `cx.notify()` — never the other way around — which upholds the notify rule
//! (no `notify` outside an update/event-dispatch closure).

use std::sync::Arc;
use vautr_app_state::VautrClient;
use vautr_domain::DecryptedOverview;

// ── VaultConfig (locally persisted) ─────────────────────────────────────

/// Persisted config stored alongside the local SQLite vault DB.
/// Serialized as JSON to `~/.vautr/config.json`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct VaultConfig {
    /// The username (email) used to register/login.
    pub username: String,
    /// Argon2id KDF salt (32 bytes), base64-encoded for JSON.
    pub kdf_salt_b64: String,
}

impl VaultConfig {
    /// Load from the default config path, or `None` if no config exists.
    pub fn load() -> Option<Self> {
        let path = config_path();
        let data = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Persist to the default config path.
    pub fn save(&self) -> Result<(), String> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create config dir: {e}"))?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize config: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("write config: {e}"))
    }

    /// Decode the stored base64 KDF salt to bytes.
    pub fn kdf_salt_bytes(&self) -> Result<[u8; 32], String> {
        use base64::{Engine, engine::general_purpose::STANDARD as B64};
        let bytes = B64
            .decode(&self.kdf_salt_b64)
            .map_err(|e| format!("decode kdf_salt: {e}"))?;
        if bytes.len() != 32 {
            return Err("kdf_salt is not 32 bytes".into());
        }
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&bytes);
        Ok(salt)
    }
}

/// Default session-token path: `$HOME/.vautr/session.json`.
/// Holds the bearer token from a prior OPAQUE login so the desktop can offer
/// a one-click "Restore session" on next launch (avoids re-running OPAQUE).
/// NOTE: the token is a bearer credential; it is stored as-is (base64-wrapped
/// for JSON safety). A production build should seal this with an OS keychain
/// or a machine-bound key — tracked separately from this parity work.
fn session_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home)
        .join(".vautr")
        .join("session.json")
}

/// Everything needed to re-establish a session without re-running OPAQUE:
/// the bearer `token`, the server-wrapped SVK, and the minimum encryption
/// generation. The password is still required to derive the DEK client-side.
#[derive(Clone, Debug)]
pub struct PersistedSession {
    pub token: String,
    pub wrapped_svk_b64: String,
    pub min_enc_key_gen: u64,
}

/// Persist a session (token + wrapped SVK + min gen) as base64-wrapped JSON.
/// Returns `false` if the parent dir cannot be created or the write fails.
pub fn save_session(session: &PersistedSession) -> bool {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    let wrapped = serde_json::json!({
        "token_b64": B64.encode(session.token.as_bytes()),
        "wrapped_svk_b64": session.wrapped_svk_b64,
        "min_enc_key_gen": session.min_enc_key_gen,
    });
    let path = session_path();
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&wrapped).unwrap_or_default(),
    )
    .is_ok()
}

/// Load a persisted session, or `None` if absent/unreadable.
pub fn load_session() -> Option<PersistedSession> {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    let data = std::fs::read_to_string(session_path()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&data).ok()?;
    let token = {
        let b64 = v.get("token_b64")?.as_str()?;
        let bytes = B64.decode(b64).ok()?;
        String::from_utf8(bytes).ok()?
    };
    let wrapped_svk_b64 = v.get("wrapped_svk_b64")?.as_str()?.to_string();
    let min_enc_key_gen = v.get("min_enc_key_gen")?.as_u64()?;
    Some(PersistedSession {
        token,
        wrapped_svk_b64,
        min_enc_key_gen,
    })
}

/// Remove any persisted session (called on explicit logout/lock).
pub fn clear_session() {
    let _ = std::fs::remove_file(session_path());
}

/// Default config directory: `$HOME/.vautr/`.
fn config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home)
        .join(".vautr")
        .join("config.json")
}

/// Default vault DB path: `$HOME/.vautr/vault.db`.
pub fn db_path() -> String {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    format!("{}/.vautr/vault.db", home)
}

// ── LoginScreenState ─────────────────────────────────────────────────────

/// The login/register form state.
pub struct LoginScreenState {
    /// The server base URL (from env var or default).
    pub base_url: String,
    /// The username (email) entered by the user.
    pub username: String,
    /// The password entered by the user (never persisted).
    pub password: String,
    /// Locally-persisted KDF salt. `None` means the user must register first.
    pub kdf_salt: Option<[u8; 32]>,
    /// Set after a successful registration (shown to the user once).
    pub recovery_mnemonic: Option<String>,
    /// Status message (e.g. "Registering...", "Login failed: ...").
    pub status: String,
    /// Callback fired on successful login: `(session_token, wrapped_svk, min_enc_key_gen)`.
    pub on_unlock: Option<Box<dyn Fn(String, Vec<u8>, u64) + Send + 'static>>,
}

impl LoginScreenState {
    pub fn new(base_url: &str) -> Self {
        let config = VaultConfig::load();
        Self {
            base_url: base_url.to_string(),
            username: config
                .as_ref()
                .map(|c| c.username.clone())
                .unwrap_or_default(),
            password: String::new(),
            kdf_salt: config.as_ref().and_then(|c| c.kdf_salt_bytes().ok()),
            recovery_mnemonic: None,
            status: String::new(),
            on_unlock: None,
        }
    }
}

// ── VaultManagerState (unchanged core) ───────────────────────────────────

/// The vault-lock screen's password entry + the items the client holds.
pub struct VaultManagerState {
    /// The core orchestrator. Shared so event listeners and the view can both
    /// reach it. Wrapped in `Option` so a test can drive pure state transitions
    /// without constructing a `VautrClient` (which needs a DB connection).
    pub client: Option<Arc<VautrClient>>,
    /// Vault items (most-recently-used order from `VautrClient::search`).
    pub items: Vec<DecryptedOverview>,
    /// Currently selected item index in `items`.
    pub selected_index: Option<usize>,
    /// Populated when a secret reveal fails; drives the error `Dialog`.
    pub error_message: Option<String>,
}

impl VaultManagerState {
    pub fn new() -> Self {
        Self {
            client: None,
            items: Vec::new(),
            selected_index: None,
            error_message: None,
        }
    }

    /// Attach a core client (used after unlock) and load its overview list.
    pub fn attach_client(&mut self, client: Arc<VautrClient>, items: Vec<DecryptedOverview>) {
        self.client = Some(client);
        self.items = items;
        if let Some(sel) = self.selected_index {
            if sel >= self.items.len() {
                self.selected_index = None;
            }
        }
    }

    /// Replace the item list (e.g. after a search / sync push).
    pub fn set_items(&mut self, items: Vec<DecryptedOverview>) {
        self.items = items;
        if let Some(sel) = self.selected_index {
            if sel >= self.items.len() {
                self.selected_index = None;
            }
        }
    }

    /// Select the item at `index`. Returns false when out of range.
    pub fn select_item(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.selected_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Move selection down (wrapping). No-op on an empty list.
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let next = self.selected_index.map(|i| i + 1).unwrap_or(0);
        self.selected_index = Some(next % self.items.len());
    }

    /// Move selection up (wrapping). No-op on an empty list.
    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let prev = self
            .selected_index
            .map(|i| if i == 0 { self.items.len() - 1 } else { i - 1 })
            .unwrap_or(self.items.len() - 1);
        self.selected_index = Some(prev);
    }

    pub fn selected_overview(&self) -> Option<&DecryptedOverview> {
        self.selected_index.and_then(|i| self.items.get(i))
    }

    /// Show the error dialog with `message`.
    pub fn show_error(&mut self, message: impl Into<String>) {
        self.error_message = Some(message.into());
    }

    /// Dismiss the error dialog (the `Dialog` cancel action).
    pub fn dismiss_error(&mut self) {
        self.error_message = None;
    }

    /// Lock the vault: zeroize the in-memory SVK/DEK and every live secret
    /// handle, then clear the UI list + selection.
    pub fn lock(&mut self) {
        if let Some(client) = &self.client {
            let client = client.clone();
            let handle = tokio::runtime::Handle::current();
            let _ = handle.spawn(async move {
                client.lock().await;
            });
        }
        self.items.clear();
        self.selected_index = None;
        self.error_message = None;
        self.client = None;
    }
}

impl Default for VaultManagerState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Client construction helpers ──────────────────────────────────────────

/// Build a real `VautrClient` connected to a local SQLite DB, initialized with
/// the schema, and wired to the sync transport with the given bearer token.
pub async fn build_client(
    db_path: &str,
    base_url: &str,
    token: &str,
) -> Result<Arc<VautrClient>, String> {
    use sea_orm::Database;

    // Ensure the parent directory exists.
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create db dir: {e}"))?;
    }

    let db_url = format!("sqlite://{}?mode=rwc", db_path);
    let db = Database::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;

    // Initialize the schema (idempotent via IF NOT EXISTS).
    vautr_db::migrate::init(&db)
        .await
        .map_err(|e| format!("db init: {e}"))?;

    let client = Arc::new(VautrClient::new(db));

    // Connect the sync transport. Uses Uuid::nil() as the server_user_id
    // because the MP-wrapped SVK from vautr_keyring::wrap::wrap_svk
    // binds to Uuid::nil() via AEAD AD.
    client
        .connect_sync(base_url, token, uuid::Uuid::nil())
        .await;

    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn overview(title: &str) -> DecryptedOverview {
        DecryptedOverview {
            uuid: Uuid::new_v4(),
            title: title.into(),
            subtitle: String::new(),
            icon_key: String::new(),
            urls: Vec::new(),
            updated_at: 0,
        }
    }

    #[test]
    fn select_item_drives_selection() {
        let mut state = VaultManagerState::new();
        state.set_items(vec![overview("Email"), overview("Bank"), overview("SSH")]);

        assert!(state.select_item(1));
        assert_eq!(state.selected_index, Some(1));
        assert_eq!(state.selected_overview().unwrap().title, "Bank");

        assert!(!state.select_item(99));
        assert_eq!(state.selected_index, Some(1));

        state.select_next();
        assert_eq!(state.selected_index, Some(2));
        state.select_next();
        assert_eq!(state.selected_index, Some(0));
        state.select_prev();
        assert_eq!(state.selected_index, Some(2));
    }

    #[test]
    fn error_dialog_drives_and_dismisses() {
        let mut state = VaultManagerState::new();
        state.show_error("vault locked: cannot reveal secret");
        assert_eq!(
            state.error_message.as_deref(),
            Some("vault locked: cannot reveal secret")
        );

        state.dismiss_error();
        assert!(state.error_message.is_none());
    }

    #[test]
    fn lock_zeroes_selection_and_detaches_client() {
        let mut state = VaultManagerState::new();
        state.set_items(vec![overview("A"), overview("B")]);
        state.select_item(0);
        state.show_error("boom");

        state.lock();
        assert!(state.items.is_empty());
        assert!(state.selected_index.is_none());
        assert!(state.error_message.is_none());
        assert!(state.client.is_none());
    }
}
