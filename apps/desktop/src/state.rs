//! Testable, GPUI-agnostic view state for the desktop `VaultManagerView`.
//!
//! The view renders this state; all selection / error-dialog transitions live
//! here so they can be unit-tested without a GPU runtime. The GPUI layer
//! (`vault_manager_view`) applies these transitions inside `cx.update_mut(...)`
//! followed by `cx.notify()` — never the other way around — which upholds the
//! notify rule (no `notify` outside an update/event-dispatch closure).

use std::sync::Arc;
use vautr_app_state::VautrClient;
use vautr_domain::DecryptedOverview;

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
        // Keep a stale selection in range after a refresh.
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
    /// handle, then clear the UI list + selection. Only touches the core when a
    /// client is attached.
    pub fn lock(&mut self) {
        if let Some(client) = &self.client {
            // Spawn so the async `lock()` (which wipes the Zeroizing keys and
            // releases all secret handles) runs without blocking the UI thread.
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

        // Selecting an item drives the highlighted index.
        assert!(state.select_item(1));
        assert_eq!(state.selected_index, Some(1));
        assert_eq!(state.selected_overview().unwrap().title, "Bank");

        // Out-of-range selection is rejected and leaves state unchanged.
        assert!(!state.select_item(99));
        assert_eq!(state.selected_index, Some(1));

        // Keyboard navigation wraps.
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
        // Trigger the error dialog (e.g. a failed reveal).
        state.show_error("vault locked: cannot reveal secret");
        assert_eq!(
            state.error_message.as_deref(),
            Some("vault locked: cannot reveal secret")
        );

        // Dismiss clears it.
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
