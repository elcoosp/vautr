//! The top-level desktop view. Conditionally renders either the login screen
//! (when the vault is not yet unlocked) or the vault manager (after unlock).
//!
//! Uses gpui-component widgets: Button, Input, Label, v_flex, h_flex.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::{
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    h_flex, v_flex,
};
use std::sync::Arc;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::app::base_url;
use crate::auth_client::AuthClient;
use crate::state::{self, VaultConfig, VaultManagerState};
use vautr_app_state::VautrClient;
use vautr_crypto::{aead, kdf, key_tree};

/// The root desktop view.
pub struct DesktopView {
    focus_handle: FocusHandle,

    // ── Login form inputs ───────────────────────────────────────────────
    username_input: Entity<InputState>,
    password_input: Entity<InputState>,
    /// Keep subscriptions alive with the view.
    _subscriptions: Vec<Subscription>,

    // ── Login form extra ─────────────────────────────────────────────────
    login_status: String,
    server_url: String,

    // ── Vault state ─────────────────────────────────────────────────────
    pub vault: VaultManagerState,
    client: Option<Arc<VautrClient>>,
    dek: Option<Zeroizing<[u8; 32]>>,

    // ── Revealed secret ─────────────────────────────────────────────────
    revealed: Option<Zeroizing<String>>,
    active_handle: Option<vautr_app_state::orchestrator::SecretHandle>,
}

