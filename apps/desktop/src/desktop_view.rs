//! The top-level desktop view. Conditionally renders either the login screen
//! (when the vault is not yet unlocked) or the vault manager (after unlock).
//!
//! All async work (auth, DB init, client construction, search) runs through
//! `cx.spawn`. A background tokio runtime must be active (started in `app.rs`).

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use gpui::{
    div, prelude::FluentBuilder, AsyncApp, Context, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement,
    Styled, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::dialog::Dialog;
use std::sync::Arc;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::app::base_url;
use crate::auth_client::AuthClient;
use crate::state::{self, VaultConfig, VaultManagerState};
use vautr_app_state::VautrClient;
use vautr_crypto::{aead, kdf, key_tree};
use vautr_domain::{DecryptedOverview, DecryptedSecret, DomainModel, ItemMetadata};

/// The root desktop view. Owns the login form state, the vault state, and the
/// active screen mode.
pub struct DesktopView {
    focus_handle: gpui::FocusHandle,

    // ── Login form fields ──────────────────────────────────────────────
    login_username: String,
    login_password: String,
    login_status: String,
    /// The server base URL.
    server_url: String,

    // ── Vault state ─────────────────────────────────────────────────────
    /// Vault items list.
    pub vault: VaultManagerState,

    // ── Vault UI extras ─────────────────────────────────────────────────
    revealed: Option<Zeroizing<String>>,
    active_handle: Option<vautr_app_state::orchestrator::SecretHandle>,

    // ── Add-item dialog ─────────────────────────────────────────────────
    add_title: String,
    add_username: String,
    add_password: String,
    add_url: String,
    show_add_dialog: bool,

    // ── Client handle across spawns ─────────────────────────────────────
    client: Option<Arc<VautrClient>>,
    /// Data Encryption Key derived from SVK at unlock. Used to encrypt
    /// new item payloads before saving through the orchestrator.
    dek: Option<Zeroizing<[u8; 32]>>,
}

impl DesktopView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let config = VaultConfig::load();
        Self {
            focus_handle: cx.focus_handle(),
            login_username: config
                .as_ref()
                .map(|c| c.username.clone())
                .unwrap_or_default(),
            login_password: String::new(),
            login_status: String::new(),
            server_url: base_url(),
            vault: VaultManagerState::new(),
            revealed: None,
            active_handle: None,
            add_title: String::new(),
            add_username: String::new(),
            add_password: String::new(),
            add_url: String::new(),
            show_add_dialog: false,
            client: None,
            dek: None,
        }
    }

    /// True when the vault is unlocked.
    fn is_unlocked(&self) -> bool {
        self.client.is_some()
    }

    // ── Login / Register actions ───────────────────────────────────────

    fn do_register(&mut self, cx: &mut Context<Self>) {
        let username = self.login_username.clone();
        let password = self.login_password.clone();
        let server_url = self.server_url.clone();

        if username.is_empty() || password.is_empty() {
            self.login_status = "Username and password are required.".into();
            cx.notify();
            return;
        }

        self.login_status = "Registering...".into();
        cx.notify();

        let _ = cx.spawn(async move |weak, app: &mut AsyncApp| {
            let auth = AuthClient::new(&server_url);
            let result = auth.register(&username, &password).await;

            let _ = app.update(|app| {
                let _ = weak.update(app, |this, cx| match result {
                    Ok(reg) => {
                        // Persist config for future logins.
                        let cfg = VaultConfig {
                            username: username.clone(),
                            kdf_salt_b64: B64
                                .encode(&reg.kdf_salt),
                        };
                        let _ = cfg.save();
                        this.login_status = format!(
                            "Registration successful! You can now log in.\n\
                             Recovery key (SAVE THIS): {}",
                            reg.recovery_mnemonic
                        );
                        this.login_password.clear();
                        cx.notify();
                    }
                    Err(e) => {
                        this.login_status = format!("Registration failed: {e}");
                        cx.notify();
                    }
                });
            });
        });
    }

    fn do_login(&mut self, cx: &mut Context<Self>) {
        let username = self.login_username.clone();
        let password = self.login_password.clone();
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
        let login_password = password.clone();
        cx.notify();

        let _ = cx.spawn(async move |weak, app: &mut AsyncApp| {
            // Step 1: OPAQUE login
            let auth = AuthClient::new(&server_url);
            let login = match auth.login(&username, &login_password, &kdf_salt).await {
                Ok(l) => l,
                Err(e) => {
                    let _ = app.update(|app| {
                        let _ = weak.update(app, |this, cx| {
                            this.login_status = format!("Login failed: {e}");
                            cx.notify();
                        });
                    });
                    return;
                }
            };

            // Step 2: Build VautrClient + unlock
            let db_path = state::db_path();
            let client = match state::build_client(&db_path, &server_url, &login.session_token)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = app.update(|app| {
                        let _ = weak.update(app, |this, cx| {
                            this.login_status = format!("Vault setup failed: {e}");
                            cx.notify();
                        });
                    });
                    return;
                }
            };

            // Derive MK → KEK → unwrap SVK → unlock the orchestrator.
            let mp = Zeroizing::new(login_password);
            let mk = match kdf::derive_master_key(&mp, &kdf_salt) {
                Ok(m) => m,
                Err(e) => {
                    let _ = app.update(|app| {
                        let _ = weak.update(app, |this, cx| {
                            this.login_status = format!("MK derive: {e}");
                            cx.notify();
                        });
                    });
                    return;
                }
            };
            let kek = match key_tree::derive_kek(&mk) {
                Ok(k) => k,
                Err(e) => {
                    let _ = app.update(|app| {
                        let _ = weak.update(app, |this, cx| {
                            this.login_status = format!("KEK derive: {e}");
                            cx.notify();
                        });
                    });
                    return;
                }
            };

            // Derive the DEK from the unwrapped SVK for later use (add_item).
            // The orchestrator will derive the same DEK internally during unlock.
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
                    let _ = app.update(|app| {
                        let _ = weak.update(app, |this, cx| {
                            this.login_status = format!("DEK derive: {e}");
                            cx.notify();
                        });
                    });
                    return;
                }
            };

            // Decrypt SVK from the server blob to verify it matches our KEK.
            // The orchestrator's `unlock_with_password` does this internally.
            match client
                .unlock_with_password(
                    mp,
                    &kdf_salt,
                    &login.wrapped_svk,
                    Uuid::nil(),
                    login.min_enc_key_gen.max(1),
                )
                .await
            {
                Ok(()) => {}
                Err(e) => {
                    let _ = app.update(|app| {
                        let _ = weak.update(app, |this, cx| {
                            this.login_status = format!("Unlock failed: {e}");
                            cx.notify();
                        });
                    });
                    return;
                }
            }

            // Step 3: Load items
            let items = match client.search("").await {
                Ok(items) => items,
                Err(e) => {
                    let _ = app.update(|app| {
                        let _ = weak.update(app, |this, cx| {
                            this.login_status = format!("Search failed: {e}");
                            cx.notify();
                        });
                    });
                    return;
                }
            };

            // Step 4: Transition to vault view on the main thread.
            let _ = app.update(|app| {
                let _ = weak.update(app, |this, cx| {
                    this.client = Some(client);
                    this.dek = Some(dek);
                    this.vault.set_items(items);
                    this.login_status.clear();
                    cx.notify();
                });
            });
        });
    }

    // ── Vault actions ──────────────────────────────────────────────────

    fn do_reveal(&mut self, cx: &mut Context<Self>) {
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

            let _ = app.update(|app| {
                let _ = weak.update(app, |this, cx| match outcome {
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
                });
            });
        });
    }

    fn do_add_item(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            self.vault.show_error("vault is locked");
            cx.notify();
            return;
        };

        let Some(dek) = self.dek.clone() else {
            self.vault.show_error("vault is locked (no DEK)");
            cx.notify();
            return;
        };

        let title = self.add_title.clone();
        let subtitle = self.add_username.clone();
        let password_str = self.add_password.clone();
        let url = self.add_url.clone();

        if title.is_empty() {
            self.vault.show_error("Title is required.");
            cx.notify();
            return;
        }

        self.show_add_dialog = false;
        self.add_title.clear();
        self.add_username.clear();
        self.add_password.clear();
        self.add_url.clear();

        let enc_key_gen = client.current_key_gen();

        let _ = cx.spawn(async move |weak, app: &mut AsyncApp| {
            let item_uuid = Uuid::new_v4();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);

            let overview = DecryptedOverview {
                uuid: item_uuid,
                title: title.clone(),
                subtitle: subtitle.clone(),
                icon_key: "key".into(),
                urls: if url.is_empty() { vec![] } else { vec![url] },
                updated_at: now,
            };

            let secret = DecryptedSecret {
                password: Zeroizing::new(password_str),
                totp: None,
                notes: Zeroizing::new(String::new()),
                fields: vec![],
            };

            let secret_json = serde_json::to_vec(&secret).unwrap_or_default();
            let payload = match aead::encrypt(&dek, &item_uuid, enc_key_gen, &secret_json) {
                Ok(p) => p,
                Err(e) => {
                    let _ = app.update(|app| {
                        let _ = weak.update(app, |this, cx| {
                            this.vault.show_error(format!("Encrypt failed: {e}"));
                            cx.notify();
                        });
                    });
                    return;
                }
            };

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
            let _ = app.update(|app| {
                let _ = weak.update(app, |this, cx| match outcome {
                    vautr_app_state::worker::TaskOutcome::Committed(_) => {
                        this.vault.dismiss_error();
                        // Refresh the list.
                        let c = this.client.clone();
                        let _ = cx.spawn(async move |w2, a2: &mut AsyncApp| {
                            if let Some(c) = c {
                                if let Ok(items) = c.search("").await {
                                    let _ = a2.update(|a2| {
                                        let _ = w2.update(a2, |t2, cx2| {
                                            t2.vault.set_items(items);
                                            cx2.notify();
                                        });
                                    });
                                }
                            }
                        });
                    }
                    _ => {
                        this.vault.show_error("Save failed");
                        cx.notify();
                    }
                });
            });
        });
    }

    fn do_delete(&mut self, cx: &mut Context<Self>) {
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

        let _ = cx.spawn(async move |weak, app: &mut AsyncApp| {
            let outcome = client.delete_item(uuid).await;

            let _ = app.update(|app| {
                let _ = weak.update(app, |this, cx| match outcome {
                    vautr_app_state::worker::TaskOutcome::Committed(_) => {
                        this.vault.dismiss_error();
                        // Refresh the list.
                        let client = this.client.clone();
                        let _ = cx.spawn(async move |weak2, app2: &mut AsyncApp| {
                            if let Some(c) = client {
                                if let Ok(items) = c.search("").await {
                                    let _ = app2.update(|app2| {
                                        let _ = weak2.update(app2, |this2, cx2| {
                                            this2.vault.set_items(items);
                                            cx2.notify();
                                        });
                                    });
                                }
                            }
                        });
                        cx.notify();
                    }
                    _ => {
                        this.vault.show_error("Delete failed");
                        cx.notify();
                    }
                });
            });
        });
    }

    fn do_sync(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            self.vault.show_error("vault is locked");
            cx.notify();
            return;
        };

        let _ = cx.spawn(async move |weak, app: &mut AsyncApp| {
            let result = client.sync().await;

            let _ = app.update(|app| {
                let _ = weak.update(app, |this, cx| {
                    match result {
                        Ok(()) => {
                            // Refresh after sync.
                            let client = this.client.clone();
                            let _ = cx.spawn(async move |weak2, app2: &mut AsyncApp| {
                                if let Some(c) = client {
                                    if let Ok(items) = c.search("").await {
                                        let _ = app2.update(|app2| {
                                            let _ = weak2.update(app2, |this2, cx2| {
                                                this2.vault.set_items(items);
                                                cx2.notify();
                                            });
                                        });
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            this.vault.show_error(format!("Sync failed: {e}"));
                            cx.notify();
                        }
                    }
                });
            });
        });
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.is_unlocked() {
            self.render_vault(window, cx).into_any_element()
        } else {
            self.render_login(cx).into_any_element()
        }
    }
}

impl DesktopView {
    fn render_login(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let username = self.login_username.clone();
        let password = self.login_password.clone();
        let status = self.login_status.clone();

        // Using simple GPUI divs for the login form.
        div()
            .id("login-screen")
            .flex()
            .flex_col()
            .gap_4()
            .p_8()
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w_96()
                    .child(div().text_2xl().child("Vautr"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().w_24().child("Username:"))
                            .child(
                                div()
                                    .flex_1()
                                    .px_2()
                                    .py_1()
                                    .bg(gpui::rgb(0x18181b))
                                    .border_1()
                                    .border_color(gpui::rgb(0x3f3f46))
                                    .rounded_md()
                                    .child(username.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().w_24().child("Password:"))
                            .child(
                                div()
                                    .flex_1()
                                    .px_2()
                                    .py_1()
                                    .bg(gpui::rgb(0x18181b))
                                    .border_1()
                                    .border_color(gpui::rgb(0x3f3f46))
                                    .rounded_md()
                                    .child(if password.is_empty() {
                                        div()
                                    } else {
                                        div().child("••••••••")
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                Button::new("register")
                                    .label("Register")
                                    .on_click(cx.listener(|this, _, _w, cx| {
                                        this.do_register(cx);
                                    })),
                            )
                            .child(
                                Button::new("login")
                                    .label("Login")
                                    .primary()
                                    .on_click(cx.listener(|this, _, _w, cx| {
                                        this.do_login(cx);
                                    })),
                            ),
                    )
                    .child(if status.is_empty() {
                        div()
                    } else {
                        div()
                            .px_2()
                            .py_1()
                            .text_sm()
                            .child(status.clone())
                    }),
            )
    }

    fn render_vault(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Error dialog gating.
        if self.vault.error_message.is_some() {
            let msg = self.vault.error_message.clone().unwrap_or_default();
            let weak = cx.weak_entity();
            let weak2 = cx.weak_entity();
            return div()
                .id("vault-manager")
                .child(
                    Dialog::new(window, cx)
                        .title("Vault Error")
                        .confirm()
                        .child(div().child(msg))
                        .on_ok(move |_, _w, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.vault.dismiss_error();
                                cx.notify();
                            });
                            true
                        })
                        .on_cancel(move |_, _w, cx| {
                            let _ = weak2.update(cx, |this, cx| {
                                this.vault.dismiss_error();
                                cx.notify();
                            });
                            true
                        }),
                )
                .into_any_element();
        }

        // Add-item dialog gating.
        if self.show_add_dialog {
            let title = self.add_title.clone();
            let uname = self.add_username.clone();
            let pwd = self.add_password.clone();
            let url = self.add_url.clone();
            let weak = cx.weak_entity();
            let weak2 = cx.weak_entity();
            return div()
                .id("vault-manager")
                .child(
                    Dialog::new(window, cx)
                        .title("Add Item")
                        .confirm()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(div().child("Title: ").child(title))
                                .child(div().child("Username: ").child(uname))
                                .child(div().child("Password: ").child(if pwd.is_empty() { div() } else { div().child("••••") }))
                                .child(div().child("URL: ").child(url)),
                        )
                        .on_ok(move |_, _w, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.do_add_item(cx);
                            });
                            true
                        })
                        .on_cancel(move |_, _w, cx| {
                            let _ = weak2.update(cx, |this, cx| {
                                this.show_add_dialog = false;
                                cx.notify();
                            });
                            true
                        }),
                )
                .into_any_element();
        }

        // ── Vault list ────────────────────────────────────────────────
        let lock_btn = Button::new("lock")
            .label("Lock")
            .danger()
            .on_click(cx.listener(|this, _, _w, cx| {
                this.do_lock(cx);
            }));

        let sync_btn = Button::new("sync")
            .label("Sync")
            .on_click(cx.listener(|this, _, _w, cx| {
                this.do_sync(cx);
            }));

        let add_btn = Button::new("add")
            .label("+ Add")
            .primary()
            .on_click(cx.listener(|this, _, _w, cx| {
                this.show_add_dialog = true;
                cx.notify();
            }));

        let reveal_btn = Button::new("reveal")
            .label("Reveal")
            .on_click(cx.listener(|this, _, _w, cx| {
                this.do_reveal(cx);
            }));

        let delete_btn = Button::new("delete")
            .label("Delete")
            .danger()
            .on_click(cx.listener(|this, _, _w, cx| {
                this.do_delete(cx);
            }));

        let mut rows = Vec::new();
        for (index, item) in self.vault.items.iter().enumerate() {
            let selected = self.vault.selected_index == Some(index);
            let title = item.title.clone();
            let row = div()
                .id(gpui::ElementId::named_usize("vault-row", index))
                .flex()
                .px_3()
                .py_2()
                .when(selected, |row| row.bg(gpui::rgb(0x27272a)))
                .child(title)
                .on_click(cx.listener(move |this, _, _w, cx| {
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
                    .bg(gpui::rgb(0x18181b))
                    .rounded_md()
                    .child("Secret: ")
                    .child(s.to_string())
            })
            .unwrap_or_else(|| div());

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
                    .child(div().text_lg().child("Vautr"))
                    .child(
                        div().flex().gap_2().child(sync_btn).child(lock_btn),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(add_btn)
                    .child(reveal_btn)
                    .child(delete_btn),
            )
            .child(div().id("items").flex().flex_col().gap_1().children(rows))
            .child(revealed_text)
            .into_any_element()
    }
}

impl Focusable for DesktopView {
    fn focus_handle(&self, _app: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}
