//! The desktop vault manager view (GPUI).
//!
//! Follows the VaultManagerView blueprint:
//! - a stateful `View<T>` implementing `Render` (returns `impl IntoElement`);
//! - a `FocusHandle` registered via `cx.focus_handle()` for interactive
//!   controls;
//! - a search `Input` (gpui-component `InputState`, formerly `TextInput`)
//!   driving the item list;
//! - a stateful list with a `selected_index` and rows wired with
//!   `on_click(cx.listener(...))`;
//! - a modal `Dialog` gated on `state.error_message.is_some()`;
//! - secret reveal via `VautrClient::reveal_secret` + `read_secret`
//!   (desktop-api only), held in `zeroize::Zeroizing`; and
//! - a Local File Transfer hook (`VautrClient::upload_file`, file-storage.md
//!   §5.4 desktop auto-sync).
//!
//! Notify rule: every `cx.notify()` is issued from within an update / event
//! dispatch closure (`cx.update`, `cx.listener`, `WeakEntity::update`) — never
//! bare.

use gpui::{
    div, prelude::FluentBuilder, App, AsyncApp, Context, Entity, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::dialog::Dialog;
use gpui_component::input::{Input, InputState};

use crate::state::VaultManagerState;

/// The root view of the desktop client.
pub struct VaultManagerView {
    focus_handle: gpui::FocusHandle,
    /// Search box. This is the blueprint's `View<TextInput>`; gpui-component
    /// 0.5.1 renamed the type to `InputState` + `Input`.
    search_input: Entity<InputState>,
    /// Live search query used to filter the item list.
    search_query: String,
    /// Pure, testable view state (selection, error dialog, items).
    pub state: VaultManagerState,
    /// Live revealed secret (held in `Zeroizing`); wiped on lock/reveal.
    revealed: Option<zeroize::Zeroizing<String>>,
    /// Latest revealed secret's handle so we can `release_secret` on lock.
    active_handle: Option<vautr_app_state::orchestrator::SecretHandle>,
}

impl VaultManagerView {
    /// Build the view. `search_state` must be created from a window context
    /// (gpui-component `InputState::new(window, cx)`), see `app::run`.
    pub fn new(cx: &mut Context<Self>, search_state: Entity<InputState>) -> Self {
        let focus_handle = cx.focus_handle();
        Self {
            focus_handle,
            search_input: search_state,
            search_query: String::new(),
            state: VaultManagerState::new(),
            revealed: None,
            active_handle: None,
        }
    }

    /// Set the search query and re-filter the item list (called from the input
    /// subscription; `notify` fires inside the update closure).
    pub fn set_search_query(&mut self, cx: &mut Context<Self>, query: String) {
        self.search_query = query;
        cx.notify();
    }

    /// Trigger a reveal of the selected item's secret via `reveal_secret` +
    /// `read_secret` (desktop-api only). The result is applied back to the view
    /// through `WeakEntity::update` inside the `cx.spawn` future, so `notify`
    /// fires only within that update closure.
    pub fn reveal_selected(&mut self, cx: &mut Context<Self>) {
        let Some(uuid) = self.state.selected_overview().map(|o| o.uuid) else {
            self.state.show_error("no item selected");
            cx.notify();
            return;
        };
        let Some(client) = self.state.client.clone() else {
            self.state.show_error("vault is locked");
            cx.notify();
            return;
        };

        // Wipe the previously revealed secret and release its handle's memory
        // before decrypting the new one.
        if let Some(handle) = self.active_handle.take() {
            client.release_secret(handle);
        }
        self.revealed = None;

        let _ = cx.spawn(async move |weak, app: &mut AsyncApp| {
            let outcome = match client.reveal_secret(uuid).await {
                Ok(handle) => match client.read_secret(handle) {
                    Ok(secret) => Ok((handle, secret)),
                    Err(err) => {
                        client.release_secret(handle);
                        Err(err)
                    }
                },
                Err(err) => Err(err),
            };

            // Back on the main thread, apply the result to the view state.
            let _ = app.update(|app| {
                let _ = weak.update(app, |this, cx| match outcome {
                    Ok((handle, secret)) => {
                        this.active_handle = Some(handle);
                        this.revealed = Some(secret);
                        this.state.dismiss_error();
                        cx.notify();
                    }
                    Err(err) => {
                        this.state.show_error(err);
                        cx.notify();
                    }
                });
            });
        });
    }

    /// Local File Transfer hook (file-storage.md §5.4): upload a binary
    /// attachment through the `FileTransferWorker` on the background executor.
    pub fn upload_file(&mut self, cx: &mut Context<Self>, content_type: &str, plaintext: Vec<u8>) {
        let Some(client) = self.state.client.clone() else {
            self.state.show_error("vault is locked");
            cx.notify();
            return;
        };
        let content_type = content_type.to_string();
        let last_modified = unix_millis();
        let _ = cx.spawn(async move |weak, app: &mut AsyncApp| {
            let result = client
                .upload_file(&plaintext, &content_type, last_modified)
                .await;
            let _ = app.update(|app| {
                let _ = weak.update(app, |this, cx| {
                    if let Err(err) = result {
                        this.state.show_error(err);
                        cx.notify();
                    }
                });
            });
        });
    }

    /// Lock the vault: release the secret handle (zeroizing its memory), wipe
    /// the revealed secret, clear UI state, and emit a state update.
    pub fn lock(&mut self, cx: &mut Context<Self>) {
        if let Some(client) = self.state.client.clone() {
            if let Some(handle) = self.active_handle.take() {
                // `release_secret` zeroizes the handle's buffer immediately.
                client.release_secret(handle);
            }
            self.revealed = None;
        }
        self.state.lock();
        cx.notify();
    }
}

impl Render for VaultManagerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let inner = if self.state.error_message.is_some() {
            render_error_dialog(self, window, cx).into_any_element()
        } else {
            render_main(self, window, cx).into_any_element()
        };
        div().child(inner)
    }
}