impl DesktopView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = VaultConfig::load();
        let stored_username = config
            .as_ref()
            .map(|c| c.username.clone())
            .unwrap_or_default();

        let username_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Username")
        });
        let password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Password")
        });

        // Pre-fill the stored username.
        if !stored_username.is_empty() {
            username_input.update(cx, |state, cx| {
                state.set_value(stored_username.as_str(), window, cx);
            });
        }

        let _subscriptions = vec![];

        Self {
            focus_handle: cx.focus_handle(),
            username_input,
            password_input,
            _subscriptions,
            login_status: String::new(),
            server_url: base_url(),
            vault: VaultManagerState::new(),
            client: None,
            dek: None,
            revealed: None,
            active_handle: None,
        }
    }

    fn is_unlocked(&self) -> bool {
        self.client.is_some()
    }

    fn username(&self, cx: &mut Context<Self>) -> String {
        self.username_input.read(cx).value().to_string()
    }

    fn password(&self, cx: &mut Context<Self>) -> String {
        self.password_input.read(cx).value().to_string()
    }

    // ── Login / Register ────────────────────────────────────────────────

    fn do_register(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let username = self.username(cx);
        let password = self.password(cx);
        let server_url = self.server_url.clone();

        if username.is_empty() || password.is_empty() {
            self.login_status = "Username and password are required.".into();
            cx.notify();
            return;
        }

        self.login_status = "Registering...".into();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let auth = AuthClient::new(&server_url);
            let result = auth.register(&username, &password).await;

            this.update(cx, |this, cx| match result {
                Ok(reg) => {
                    let cfg = VaultConfig {
                        username: username.clone(),
                        kdf_salt_b64: B64.encode(&reg.kdf_salt),
                    };
                    let _ = cfg.save();
                    this.login_status = format!(
                        "Registration successful! You can now log in.\n\
                         Recovery key (SAVE THIS): {}",
                        reg.recovery_mnemonic
                    );
                    cx.notify();
                }
                Err(e) => {
                    this.login_status = format!("Registration failed: {e}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let username = self.username(cx);
        let password = self.password(cx);
        let server_url = self.server_url.clone();

        if username.is_empty() || password.is_empty() {
            self.login_status = "Username and password are required.".into();
            cx.notify();
            return;
        }

        let kdf_salt = match VaultConfig::load().and_then(|c| c.kdf_salt_bytes().ok()) {
            Some(s) => s,
            None => {
                self.login_status =
                    "No local KDF salt found. Please register first.".into();
                cx.notify();
                return;
            }
        };

        self.login_status = "Logging in...".into();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let auth = AuthClient::new(&server_url);
            let login = match auth.login(&username, &password, &kdf_salt).await {
                Ok(l) => l,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.login_status = format!("Login failed: {e}");
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            let db_path = state::db_path();
            let client = match state::build_client(&db_path, &server_url, &login.session_token)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.login_status = format!("Vault setup failed: {e}");
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            let mp = Zeroizing::new(password);
            let mk = match kdf::derive_master_key(&mp, &kdf_salt) {
                Ok(m) => m,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.login_status = format!("MK derive: {e}");
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let kek = match key_tree::derive_kek(&mk) {
                Ok(k) => k,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.login_status = format!("KEK derive: {e}");
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            let dek: Zeroizing<[u8; 32]> = match (|| -> Result<Zeroizing<[u8; 32]>, String> {
                let svk_bytes = aead::decrypt(&kek, &Uuid::nil(), 0, &login.wrapped_svk)
                    .map_err(|_| "SVK unwrap failed".to_string())?;
                if svk_bytes.len() != 32 {
                    return Err("malformed SVK".into());
                }
                let mut svk = Zeroizing::new([0u8; 32]);
                svk.copy_from_slice(&svk_bytes);
                key_tree::derive_dek(&svk).map_err(|e| format!("DEK: {e}"))
            })() {
                Ok(d) => d,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.login_status = format!("DEK derive: {e}");
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            let local_gen = login.min_enc_key_gen.max(1);
            match client
                .unlock_with_password(mp, &kdf_salt, &login.wrapped_svk, Uuid::nil(), local_gen)
                .await
            {
                Ok(()) => {}
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.login_status = format!("Unlock failed: {e}");
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            }

            let items = match client.search("").await {
                Ok(items) => items,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.login_status = format!("Search failed: {e}");
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            this.update(cx, |this, cx| {
                this.client = Some(client);
                this.dek = Some(dek);
                this.vault.set_items(items);
                this.login_status.clear();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ── Vault actions ──────────────────────────────────────────────────

    fn do_reveal(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let uuid = match self.vault.selected_overview().map(|o| o.uuid) {
            Some(u) => u,
            None => {
                self.vault.show_error("no item selected");
                cx.notify();
                return;
            }
        };
        let Some(client) = self.client.clone() else {
            self.vault.show_error("vault is locked");
            cx.notify();
            return;
        };

        if let Some(handle) = self.active_handle.take() {
            client.release_secret(handle);
        }
        self.revealed = None;

        cx.spawn(async move |this, cx| {
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

            this.update(cx, |this, cx| match outcome {
                Ok((handle, secret)) => {
                    this.active_handle = Some(handle);
                    this.revealed = Some(secret);
                    this.vault.dismiss_error();
                    cx.notify();
                }
                Err(err) => {
                    this.vault.show_error(err);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }


    fn do_delete(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let uuid = match self.vault.selected_overview().map(|o| o.uuid) {
            Some(u) => u,
            None => {
                self.vault.show_error("no item selected");
                cx.notify();
                return;
            }
        };
        let Some(client) = self.client.clone() else {
            self.vault.show_error("vault is locked");
            cx.notify();
            return;
        };

        cx.spawn(async move |this, cx| {
            let outcome = client.delete_item(uuid).await;
            this.update(cx, |this, cx| match outcome {
                vautr_app_state::worker::TaskOutcome::Committed(_) => {
                    this.vault.dismiss_error();
                    cx.notify();
                    let c = this.client.clone();
                    cx.spawn(async move |this, cx| {
                        if let Some(c) = c {
                            if let Ok(items) = c.search("").await {
                                this.update(cx, |this, cx| {
                                    this.vault.set_items(items);
                                    cx.notify();
                                })
                                .ok();
                            }
                        }
                    })
                    .detach();
                }
                _ => {
                    this.vault.show_error("Delete failed");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_sync(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            self.vault.show_error("vault is locked");
            cx.notify();
            return;
        };

        cx.spawn(async move |this, cx| {
            let result = client.sync().await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        let c = this.client.clone();
                        cx.spawn(async move |this, cx| {
                            if let Some(c) = c {
                                if let Ok(items) = c.search("").await {
                                    this.update(cx, |this, cx| {
                                        this.vault.set_items(items);
                                        cx.notify();
                                    })
                                    .ok();
                                }
                            }
                        })
                        .detach();
                    }
                    Err(e) => {
                        this.vault.show_error(format!("Sync failed: {e}"));
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_lock(&mut self, cx: &mut Context<Self>) {
        if let Some(client) = self.client.clone() {
            if let Some(handle) = self.active_handle.take() {
                client.release_secret(handle);
            }
            self.revealed = None;
        }
        self.vault.lock();
        self.client = None;
        self.dek = None;
        cx.notify();
    }
}

// ── Render ──────────────────────────────────────────────────────────────

impl Render for DesktopView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.is_unlocked() {
            self.render_vault(cx).into_any_element()
        } else {
            self.render_login(cx).into_any_element()
        }
    }
}

impl DesktopView {
    fn render_login(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.login_status.clone();
        let has_error = !status.is_empty()
            && (status.contains("failed") || status.contains("required") || status.contains("No local"));

        v_flex()
            .gap_4()
            .p_8()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Vautr"),
            )
            .child(
                v_flex()
                    .gap_3()
                    .w_80()
                    .child(Input::new(&self.username_input).w_full())
                    .child(Input::new(&self.password_input).w_full()),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Button::new("register-btn")
                            .label("Register")
                            .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                                this.do_register(window, cx);
                            })),
                    )
                    .child(
                        Button::new("login-btn")
                            .primary()
                            .label("Login")
                            .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                                this.do_login(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .when(!status.is_empty(), |this| {
                        this.text_sm()
                            .px_2()
                            .when(has_error, |this| this.text_color(gpui::red()))
                            .child(status)
                    }),
            )
    }

    fn render_vault(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut rows = Vec::new();
        for (index, item) in self.vault.items.iter().enumerate() {
            let selected = self.vault.selected_index == Some(index);
            let title = item.title.clone();
            let subtitle = item.subtitle.clone();

            let row = div()
                .id(SharedString::from(format!("vault-row-{index}")))
                .flex()
                .flex_col()
                .px_3()
                .py_2()
                .rounded_md()
                .when(selected, |row| row.bg(rgb(0x27272a)))
                .cursor_pointer()
                .child(div().text_sm().child(title))
                .child(div().text_xs().text_color(rgb(0x71717a)).child(subtitle))
                .on_click(cx.listener(move |this, _, _window, cx| {
                    if this.vault.select_item(index) {
                        this.revealed = None;
                        if let Some(client) = this.client.clone() {
                            if let Some(h) = this.active_handle.take() {
                                client.release_secret(h);
                            }
                        }
                        cx.notify();
                    }
                }));
            rows.push(row);
        }

        let revealed_text = self
            .revealed
            .as_deref()
            .map(|s| {
                div()
                    .px_3()
                    .py_2()
                    .mt_2()
                    .bg(rgb(0x18181b))
                    .rounded_md()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_xs().text_color(rgb(0xa1a1aa)).child("Secret"))
                    .child(div().text_sm().child(s.to_string()))
            })
            .unwrap_or_else(|| div());

        let error = self.vault.error_message.clone().unwrap_or_default();
        let has_error = !error.is_empty();

        v_flex()
            .size_full()
            .child(
                // ── Header bar ───────────────────────────────────────
                h_flex()
                    .justify_between()
                    .items_center()
                    .px_6()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(0x27272a))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child("Vautr"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("sync-btn")
                                    .label("Sync")
                                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                                        this.do_sync(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("lock-btn")
                                    .danger()
                                    .label("Lock")
                                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, _window, cx| {
                                        this.do_lock(cx);
                                    })),
                            ),
                    ),
            )
            .child(
                // ── Action bar ───────────────────────────────────────
                h_flex()
                    .gap_2()
                    .px_6()
                    .py_2()
                    .child(
                        Button::new("add-btn")
                            .primary()
                            .label("+ Add")
                            .on_click(cx.listener(|this, _: &gpui::ClickEvent, _window, cx| {
                                this.vault.show_error("Add item dialog not yet implemented");
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("reveal-btn")
                            .label("Reveal")
                            .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                                this.do_reveal(window, cx);
                            })),
                    )
                    .child(
                        Button::new("delete-btn")
                            .danger()
                            .label("Delete")
                            .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                                this.do_delete(window, cx);
                            })),
                    ),
            )
            .child(
                // ── Error banner ─────────────────────────────────────
                div()
                    .when(has_error, |this| {
                        this.px_6()
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(rgb(0x450a0a))
                                    .text_color(rgb(0xfca5a5))
                                    .text_sm()
                                    .child(error),
                            )
                    }),
            )
            .child(
                // ── Item list ────────────────────────────────────────
                div()
                    .id("item-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_4()
                    .py_2()
                    .children(rows),
            )
            .child(revealed_text)
    }
}

impl Focusable for DesktopView {
    fn focus_handle(&self, _app: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