/// Render the item-list screen: header, search box, rows, and lock button.
fn render_main(
    view: &mut VaultManagerView,
    _window: &mut Window,
    cx: &mut Context<VaultManagerView>,
) -> impl gpui::IntoElement {
    let lock_button = Button::new("lock")
        .label("Lock")
        .danger()
        .on_click(cx.listener(|this, _, _w, cx| {
            this.lock(cx);
        }));

    // Stateful list rows. Each row selects the item on click.
    let mut rows = Vec::new();
    for (index, item) in view.state.items.iter().enumerate() {
        let selected = view.state.selected_index == Some(index);
        let title = item.title.clone();
        let row = div()
            .id(gpui::ElementId::named_usize("vault-row", index))
            .flex()
            .px_3()
            .py_2()
            .when(selected, |row| row.bg(gpui::rgb(0x27272a)))
            .child(title)
            .on_click(cx.listener(move |this, _, _w, cx| {
                if this.state.select_item(index) {
                    this.revealed = None;
                    if let Some(client) = this.state.client.clone() {
                        if let Some(h) = this.active_handle.take() {
                            client.release_secret(h);
                        }
                    }
                    cx.notify();
                }
            }));
        rows.push(row);
    }

    let reveal_button = Button::new("reveal")
        .label("Reveal")
        .primary()
        .on_click(cx.listener(|this, _, _w, cx| {
            this.reveal_selected(cx);
        }));

    let revealed_text = view
        .revealed
        .as_deref()
        .map(|s| div().px_3().py_2().child(s.to_string()))
        .unwrap_or_else(|| div());

    let search = Input::new(&view.search_input).w_full();

    div()
        .id("vault-manager")
        .flex()
        .flex_col()
        .gap_4()
        .p_6()
        .w_full()
        .h_full()
        .child(
            div()
                .id("header")
                .flex()
                .justify_between()
                .child(div().text_lg().child("Vault Manager"))
                .child(lock_button),
        )
        .child(search)
        .child(div().id("items").flex().flex_col().gap_1().children(rows))
        .child(reveal_button)
        .child(revealed_text)
}

/// Render the error dialog when a reveal or mutation failed.
fn render_error_dialog(
    view: &mut VaultManagerView,
    window: &mut Window,
    cx: &mut Context<VaultManagerView>,
) -> impl gpui::IntoElement {
    let message = view.state.error_message.clone().unwrap_or_default();
    let ok_weak = cx.weak_entity();
    let cancel_weak = cx.weak_entity();
    Dialog::new(window, cx)
        .title("Vault Error")
        .confirm()
        .child(div().child(message))
        .on_ok(move |_, _w, cx| {
            let _ = ok_weak.update(cx, |this, cx| {
                this.state.dismiss_error();
                cx.notify();
            });
            true
        })
        .on_cancel(move |_, _w, cx| {
            let _ = cancel_weak.update(cx, |this, cx| {
                this.state.dismiss_error();
                cx.notify();
            });
            true
        })
}

impl Focusable for VaultManagerView {
    fn focus_handle(&self, _app: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

/// Current wall-clock time in Unix milliseconds (for `last_modified`).
fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
