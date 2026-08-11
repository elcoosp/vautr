//! The top-level desktop view. Renders either the login screen (when the
//! vault is not yet unlocked) or the post-login app shell (after unlock).
//!
//! The app shell has two sections: **Vault** (local item list + reveal, the
//! sole holder of `read_secret`) and **Projects** (server-backed Projects,
//! roles, members, and Secrets UI). Uses gpui-component widgets throughout:
//! Button, Input/InputState, h_flex/v_flex.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::{
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    h_flex, v_flex, Icon, IconName,
};
use rand::RngCore;
use std::sync::Arc;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::api_client::{self, ApiClient};
use crate::app::base_url;
use crate::auth_client::AuthClient;
use crate::project_state::{DetailTab, ProjectsState};
use crate::state::{self, VaultConfig, VaultManagerState};
use crate::theme;
use vautr_app_state::VautrClient;
use vautr_crypto::{aead, kdf, key_tree};
use vautr_domain::{DecryptedOverview, DecryptedSecret, DomainModel, ItemMetadata};

/// Which post-login section is active.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Section {
    Dashboard,
    Projects,
    Vault,
    Generator,
    Secrets,
    MachineAccounts,
    Tokens,
    Mfa,
    ImportExport,
    Settings,
}

/// Which login form mode is active (mirrors the web UnlockScreen).
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoginMode {
    Login,
    Register,
}

/// Available access scopes for machine accounts and API tokens, mirroring
/// the web's `AccessScope` list.
const SCOPES: [&str; 7] = [
    "secrets:read",
    "secrets:write",
    "secrets:reveal",
    "projects:read",
    "projects:write",
    "tokens:manage",
    "machine_accounts:manage",
];

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
    login_mode: LoginMode,
    server_url: String,

    // ── Vault state ─────────────────────────────────────────────────────
    pub vault: VaultManagerState,
    client: Option<Arc<VautrClient>>,
    dek: Option<Zeroizing<[u8; 32]>>,

    // ── Revealed secret (vault items) ───────────────────────────────────
    revealed: Option<Zeroizing<String>>,
    active_handle: Option<vautr_app_state::orchestrator::SecretHandle>,
    /// Whether the inline "Add item" form is shown in the Vault section.
    vault_adding: bool,

    // ── Projects / Secrets state ────────────────────────────────────────
    /// The bearer session token from OPAQUE login, used for the Projects/Secrets API.
    token: Option<String>,
    projects: ProjectsState,
    section: Section,
    /// UUID of the secret whose value is being revealed.
    reveal_target_uuid: Option<String>,

    // ── Projects form inputs ────────────────────────────────────────────
    project_name_input: Entity<InputState>,
    project_desc_input: Entity<InputState>,
    member_user_input: Entity<InputState>,
    secret_key_input: Entity<InputState>,
    secret_value_input: Entity<InputState>,
    offboard_input: Entity<InputState>,

    // ── Generator section (canonical screen, ui-logic equivalent) ───────
    generator_password: String,

    // ── MFA section (canonical screen) ──────────────────────────────────
    mfa_status: Option<api_client::MfaStatusDto>,
    mfa_enrolled: Option<api_client::TotpIssueDto>,
    mfa_code_input: Entity<InputState>,
    mfa_text: String,

    // ── Settings section (canonical screen) ─────────────────────────────
    machines: Vec<api_client::MachineAccountDto>,
    tokens: Vec<api_client::AccessTokenDto>,
    settings_name_input: Entity<InputState>,
    settings_scopes: Vec<String>,
    settings_text: String,

    // ── Dashboard section ───────────────────────────────────────────────
    backup: Option<api_client::BackupStatusDto>,
    dashboard_loading: bool,

    // ── Secrets overview section ────────────────────────────────────────
    secrets_rows: Vec<(api_client::ProjectDto, api_client::SecretDto)>,
    secrets_loading: bool,
    secrets_error: Option<String>,
    secrets_revealed: std::collections::HashMap<String, String>,
    /// The secret currently being revealed (uuid).
    secrets_reveal_target: Option<String>,

    // ── Tokens section ──────────────────────────────────────────────────
    /// The one-time raw token value returned on creation.
    issued_token: Option<String>,

    // ── Import / export section ─────────────────────────────────────────
    include_secrets: bool,
    import_text: String,
    import_busy: bool,
    import_archive_input: Entity<InputState>,
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
                .placeholder("you@example.com")
        });
        let password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("••••••••")
        });

        // Pre-fill the stored username.
        if !stored_username.is_empty() {
            username_input.update(cx, |state, cx| {
                state.set_value(stored_username.as_str(), window, cx);
            });
        }

        let project_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Project name"));
        let project_desc_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Description (optional)")
        });
        let member_user_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("User UUID or email")
        });
        let secret_key_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Secret key, e.g. DATABASE_URL")
        });
        let secret_value_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Secret value")
        });
        let offboard_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("User UUID to revoke all access")
        });
        let mfa_code_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("000000")
        });
        let settings_name_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Machine account name, e.g. ci-deploy")
        });
        let import_archive_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Paste base64 archive here…")
        });

        let _subscriptions = vec![];

        Self {
            focus_handle: cx.focus_handle(),
            username_input,
            password_input,
            _subscriptions,
            login_status: String::new(),
            login_mode: LoginMode::Login,
            server_url: base_url(),
            vault: VaultManagerState::new(),
            client: None,
            dek: None,
            revealed: None,
            active_handle: None,
            vault_adding: false,
            token: None,
            projects: ProjectsState::new(),
            section: Section::Vault,
            reveal_target_uuid: None,
            project_name_input,
            project_desc_input,
            member_user_input,
            secret_key_input,
            secret_value_input,
            offboard_input,
            generator_password: generate_password(20),
            mfa_status: None,
            mfa_enrolled: None,
            mfa_code_input,
            mfa_text: String::new(),
            machines: Vec::new(),
            tokens: Vec::new(),
            settings_name_input,
            settings_scopes: vec!["secrets:read".into()],
            settings_text: String::new(),
            backup: None,
            dashboard_loading: false,
            secrets_rows: Vec::new(),
            secrets_loading: false,
            secrets_error: None,
            secrets_revealed: std::collections::HashMap::new(),
            secrets_reveal_target: None,
            issued_token: None,
            include_secrets: true,
            import_text: String::new(),
            import_busy: false,
            import_archive_input,
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

    fn api(&self) -> ApiClient {
        ApiClient::new(&self.server_url)
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
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
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
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
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
                this.token = Some(login.session_token);
                this.vault.set_items(items);
                this.section = Section::Vault;
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
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
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
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let outcome = client.delete_item(uuid).await;
            this.update(cx, |this, cx| match outcome {
                vautr_app_state::worker::TaskOutcome::Committed(_) => {
                    this.vault.dismiss_error();
                    cx.notify();
                    let c = this.client.clone();
                    cx.spawn(async move |this, cx| {
                        let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
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
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = client.sync().await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        let c = this.client.clone();
                        cx.spawn(async move |this, cx| {
                            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
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

    /// Create a new vault item from the inline Add form: build a
    /// [`DomainModel`], encrypt the secret payload client-side with the DEK
    /// (zero-knowledge — the server never sees plaintext), then `save_item`.
    fn do_add_vault_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            self.vault.show_error("vault is locked");
            cx.notify();
            return;
        };
        let Some(dek) = self.dek.clone() else {
            self.vault.show_error("vault is locked; cannot encrypt item");
            cx.notify();
            return;
        };
        let title = self.secret_key_input.read(cx).value().to_string();
        let value = self.secret_value_input.read(cx).value().to_string();
        if title.trim().is_empty() {
            self.vault.show_error("Item title is required.");
            cx.notify();
            return;
        }

        // Clear the form fields now (the async path has no `Window` to do so).
        self.secret_key_input.update(cx, |s, cx| {
            s.set_value("", window, cx);
        });
        self.secret_value_input.update(cx, |s, cx| {
            s.set_value("", window, cx);
        });

        let uuid = Uuid::new_v4();
        let item = DomainModel {
            uuid,
            enc_key_gen: 1,
            overview: DecryptedOverview {
                uuid,
                title: title.trim().to_string(),
                subtitle: "secret".to_string(),
                icon_key: "key".to_string(),
                urls: Vec::new(),
                updated_at: 0,
            },
            secret: DecryptedSecret {
                password: Zeroizing::new(value.clone()),
                totp: None,
                notes: Zeroizing::new(String::new()),
                fields: Vec::new(),
            },
            metadata: ItemMetadata {
                created_at: 0,
                updated_at: 0,
                trashed: false,
            },
        };
        let plaintext = match serde_json::to_vec(&item.secret) {
            Ok(pt) => pt,
            Err(e) => {
                self.vault.show_error(format!("Encode failed: {e}"));
                cx.notify();
                return;
            }
        };
        let envelope = match aead::encrypt(&dek, &uuid, 1, &plaintext) {
            Ok(e) => e,
            Err(e) => {
                self.vault.show_error(format!("Encryption failed: {e}"));
                cx.notify();
                return;
            }
        };

        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let outcome = client.save_item(item, envelope).await;
            this.update(cx, |this, cx| match outcome {
                vautr_app_state::worker::TaskOutcome::Committed(_) => {
                    this.vault.dismiss_error();
                    this.vault_adding = false;
                    cx.notify();
                    let c = this.client.clone();
                    cx.spawn(async move |this, cx| {
                        let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
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
                    this.vault.show_error("Add item failed");
                    cx.notify();
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
        self.token = None;
        self.projects = ProjectsState::new();
        self.section = Section::Vault;
        self.reveal_target_uuid = None;
        cx.notify();
    }

    // ── Projects / Secrets actions ──────────────────────────────────────

    fn do_refresh_projects(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        self.projects.loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.list_projects(&token).await;
            this.update(cx, |this, cx| match result {
                Ok(projects) => {
                    this.projects.loading = false;
                    this.projects.set_projects(projects);
                    this.projects.dismiss_error();
                    this.projects.clear_detail();
                    cx.notify();
                    if this.projects.selected_project().is_some() {
                        this.load_project_detail(cx);
                    }
                }
                Err(e) => {
                    this.projects.loading = false;
                    this.projects.show_error(format!("Failed to load projects: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn load_project_detail(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(uuid) = self.projects.selected_project_uuid() else {
            return;
        };
        let api = self.api();
        self.projects.loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let members = api.list_members(&token, &uuid).await;
            let secrets = api.list_secrets(&token, &uuid).await;
            this.update(cx, |this, cx| {
                this.projects.loading = false;
                if let Ok(m) = members {
                    this.projects.set_members(m);
                }
                if let Ok(s) = secrets {
                    this.projects.set_secrets(s);
                }
                this.projects.dismiss_error();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn do_select_project(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.projects.select_project(index) {
            self.projects.clear_detail();
            self.reveal_target_uuid = None;
            cx.notify();
            self.load_project_detail(cx);
        }
    }

    fn do_create_project(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let name = self.project_name_input.read(cx).value().to_string();
        let desc = self.project_desc_input.read(cx).value().to_string();
        let kind = self.projects.new_kind.clone();
        if name.trim().is_empty() {
            self.projects.show_error("Project name is required.");
            cx.notify();
            return;
        }
        let api = self.api();
        self.projects.set_status("Creating project...");
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let desc_opt = if desc.trim().is_empty() {
                None
            } else {
                Some(desc.trim().to_string())
            };
            let result = api
                .create_project(&token, name.trim(), &kind, desc_opt.as_deref())
                .await;
            this.update(cx, |this, cx| match result {
                Ok(created) => {
                    this.projects
                        .set_status(format!("Created project '{}'.", created.name));
                    this.projects.dismiss_error();
                    cx.notify();
                    this.do_refresh_projects_to(cx);
                }
                Err(e) => {
                    this.projects.show_error(format!("Create failed: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Helper that refreshes the project list after a mutation.
    fn do_refresh_projects_to(&mut self, cx: &mut Context<Self>) {
        self.do_refresh_projects_impl(cx);
    }

    fn do_refresh_projects_impl(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.list_projects(&token).await;
            this.update(cx, |this, cx| match result {
                Ok(projects) => {
                    this.projects.set_projects(projects);
                    this.projects.clear_detail();
                    this.projects.dismiss_error();
                    cx.notify();
                    if this.projects.selected_project().is_some() {
                        this.load_project_detail(cx);
                    }
                }
                Err(e) => {
                    this.projects.show_error(format!("Failed to reload projects: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_delete_project(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(uuid) = self.projects.selected_project_uuid() else {
            self.projects.show_error("no project selected");
            cx.notify();
            return;
        };
        let name = self
            .projects
            .selected_project()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let api = self.api();
        self.projects.set_status(format!("Deleting project '{}'...", name));
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.delete_project(&token, &uuid).await;
            this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.projects.set_status(format!("Deleted project '{}'.", name));
                    this.projects.dismiss_error();
                    cx.notify();
                    this.do_refresh_projects_impl(cx);
                }
                Err(e) => {
                    this.projects.show_error(format!("Delete failed: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_add_member(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(project_uuid) = self.projects.selected_project_uuid() else {
            self.projects.show_error("no project selected");
            cx.notify();
            return;
        };
        let user_uuid = self.member_user_input.read(cx).value().to_string();
        let role = self.projects.member_role.clone();
        let permission = self.projects.member_permission.clone();
        if user_uuid.trim().is_empty() {
            self.projects.show_error("Member user UUID is required.");
            cx.notify();
            return;
        }
        let api = self.api();
        self.projects.set_status(format!("Adding member {user_uuid}..."));
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api
                .add_member(&token, &project_uuid, user_uuid.trim(), &role, &permission)
                .await;
            this.update(cx, |this, cx| match result {
                Ok(m) => {
                    let who = m.display_name.clone().unwrap_or_else(|| m.user_uuid.clone());
                    this.projects
                        .set_status(format!("Added {who} as {} ({})", m.role, m.permission));
                    this.projects.dismiss_error();
                    cx.notify();
                    this.reload_members(cx);
                }
                Err(e) => {
                    this.projects.show_error(format!("Add member failed: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn reload_members(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(uuid) = self.projects.selected_project_uuid() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.list_members(&token, &uuid).await;
            this.update(cx, |this, cx| {
                if let Ok(members) = result {
                    this.projects.set_members(members);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_update_member_permission(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        user_uuid: String,
        permission: String,
    ) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(project_uuid) = self.projects.selected_project_uuid() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api
                .update_member(&token, &project_uuid, &user_uuid, None, Some(&permission))
                .await;
            this.update(cx, |this, cx| match result {
                Ok(_) => {
                    this.projects
                        .set_status(format!("Updated permission to {permission}."));
                    this.projects.dismiss_error();
                    cx.notify();
                    this.reload_members(cx);
                }
                Err(e) => {
                    this.projects.show_error(format!("Update permission failed: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_remove_member(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        user_uuid: String,
    ) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(project_uuid) = self.projects.selected_project_uuid() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.remove_member(&token, &project_uuid, &user_uuid).await;
            this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.projects.set_status("Member removed.");
                    this.projects.dismiss_error();
                    cx.notify();
                    this.reload_members(cx);
                }
                Err(e) => {
                    this.projects.show_error(format!("Remove member failed: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_add_secret(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(project_uuid) = self.projects.selected_project_uuid() else {
            self.projects.show_error("no project selected");
            cx.notify();
            return;
        };
        let Some(dek) = self.dek.clone() else {
            self.projects.show_error("vault is locked; cannot encrypt secret");
            cx.notify();
            return;
        };
        let key = self.secret_key_input.read(cx).value().to_string();
        let value = self.secret_value_input.read(cx).value().to_string();
        if key.trim().is_empty() {
            self.projects.show_error("Secret key is required.");
            cx.notify();
            return;
        }
        let api = self.api();
        self.projects.set_status(format!("Creating secret '{key}'..."));
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            // Encrypt the value client-side (zero-knowledge): the server only
            // ever sees the AEAD ciphertext, bound to (project, key).
            let ad = api_client::secret_ad(&project_uuid, key.trim());
            let mut nonce = [0u8; aead::NONCE_LEN];
            rand::rngs::OsRng.fill_bytes(&mut nonce);
            let ciphertext =
                aead::encrypt_with_nonce(&dek, &nonce, &ad, value.trim().as_bytes());
            let ciphertext = match ciphertext {
                Ok(c) => c,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.projects.show_error(format!("Encryption failed: {e}"));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let value_b64 = api_client::b64_encode(&ciphertext);
            let result = api
                .create_secret(&token, &project_uuid, key.trim(), &value_b64)
                .await;
            this.update(cx, |this, cx| match result {
                Ok(secret) => {
                    this.projects
                        .set_status(format!("Created secret '{}' (v{}).", secret.key, secret.version));
                    this.projects.dismiss_error();
                    cx.notify();
                    this.reload_secrets(cx);
                }
                Err(e) => {
                    this.projects.show_error(format!("Create secret failed: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn reload_secrets(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(uuid) = self.projects.selected_project_uuid() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.list_secrets(&token, &uuid).await;
            this.update(cx, |this, cx| {
                if let Ok(secrets) = result {
                    this.projects.set_secrets(secrets);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_reveal_secret(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(project_uuid) = self.projects.selected_project_uuid() else {
            return;
        };
        let Some(dek) = self.dek.clone() else {
            self.projects.show_error("vault is locked; cannot decrypt secret");
            cx.notify();
            return;
        };
        let Some(uuid) = self.reveal_target_uuid.clone() else {
            self.projects.show_error("no secret selected");
            cx.notify();
            return;
        };
        let api = self.api();
        self.projects.set_status("Revealing secret...");
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.get_secret_value(&token, &uuid).await;
            this.update(cx, |this, cx| match result {
                Ok(value) => {
                    let ad = api_client::secret_ad(&project_uuid, &value.key);
                    let ct = match api_client::b64_decode(&value.value_ciphertext) {
                        Ok(c) => c,
                        Err(e) => {
                            this.projects.show_error(format!("Decode failed: {e}"));
                            cx.notify();
                            return;
                        }
                    };
                    match aead::decrypt_with_ad(&dek, &ad, &ct) {
                        Ok(plain) => match String::from_utf8(plain) {
                            Ok(s) => {
                                this.projects
                                    .reveal_secret(value.key.clone(), s);
                                this.projects.set_status("Secret revealed.");
                                this.projects.dismiss_error();
                                cx.notify();
                            }
                            Err(e) => {
                                this.projects.show_error(format!("Secret is not UTF-8: {e}"));
                                cx.notify();
                            }
                        },
                        Err(e) => {
                            this.projects.show_error(format!("Decryption failed: {e}"));
                            cx.notify();
                        }
                    }
                }
                Err(e) => {
                    this.projects.show_error(format!("Reveal failed: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_delete_secret(&mut self, _window: &mut Window, cx: &mut Context<Self>, uuid: String) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.delete_secret(&token, &uuid).await;
            this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.projects.set_status("Secret deleted.");
                    this.projects.dismiss_error();
                    cx.notify();
                    this.reload_secrets(cx);
                }
                Err(e) => {
                    this.projects.show_error(format!("Delete secret failed: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_offboard(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let user_uuid = self.offboard_input.read(cx).value().to_string();
        if user_uuid.trim().is_empty() {
            self.projects.show_error("User UUID is required to offboard.");
            cx.notify();
            return;
        }
        let api = self.api();
        self.projects.set_status("Revoking all access...");
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.offboard(&token, user_uuid.trim(), Some("desktop offboard")).await;
            this.update(cx, |this, cx| match result {
                Ok(o) => {
                    this.projects.offboard_result = Some(format!(
                        "Revoked {} projects, {} memberships, {} tokens.",
                        o.revoked_projects, o.revoked_memberships, o.revoked_tokens
                    ));
                    this.projects.set_status("Offboarding complete.");
                    this.projects.dismiss_error();
                    cx.notify();
                }
                Err(e) => {
                    this.projects.show_error(format!("Offboard failed: {e}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    // ── Generator (canonical screen) ────────────────────────────────────

    fn do_regenerate_generator(&mut self, cx: &mut Context<Self>) {
        self.generator_password = generate_password(20);
        cx.notify();
    }

    // ── MFA (canonical screen) ──────────────────────────────────────────

    fn do_refresh_mfa(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.mfa_status(&token).await;
            this.update(cx, |this, cx| match result {
                Ok(status) => {
                    this.mfa_status = Some(status);
                    this.mfa_text = String::new();
                    cx.notify();
                }
                Err(e) => {
                    this.mfa_text = format!("Failed to load MFA status: {e}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_enroll_totp(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        self.mfa_text = "Starting TOTP enrollment...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.mfa_totp_issue(&token).await;
            this.update(cx, |this, cx| match result {
                Ok(issued) => {
                    this.mfa_enrolled = Some(issued);
                    this.mfa_text = String::new();
                    cx.notify();
                }
                Err(e) => {
                    this.mfa_text = format!("Enrollment failed: {e}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_verify_totp(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let enrollment_id = self
            .mfa_enrolled
            .as_ref()
            .map(|e| e.enrollment_id.clone());
        let code = self.mfa_code_input.read(cx).value().to_string();
        if code.trim().len() < 6 {
            self.mfa_text = "Enter a valid 6-digit code.".into();
            cx.notify();
            return;
        }
        let api = self.api();
        self.mfa_text = "Verifying...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api
                .mfa_totp_verify(&token, enrollment_id.as_deref(), code.trim())
                .await;
            this.update(cx, |this, cx| match result {
                Ok(verified) => {
                    this.mfa_text = format!("TOTP verified: {}", verified.status);
                    this.mfa_enrolled = None;
                    cx.notify();
                    let api = this.api();
                    let token = this.token.clone();
                    if let Some(token) = token {
                        let api = api;
                        cx.spawn(async move |this, cx| {
                            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
                            if let Ok(status) = api.mfa_status(&token).await {
                                this.update(cx, |this, cx| {
                                    this.mfa_status = Some(status);
                                    cx.notify();
                                })
                                .ok();
                            }
                        })
                        .detach();
                    }
                }
                Err(e) => {
                    this.mfa_text = format!("Verification failed: {e}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    // ── Settings (canonical screen: machine accounts + tokens) ──────────

    fn do_refresh_settings(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let machines = api.list_machine_accounts(&token).await;
            let tokens = api.list_tokens(&token).await;
            this.update(cx, |this, cx| {
                if let Ok(m) = machines {
                    this.machines = m;
                }
                if let Ok(t) = tokens {
                    this.tokens = t;
                }
                this.settings_text = String::new();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn do_create_machine(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let name = self.settings_name_input.read(cx).value().to_string();
        if name.trim().is_empty() {
            self.settings_text = "Machine account name is required.".into();
            cx.notify();
            return;
        }
        let scopes: Vec<String> = self.settings_scopes.iter().cloned().collect();
        let api = self.api();
        self.settings_text = "Creating machine account...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let scope_refs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
            let result = api
                .create_machine_account(&token, name.trim(), None, None, &scope_refs)
                .await;
            this.update(cx, |this, cx| match result {
                Ok(created) => {
                    this.settings_text = format!("Created machine account '{}'.", created.name);
                    cx.notify();
                    this.do_refresh_settings_to(cx);
                }
                Err(e) => {
                    this.settings_text = format!("Create failed: {e}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_refresh_settings_to(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let machines = api.list_machine_accounts(&token).await;
            let tokens = api.list_tokens(&token).await;
            this.update(cx, |this, cx| {
                if let Ok(m) = machines {
                    this.machines = m;
                }
                if let Ok(t) = tokens {
                    this.tokens = t;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ── Dashboard ───────────────────────────────────────────────────────

    fn do_refresh_dashboard(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        self.dashboard_loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let projects = api.list_projects(&token).await;
            let machines = api.list_machine_accounts(&token).await;
            let tokens = api.list_tokens(&token).await;
            let mfa = api.mfa_status(&token).await;
            let backup = api.backup_status(&token).await;
            this.update(cx, |this, cx| {
                if let Ok(p) = projects {
                    this.projects.set_projects(p);
                }
                if let Ok(m) = machines {
                    this.machines = m;
                }
                if let Ok(t) = tokens {
                    this.tokens = t;
                }
                if let Ok(m) = mfa {
                    this.mfa_status = Some(m);
                }
                if let Ok(b) = backup {
                    this.backup = Some(b);
                }
                this.dashboard_loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ── Secrets overview ────────────────────────────────────────────────

    fn do_refresh_secrets(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        self.secrets_loading = true;
        self.secrets_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let projects = api.list_projects(&token).await;
            let projects = match projects {
                Ok(p) => p,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.secrets_loading = false;
                        this.secrets_error = Some(format!("Failed to load projects: {e}"));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let mut rows: Vec<(api_client::ProjectDto, api_client::SecretDto)> = Vec::new();
            let mut err: Option<String> = None;
            for p in &projects {
                match api.list_secrets(&token, &p.uuid).await {
                    Ok(secrets) => {
                        for s in secrets {
                            rows.push((p.clone(), s));
                        }
                    }
                    Err(e) => {
                        err = Some(format!("Failed to load secrets for '{}': {e}", p.name));
                    }
                }
            }
            this.update(cx, |this, cx| {
                this.secrets_rows = rows;
                this.secrets_loading = false;
                this.secrets_error = err;
                this.secrets_revealed.clear();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn do_reveal_overview_secret(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        uuid: String,
    ) {
        // Toggle: hide an already-revealed value.
        if self.secrets_revealed.contains_key(&uuid) {
            self.secrets_revealed.remove(&uuid);
            cx.notify();
            return;
        }
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(dek) = self.dek.clone() else {
            self.secrets_error = Some("vault is locked; cannot decrypt secret".into());
            cx.notify();
            return;
        };
        let Some((project, _)) = self
            .secrets_rows
            .iter()
            .find(|(_, s)| s.uuid == uuid)
            .cloned()
        else {
            return;
        };
        let api = self.api();
        self.secrets_reveal_target = Some(uuid.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.get_secret_value(&token, &uuid).await;
            this.update(cx, |this, cx| {
                this.secrets_reveal_target = None;
                match result {
                    Ok(value) => {
                        let ad = api_client::secret_ad(&project.uuid, &value.key);
                        let plain = api_client::b64_decode(&value.value_ciphertext)
                            .and_then(|ct| {
                                aead::decrypt_with_ad(&dek, &ad, &ct)
                                    .map_err(|e| e.to_string())
                            });
                        match plain {
                            Ok(plain) => match String::from_utf8(plain) {
                                Ok(s) => {
                                    this.secrets_revealed.insert(uuid.clone(), s);
                                    this.secrets_error = None;
                                }
                                Err(e) => {
                                    this.secrets_error =
                                        Some(format!("Secret is not UTF-8: {e}"));
                                }
                            },
                            Err(e) => {
                                this.secrets_error = Some(format!("Decryption failed: {e}"));
                            }
                        }
                    }
                    Err(e) => {
                        this.secrets_error = Some(format!("Reveal failed: {e}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ── Machine account mutations (dedicated section) ───────────────────

    fn do_toggle_scope(&mut self, scope: String, cx: &mut Context<Self>) {
        if let Some(i) = self.settings_scopes.iter().position(|s| *s == scope) {
            self.settings_scopes.remove(i);
        } else {
            self.settings_scopes.push(scope);
        }
        cx.notify();
    }

    fn do_toggle_machine(&mut self, _window: &mut Window, cx: &mut Context<Self>, uuid: String) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let status = self
            .machines
            .iter()
            .find(|m| m.uuid == uuid)
            .map(|m| {
                if m.status == "active" {
                    "disabled".to_string()
                } else {
                    "active".to_string()
                }
            })
            .unwrap_or_else(|| "active".into());
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.update_machine_account_status(&token, &uuid, &status).await;
            this.update(cx, |this, cx| match result {
                Ok(_) => {
                    this.settings_text = format!("Machine account {status}.");
                    this.do_refresh_settings_to(cx);
                }
                Err(e) => {
                    this.settings_text = format!("Update failed: {e}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_delete_machine(&mut self, _window: &mut Window, cx: &mut Context<Self>, uuid: String) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.delete_machine_account(&token, &uuid).await;
            this.update(cx, |this, cx| match result {
                Ok(_) => {
                    this.settings_text = "Machine account deleted.".into();
                    this.do_refresh_settings_to(cx);
                }
                Err(e) => {
                    this.settings_text = format!("Delete failed: {e}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    // ── Access token mutations (dedicated section) ──────────────────────

    fn do_create_token(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let name = self.settings_name_input.read(cx).value().to_string();
        if name.trim().is_empty() || self.settings_scopes.is_empty() {
            self.settings_text = "Token name and at least one scope are required.".into();
            cx.notify();
            return;
        }
        let scopes: Vec<String> = self.settings_scopes.iter().cloned().collect();
        let api = self.api();
        self.settings_name_input.update(cx, |st, cx| {
            st.set_value("", window, cx);
        });
        self.settings_text = "Creating token...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let scope_refs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
            let result = api.create_token(&token, name.trim(), &scope_refs).await;
            this.update(cx, |this, cx| match result {
                Ok(created) => {
                    this.issued_token = Some(created.token);
                    this.settings_text =
                        format!("Created token '{}'. Save the raw value now.", created.token_id);
                    this.do_refresh_settings_to(cx);
                }
                Err(e) => {
                    this.settings_text = format!("Create failed: {e}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_revoke_token(&mut self, _window: &mut Window, cx: &mut Context<Self>, uuid: String) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.revoke_token(&token, &uuid).await;
            this.update(cx, |this, cx| match result {
                Ok(_) => {
                    this.settings_text = "Token revoked.".into();
                    this.do_refresh_settings_to(cx);
                }
                Err(e) => {
                    this.settings_text = format!("Revoke failed: {e}");
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    // ── Import / export (backup) ────────────────────────────────────────

    fn do_refresh_backup(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let api = self.api();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.backup_status(&token).await;
            this.update(cx, |this, cx| {
                if let Ok(b) = result {
                    this.backup = Some(b);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn do_export_backup(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let include = self.include_secrets;
        let api = self.api();
        self.import_busy = true;
        self.import_text = "Exporting backup...".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.backup_export(&token, include).await;
            this.update(cx, |this, cx| {
                this.import_busy = false;
                match result {
                    Ok(exp) => {
                        this.import_text = format!(
                            "Backup created (id {}, {} bytes).",
                            exp.backup_id, exp.size_bytes
                        );
                        this.do_refresh_backup(cx);
                    }
                    Err(e) => {
                        this.import_text = format!("Export failed: {e}");
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_restore_backup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(token) = self.token.clone() else {
            return;
        };
        let archive = self.import_archive_input.read(cx).value().to_string();
        if archive.trim().is_empty() {
            self.import_text = "Paste a base64 backup archive to restore.".into();
            cx.notify();
            return;
        }
        let api = self.api();
        self.import_busy = true;
        self.import_text = "Restoring...".into();
        self.import_archive_input.update(cx, |st, cx| {
            st.set_value("", window, cx);
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _rt = crate::runtime::enter(); // tokio reactor for reqwest in this block
            let result = api.backup_restore(&token, archive.trim()).await;
            this.update(cx, |this, cx| {
                this.import_busy = false;
                match result {
                    Ok(res) => {
                        this.import_text = format!(
                            "Restore {}: {} records.",
                            res.status, res.restored_records
                        );
                        cx.notify();
                    }
                    Err(e) => {
                        this.import_text = format!("Restore failed: {e}");
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn do_toggle_include_secrets(&mut self, cx: &mut Context<Self>) {
        self.include_secrets = !self.include_secrets;
        cx.notify();
    }
}

impl Render for DesktopView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.is_unlocked() {
            self.render_app(cx).into_any_element()
        } else {
            self.render_login(cx).into_any_element()
        }
    }
}

impl DesktopView {
    fn render_login(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mode = self.login_mode;
        let status = self.login_status.clone();
        let has_error = !status.is_empty()
            && (status.contains("failed")
                || status.contains("required")
                || status.contains("No local")
                || status.contains("Invalid")
                || status.contains("error"));
        let busy = status.contains("...")
            || status.contains("Logging")
            || status.contains("Registering")
            || status.contains("Unlocking")
            || status.contains("Creating");

        let subtitle = match mode {
            LoginMode::Login => "Unlock your vault to view saved items.",
            LoginMode::Register => "Create a new zero-knowledge vault.",
        };
        let submit_label = match (mode, busy) {
            (LoginMode::Login, true) => "Unlocking…",
            (LoginMode::Register, true) => "Creating…",
            (LoginMode::Login, false) => "Unlock vault",
            (LoginMode::Register, false) => "Create vault",
        };
        let submit_caption = match (mode, busy) {
            (LoginMode::Login, _) => "Register an account",
            (LoginMode::Register, _) => "Log in",
        };

        // Mirrors the web UnlockScreen: a centered card with a segmented
        // Log in / Register toggle, labeled fields, and a full-width accent CTA.
        let login_tab = self.render_login_tab(cx, LoginMode::Login, "Log in");
        let register_tab = self.render_login_tab(cx, LoginMode::Register, "Register");

        let username = Input::new(&self.username_input).w_full();
        let password = Input::new(&self.password_input).w_full();

        div()
            .size_full()
            .bg(theme::BG)
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .child(
                div()
                    .w(px(384.))
                    .max_w_full()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::BORDER)
                    .bg(theme::SURFACE)
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::TEXT)
                            .child("Vautr"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme::TEXT_MUTED)
                            .child(subtitle),
                    )
                    // Segmented Log in / Register toggle.
                    .child(
                        h_flex()
                            .gap_1()
                            .rounded_md()
                            .bg(theme::SURFACE_RAISED)
                            .p_1()
                            .child(login_tab)
                            .child(register_tab),
                    )
                    // Form fields.
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::TEXT)
                                    .child("Username"),
                            )
                            .child(username)
                            .mt_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::TEXT)
                                    .child("Master password"),
                            )
                            .child(password),
                    )
                    // Status / error.
                    .child(
                        div()
                            .when(!status.is_empty(), |this| {
                                this.text_sm()
                                    .when(has_error, |this| {
                                        this.text_color(theme::DANGER)
                                    })
                                    .when(!has_error, |this| {
                                        this.text_color(theme::TEXT_MUTED)
                                    })
                                    .child(status)
                            }),
                    )
                    // Full-width primary CTA.
                    .child(
                        Button::new("login-submit")
                            .primary()
                            .w_full()
                            .label(submit_label)
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, window, cx| {
                                    match this.login_mode {
                                        LoginMode::Login => this.do_login(window, cx),
                                        LoginMode::Register => this.do_register(window, cx),
                                    }
                                },
                            )),
                    )
                    // Toggle link at the bottom.
                    .child(
                        div()
                            .mt_1()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(theme::TEXT_MUTED)
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .justify_center()
                                    .child("New here?")
                                    .child(
                                        div()
                                            .id("login-toggle")
                                            .text_color(theme::ACCENT)
                                            .underline()
                                            .cursor_pointer()
                                            .child(submit_caption)
                                            .on_click(cx.listener(
                                                |this, _: &gpui::ClickEvent, _window, cx| {
                                                    this.login_mode = match this.login_mode {
                                                        LoginMode::Login => LoginMode::Register,
                                                        LoginMode::Register => LoginMode::Login,
                                                    };
                                                    this.login_status.clear();
                                                    cx.notify();
                                                },
                                            )),
                                    ),
                            ),
                    ),
            )
    }

    /// A single segment of the Log in / Register segmented toggle.
    fn render_login_tab(
        &mut self,
        cx: &mut Context<Self>,
        which: LoginMode,
        label: &'static str,
    ) -> AnyElement {
        let active = self.login_mode == which;
        let id = match which {
            LoginMode::Login => "login-tab",
            LoginMode::Register => "register-tab",
        };
        div()
            .id(SharedString::from(id))
            .flex_1()
            .rounded_md()
            .px_3()
            .h_8()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .items_center()
            .justify_center()
            .when(active, |d| d.bg(theme::ACCENT).text_color(theme::ACCENT_INK))
            .when(!active, |d| d.text_color(theme::TEXT_MUTED))
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.login_mode = which;
                this.login_status.clear();
                cx.notify();
            }))
            .into_any_element()
    }

    /// The post-login shell: a web-style left sidebar + active section content.
    fn render_app(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let section = self.section;
        let content = match section {
            Section::Dashboard => self.render_dashboard(cx).into_any_element(),
            Section::Projects => self.render_projects(cx).into_any_element(),
            Section::Vault => self.render_vault_content(cx).into_any_element(),
            Section::Generator => self.render_generator(cx).into_any_element(),
            Section::Secrets => self.render_secrets_overview(cx).into_any_element(),
            Section::MachineAccounts => self.render_machine_accounts(cx).into_any_element(),
            Section::Tokens => self.render_tokens(cx).into_any_element(),
            Section::Mfa => self.render_mfa(cx).into_any_element(),
            Section::ImportExport => self.render_import_export(cx).into_any_element(),
            Section::Settings => self.render_settings(cx).into_any_element(),
        };

        h_flex()
            .size_full()
            .bg(theme::BG)
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(content),
            )
    }

    /// The left navigation sidebar, mirroring the web `_authed` layout: a
    /// brand header, an icon + label nav list, and a Log out row at the bottom.
    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let section = self.section;

        let items: [(Section, &'static str, IconName); 10] = [
            (Section::Dashboard, "Dashboard", IconName::LayoutDashboard),
            (Section::Projects, "Projects", IconName::Folder),
            (Section::Vault, "Vault", IconName::Eye),
            (Section::Generator, "Generator", IconName::Settings2),
            (Section::Secrets, "Secrets", IconName::HardDrive),
            (Section::MachineAccounts, "Machine accounts", IconName::Bot),
            (Section::Tokens, "Tokens", IconName::Globe),
            (Section::Mfa, "MFA & security", IconName::CircleCheck),
            (Section::ImportExport, "Import / export", IconName::Replace),
            (Section::Settings, "Settings", IconName::Settings),
        ];

        let mut nav_rows: Vec<AnyElement> = Vec::new();
        for (sec, label, icon) in items {
            let active = section == sec;
            let ink = if active { theme::TEXT } else { theme::TEXT_MUTED };
            let row = div()
                .id(SharedString::from(format!("nav-{label}")))
                .flex()
                .items_center()
                .gap(px(10.))
                .px_3()
                .py_2()
                .rounded_md()
                .when(active, |d| d.bg(theme::SURFACE_RAISED))
                .text_color(ink)
                .cursor_pointer()
                .child(Icon::new(icon).size_4().text_color(ink))
                .child(div().text_sm().child(label))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.activate_section(sec, window, cx);
                }));
            nav_rows.push(row.into_any_element());
        }

        v_flex()
            .w_64()
            .flex_shrink_0()
            .h_full()
            .bg(theme::SURFACE)
            .border_r_1()
            .border_color(theme::BORDER)
            // Brand header.
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_4()
                    .border_b_1()
                    .border_color(theme::BORDER)
                    .items_center()
                    .child(
                        div()
                            .size_8()
                            .rounded_lg()
                            .bg(theme::ACCENT_DIM)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::Eye).size_4().text_color(theme::ACCENT)),
                    )
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::TEXT)
                            .child("Vautr"),
                    ),
            )
            // Nav list.
            .child(
                v_flex()
                    .id("sidebar-nav")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_2()
                    .py_2()
                    .gap_1()
                    .children(nav_rows),
            )
            // Log out.
            .child(
                v_flex()
                    .border_t_1()
                    .border_color(theme::BORDER)
                    .p_2()
                    .child(
                        div()
                            .id("nav-logout")
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_color(theme::TEXT_MUTED)
                            .cursor_pointer()
                            .child(div().text_sm().child("Log out"))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.do_lock(cx);
                            })),
                    ),
            )
    }

    /// Switch to a nav section, triggering the relevant data refresh.
    fn activate_section(&mut self, sec: Section, window: &mut Window, cx: &mut Context<Self>) {
        self.section = sec;
        match sec {
            Section::Vault | Section::Generator => cx.notify(),
            Section::Dashboard => self.do_refresh_dashboard(window, cx),
            Section::Projects => self.do_refresh_projects(window, cx),
            Section::Secrets => self.do_refresh_secrets(window, cx),
            Section::MachineAccounts | Section::Tokens => self.do_refresh_settings(window, cx),
            Section::Mfa => self.do_refresh_mfa(window, cx),
            Section::ImportExport => self.do_refresh_backup(cx),
            Section::Settings => self.do_refresh_settings(window, cx),
        }
    }

    // ── Web-style page primitives ───────────────────────────────────────
    // The web app lays every authed page out as `p-6 space-y-6`: a heading
    // (title + muted subtitle) followed by bordered rounded cards. These
    // helpers reproduce that structure so the desktop reads as the web.

    /// Padded, vertically-scrolling page column (`p-6 space-y-6`).
    fn page(&self) -> Stateful<Div> {
        v_flex()
            .id("section-page")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .gap_6()
    }

    /// Web-style page heading: `text-2xl` title + `text-sm` muted subtitle.
    fn page_header(&self, title: &str, subtitle: &str) -> Div {
        v_flex()
            .gap_1()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::TEXT)
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme::TEXT_MUTED)
                    .child(subtitle.to_string()),
            )
    }

    /// Web-style card: bordered, rounded, surface background, with a title +
    /// description header block. Callers append card body children.
    fn card(&self, title: &str, description: &str) -> Div {
        v_flex()
            .w_full()
            .border_1()
            .border_color(theme::BORDER)
            .rounded_lg()
            .bg(theme::SURFACE)
            .p_5()
            .gap_4()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::TEXT)
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme::TEXT_MUTED)
                            .child(description.to_string()),
                    ),
            )
    }

    // ── Vault section ────────────────────────────────────────────────────

    fn render_vault_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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
                .when(selected, |row| row.bg(theme::BORDER))
                .cursor_pointer()
                .child(div().text_sm().child(title))
                .child(div().text_xs().text_color(theme::TEXT_DIM).child(subtitle))
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

        let error = self.vault.error_message.clone().unwrap_or_default();
        let has_error = !error.is_empty();

        let mut page = self
            .page()
            .child(self.page_header(
                "Vault",
                "Your encrypted secrets, unlocked locally.",
            ))
            .child(
                self.card("Items", "Select an item to view or reveal its secret.")
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("add-btn")
                                    .primary()
                                    .label("Add item")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, _window, cx| {
                                            this.vault_adding = !this.vault_adding;
                                            this.vault.dismiss_error();
                                            cx.notify();
                                        },
                                    )),
                            )
                            .child(
                                Button::new("sync-btn")
                                    .label("Sync")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_sync(window, cx);
                                        },
                                    )),
                            )
                            .child(
                                Button::new("reveal-btn")
                                    .label("Reveal")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_reveal(window, cx);
                                        },
                                    )),
                            )
                            .child(
                                Button::new("delete-btn")
                                    .danger()
                                    .label("Delete")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_delete(window, cx);
                                        },
                                    )),
                            ),
                    )
                    .when(has_error, |this| {
                        this.child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(theme::DANGER_BG)
                                .text_color(theme::DANGER_TEXT)
                                .text_sm()
                                .child(error),
                        )
                    })
                    .when(rows.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(theme::TEXT_MUTED)
                                .child("No vault items yet. Add one below."),
                        )
                    })
                    .children(rows),
            );

        if self.vault_adding {
            page = page.child(
                self.card("Add item", "Save a new username/password entry to your vault.")
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::TEXT)
                                    .child("Title"),
                            )
                            .child(Input::new(&self.secret_key_input).w_full())
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::TEXT)
                                    .child("Password / value"),
                            )
                            .child(Input::new(&self.secret_value_input).w_full()),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("vault-save-btn")
                                    .primary()
                                    .label("Save item")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_add_vault_item(window, cx);
                                        },
                                    )),
                            )
                            .child(
                                Button::new("vault-cancel-btn")
                                    .label("Cancel")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, _window, cx| {
                                            this.vault_adding = false;
                                            cx.notify();
                                        },
                                    )),
                            ),
                    ),
            );
        }

        if let Some(s) = self.revealed.as_deref() {
            page = page.child(
                self.card("Revealed secret", "Plaintext for the selected item.")
                    .child(
                        div()
                            .text_sm()
                            .font_family("ui-monospace")
                            .text_color(theme::TEXT)
                            .child(s.to_string()),
                    ),
            );
        }

        page
    }

    // ── Projects section ─────────────────────────────────────────────────

    fn render_projects(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let error = self.projects.error_message.clone().unwrap_or_default();
        let status = self.projects.status.clone();
        let has_error = !error.is_empty();

        let mut project_rows = Vec::new();
        for (i, p) in self.projects.projects.iter().enumerate() {
            let selected = self.projects.selected_index == Some(i);
            let name = p.name.clone();
            let kind = p.kind.clone();
            let role = p.role.clone();
            let perm = p.permission.clone().unwrap_or_else(|| "—".into());
            let meta = format!("{kind} · {role} · {perm}");
            let row = div()
                .id(SharedString::from(format!("project-row-{i}")))
                .flex()
                .flex_col()
                .px_3()
                .py_2()
                .rounded_md()
                .when(selected, |row| row.bg(theme::BORDER))
                .cursor_pointer()
                .child(div().text_sm().font_weight(FontWeight::BOLD).child(name))
                .child(div().text_xs().text_color(theme::TEXT_DIM).child(meta))
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.do_select_project(i, cx);
                }));
            project_rows.push(row);
        }

        // Project list panel (left).
        let list_panel = v_flex()
            .w_72()
            .border_r_1()
            .border_color(theme::BORDER)
            .p_3()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Projects"))
                    .child(
                        Button::new("projects-refresh")
                            .compact()
                            .label("Refresh")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, window, cx| {
                                    this.do_refresh_projects(window, cx);
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .id("project-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(project_rows),
            )
            .child(div().border_t_1().border_color(theme::BORDER))
            .child(div().text_xs().text_color(theme::TEXT_MUTED).child("New project"))
            .child(Input::new(&self.project_name_input).w_full())
            .child(Input::new(&self.project_desc_input).w_full())
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("kind-personal")
                            .when(self.projects.new_kind == "personal", |b| b.primary())
                            .compact()
                            .label("Personal")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.new_kind = "personal".into();
                                    cx.notify();
                                },
                            )),
                    )
                    .child(
                        Button::new("kind-shared")
                            .when(self.projects.new_kind == "shared", |b| b.primary())
                            .compact()
                            .label("Shared")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.new_kind = "shared".into();
                                    cx.notify();
                                },
                            )),
                    ),
            )
            .child(
                Button::new("create-project-btn")
                    .primary()
                    .label("Create project")
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                        this.do_create_project(window, cx);
                    })),
            )
            .child(
                Button::new("delete-project-btn")
                    .danger()
                    .label("Delete selected project")
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                        this.do_delete_project(window, cx);
                    })),
            );

        // Detail panel (right): header + tabs + content.
        let (proj_name, proj_type) = self
            .projects
            .selected_project()
            .map(|p| (p.name.clone(), p.kind.clone()))
            .unwrap_or_else(|| ("No project selected".into(), String::new()));

        let detail = v_flex()
            .flex_1()
            .p_3()
            .gap_2()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(proj_name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::TEXT_DIM)
                            .child(if proj_type.is_empty() {
                                "Select a project to see its members and secrets.".into()
                            } else {
                                format!("Type: {proj_type}")
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("tab-members")
                            .when(self.projects.detail_tab == DetailTab::Members, |b| b.primary())
                            .compact()
                            .label("Members")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.detail_tab = DetailTab::Members;
                                    cx.notify();
                                },
                            )),
                    )
                    .child(
                        Button::new("tab-secrets")
                            .when(self.projects.detail_tab == DetailTab::Secrets, |b| b.primary())
                            .compact()
                            .label("Secrets")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.detail_tab = DetailTab::Secrets;
                                    cx.notify();
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .id("projects-detail")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(match self.projects.detail_tab {
                        DetailTab::Members => self.render_members(cx).into_any_element(),
                        DetailTab::Secrets => self.render_secrets(cx).into_any_element(),
                    }),
            )
            .child(self.render_offboard(cx));

        v_flex()
            .size_full()
            .p_6()
            .gap_4()
            .child(self.page_header(
                "Projects",
                "Create and manage shared vaults with members and secrets.",
            ))
            .child(
                div()
                    .when(has_error, |this| {
                        this.child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(theme::DANGER_BG)
                                .text_color(theme::DANGER_TEXT)
                                .text_sm()
                                .child(error),
                        )
                    }),
            )
            .child(
                div()
                    .when(!status.is_empty(), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(theme::TEXT_MUTED)
                                .child(status),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .border_1()
                    .border_color(theme::BORDER)
                    .rounded_lg()
                    .bg(theme::SURFACE)
                    .overflow_x_hidden()
                    .child(list_panel)
                    .child(detail),
            )
    }

    fn render_members(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut rows = Vec::new();
        for (i, m) in self.projects.members.iter().enumerate() {
            let user_uuid = m.user_uuid.clone();
            let display = m
                .display_name
                .clone()
                .unwrap_or_else(|| m.user_uuid.clone());
            let role = m.role.clone();
            let permission = m.permission.clone();

            let (u_view, u_edit, u_manage, u_remove) = (
                user_uuid.clone(),
                user_uuid.clone(),
                user_uuid.clone(),
                user_uuid.clone(),
            );

            let perm_controls = h_flex()
                .gap_1()
                .child(
                    Button::new(format!("mperm-view-{i}"))
                        .compact()
                        .when(permission == "can_view", |b| b.primary())
                        .label("View")
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                            this.do_update_member_permission(
                                window,
                                cx,
                                u_view.clone(),
                                "can_view".into(),
                            );
                        })),
                )
                .child(
                    Button::new(format!("mperm-edit-{i}"))
                        .compact()
                        .when(permission == "can_edit", |b| b.primary())
                        .label("Edit")
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                            this.do_update_member_permission(
                                window,
                                cx,
                                u_edit.clone(),
                                "can_edit".into(),
                            );
                        })),
                )
                .child(
                    Button::new(format!("mperm-manage-{i}"))
                        .compact()
                        .when(permission == "can_manage", |b| b.primary())
                        .label("Manage")
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                            this.do_update_member_permission(
                                window,
                                cx,
                                u_manage.clone(),
                                "can_manage".into(),
                            );
                        })),
                )
                .child(
                    Button::new(format!("mremove-{i}"))
                        .compact()
                        .danger()
                        .label("Remove")
                        .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                            this.do_remove_member(window, cx, u_remove.clone());
                        })),
                );

            let row = div()
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(theme::BORDER)
                .child(
                    v_flex()
                        .gap_0()
                        .child(div().text_sm().font_weight(FontWeight::BOLD).child(display))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::TEXT_DIM)
                                .child(format!("{role} · {permission}")),
                        ),
                )
                .child(perm_controls);
            rows.push(row);
        }

        let user_uuid = self.member_user_input.read(cx).value().to_string();

        v_flex()
            .gap_2()
            .child(
                div().text_sm().font_weight(FontWeight::BOLD).child("Members"),
            )
            .children(rows)
            .child(div().border_t_1().border_color(theme::BORDER).mt_1())
            .child(div().text_xs().text_color(theme::TEXT_MUTED).child("Add member"))
            .child(Input::new(&self.member_user_input).w_full())
            .child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .child(
                        Button::new("role-member")
                            .when(self.projects.member_role == "member", |b| b.primary())
                            .compact()
                            .label("member")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.member_role = "member".into();
                                    cx.notify();
                                },
                            )),
                    )
                    .child(
                        Button::new("role-manager")
                            .when(self.projects.member_role == "manager", |b| b.primary())
                            .compact()
                            .label("manager")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.member_role = "manager".into();
                                    cx.notify();
                                },
                            )),
                    )
                    .child(
                        Button::new("role-admin")
                            .when(self.projects.member_role == "admin", |b| b.primary())
                            .compact()
                            .label("admin")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.member_role = "admin".into();
                                    cx.notify();
                                },
                            )),
                    )
                    .child(
                        Button::new("role-owner")
                            .when(self.projects.member_role == "owner", |b| b.primary())
                            .compact()
                            .label("owner")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.member_role = "owner".into();
                                    cx.notify();
                                },
                            )),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("perm-canview")
                            .when(self.projects.member_permission == "can_view", |b| b.primary())
                            .compact()
                            .label("Can View")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.member_permission = "can_view".into();
                                    cx.notify();
                                },
                            )),
                    )
                    .child(
                        Button::new("perm-canedit")
                            .when(self.projects.member_permission == "can_edit", |b| b.primary())
                            .compact()
                            .label("Can Edit")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.member_permission = "can_edit".into();
                                    cx.notify();
                                },
                            )),
                    )
                    .child(
                        Button::new("perm-canmanage")
                            .when(self.projects.member_permission == "can_manage", |b| b.primary())
                            .compact()
                            .label("Can Manage")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, _window, cx| {
                                    this.projects.member_permission = "can_manage".into();
                                    cx.notify();
                                },
                            )),
                    ),
            )
            .child(
                Button::new("add-member-btn")
                    .primary()
                    .label("Add member")
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                        this.do_add_member(window, cx);
                    })),
            )
            .child(div().text_xs().text_color(theme::TEXT_DIM).child(format!(
                "Current member field: {user_uuid}"
            )))
    }

    fn render_secrets(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut rows = Vec::new();
        for (i, s) in self.projects.secrets.iter().enumerate() {
            let uuid = s.uuid.clone();
            let uuid_reveal = uuid.clone();
            let uuid_delete = uuid.clone();
            let key = s.key.clone();
            let version = s.version;
            let updated_at = s.updated_at;
            let row = div()
                .id(SharedString::from(format!("secret-row-{i}")))
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(theme::BORDER)
                .child(
                    v_flex()
                        .gap_0()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child(key.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::TEXT_DIM)
                                .child(format!("v{version} · updated {updated_at}")),
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new(format!("sreveal-{i}"))
                                .compact()
                                .label("Reveal")
                                .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                                    this.reveal_target_uuid = Some(uuid_reveal.clone());
                                    this.do_reveal_secret(window, cx);
                                })),
                        )
                        .child(
                            Button::new(format!("sdelete-{i}"))
                                .compact()
                                .danger()
                                .label("Delete")
                                .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                                    this.do_delete_secret(window, cx, uuid_delete.clone());
                                })),
                        ),
                );
            rows.push(row);
        }

        let revealed = self
            .projects
            .revealed
            .clone()
            .map(|(key, value)| {
                div()
                    .px_3()
                    .py_2()
                    .mt_1()
                    .bg(theme::SURFACE)
                    .rounded_md()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::TEXT_MUTED)
                            .child(format!("Revealed {key}")),
                    )
                    .child(div().text_sm().child(value))
            })
            .unwrap_or_else(|| div());

        v_flex()
            .gap_2()
            .child(div().text_sm().font_weight(FontWeight::BOLD).child("Secrets"))
            .children(rows)
            .child(div().border_t_1().border_color(theme::BORDER).mt_1())
            .child(div().text_xs().text_color(theme::TEXT_MUTED).child("New secret"))
            .child(Input::new(&self.secret_key_input).w_full())
            .child(Input::new(&self.secret_value_input).w_full())
            .child(
                Button::new("add-secret-btn")
                    .primary()
                    .label("Create secret")
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                        this.do_add_secret(window, cx);
                    })),
            )
            .child(revealed)
    }

    fn render_offboard(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let result = self.projects.offboard_result.clone();
        v_flex()
            .mt_1()
            .pt_2()
            .border_t_1()
            .border_color(theme::BORDER)
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(theme::DANGER_TEXT)
                    .child("Offboard (revoke all access)"),
            )
            .child(h_flex().gap_2().child(Input::new(&self.offboard_input).w_full()).child(
                Button::new("offboard-btn")
                    .danger()
                    .label("Revoke all")
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                        this.do_offboard(window, cx);
                    })),
            ))
            .child(
                div()
                    .when(result.is_some(), |this| {
                        this.text_xs()
                            .text_color(theme::SUCCESS)
                            .child(result.clone().unwrap_or_default())
                    }),
            )
    }

    // ── Generator section ─────────────────────────────────────────────────

    fn render_generator(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let password = self.generator_password.clone();
        self.page()
            .child(self.page_header(
                "Password generator",
                "Generate strong passwords and detect weak or reused ones.",
            ))
            .child(
                self.card(
                    "Generator",
                    "Options for a cryptographically-secure random password.",
                )
                .child(
                    div()
                        .w_full()
                        .p_3()
                        .rounded_md()
                        .bg(theme::SURFACE_RAISED)
                        .border_1()
                        .border_color(theme::BORDER)
                        .font_family("ui-monospace")
                        .text_color(theme::WARN)
                        .child(password),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("generator-btn")
                                .primary()
                                .label("Generate")
                                .on_click(cx.listener(
                                    |this, _: &gpui::ClickEvent, _window, cx| {
                                        this.do_regenerate_generator(cx);
                                    },
                                )),
                        ),
                ),
            )
    }

    // ── MFA section ───────────────────────────────────────────────────────

    fn render_mfa(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.mfa_status.clone();
        let enrolled = self.mfa_enrolled.clone();
        let text = self.mfa_text.clone();
        let status_line = match &status {
            Some(s) => {
                let methods = if s.configured_methods.is_empty() {
                    "none".to_string()
                } else {
                    s.configured_methods.join(", ")
                };
                format!("MFA: required={}  configured=[{}]", s.required, methods)
            }
            None => "MFA status: unknown".to_string(),
        };
        let mut body = self
            .page()
            .child(self.page_header(
                "MFA & security",
                "Manage two-factor authentication for your account.",
            ))
            .child(
                self.card("Status", "Your current two-factor authentication state.")
                    .child(div().text_sm().text_color(theme::TEXT_MUTED).child(status_line))
                    .child(
                        h_flex().gap_2().child(
                            Button::new("mfa-refresh-btn")
                                .label("Refresh")
                                .on_click(cx.listener(
                                    |this, _: &gpui::ClickEvent, window, cx| {
                                        this.do_refresh_mfa(window, cx);
                                    },
                                )),
                        ),
                    )
                    .when(!text.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(theme::WARN)
                                .child(text.clone()),
                        )
                    }),
            );

        if let Some(issued) = enrolled {
            body = body.child(
                self.card("Enrollment", "Finish enrolling your authenticator.")
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme::TEXT_MUTED)
                            .child(
                                "Scan the QR code / enter this TOTP secret into your authenticator:",
                            ),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme::SURFACE_RAISED)
                            .font_family("ui-monospace")
                            .child(issued.secret.clone()),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Input::new(&self.mfa_code_input).w_full())
                            .child(
                                Button::new("mfa-verify-btn")
                                    .primary()
                                    .label("Verify")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_verify_totp(window, cx);
                                        },
                                    )),
                            ),
                    ),
            );
        }

        body = body.child(
            self.card("Setup", "Enroll a new authenticator app.")
                .child(
                    h_flex().gap_2().child(
                        Button::new("mfa-enroll-btn")
                            .primary()
                            .label("Enroll TOTP")
                            .on_click(cx.listener(
                                |this, _: &gpui::ClickEvent, window, cx| {
                                    this.do_enroll_totp(window, cx);
                                },
                            )),
                    ),
                ),
        );

        body
    }

    // ── Dashboard section ────────────────────────────────────────────────

    fn render_dashboard(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let projects = self.projects.projects.clone();
        let machines = self.machines.clone();
        let tokens = self.tokens.clone();
        let mfa_methods = self
            .mfa_status
            .as_ref()
            .map(|m| m.configured_methods.len())
            .unwrap_or(0);
        let backup = self.backup.clone();
        let loading = self.dashboard_loading;

        let stat = |label: &str, value: usize, icon: IconName| {
            v_flex()
                .flex_1()
                .border_1()
                .border_color(theme::BORDER)
                .rounded_lg()
                .bg(theme::SURFACE)
                .p_4()
                .gap_2()
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme::TEXT_MUTED)
                                .child(label.to_string()),
                        )
                        .child(Icon::new(icon).size_4().text_color(theme::ACCENT)),
                )
                .child(
                    div()
                        .text_3xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::TEXT)
                        .child(value.to_string()),
                )
        };

        let mut recent_rows: Vec<AnyElement> = Vec::new();
        for p in projects.iter().take(5) {
            let name = p.name.clone();
            let kind = p.kind.clone();
            let perm = p.permission.clone().unwrap_or_else(|| "—".into());
            let meta = format!("{kind} · {perm}");
            recent_rows.push(
                h_flex()
                    .justify_between()
                    .items_center()
                    .px_4()
                    .py_3()
                    .rounded_md()
                    .border_1()
                    .border_color(theme::BORDER)
                    .bg(theme::SURFACE_RAISED)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(IconName::Folder).size_4().text_color(theme::ACCENT))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::TEXT)
                                    .child(name),
                            ),
                    )
                    .child(div().text_xs().text_color(theme::TEXT_MUTED).child(meta))
                    .into_any_element(),
            );
        }

        let backup_text = match &backup {
            Some(b) if b.enabled => format!(
                "Enabled · last backup {}",
                b.last_backup_at.map(fmt_time).unwrap_or_else(|| "n/a".into())
            ),
            Some(_) => "Not configured".to_string(),
            None => "Backup API unavailable".to_string(),
        };
        let projects_desc = if projects.is_empty() {
            "No projects yet. Create one to get started.".to_string()
        } else {
            format!("You can access {} project(s).", projects.len())
        };

        self.page()
            .child(self.page_header(
                "Dashboard",
                "Overview of your organization's vaults and secrets.",
            ))
            .when(loading, |this| {
                this.child(div().text_sm().text_color(theme::TEXT_MUTED).child("Loading…"))
            })
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .child(stat("Projects", projects.len(), IconName::Folder))
                    .child(stat("Machine accounts", machines.len(), IconName::Bot))
                    .child(stat("Access tokens", tokens.len(), IconName::Globe))
                    .child(stat("MFA", mfa_methods, IconName::CircleCheck)),
            )
            .child(
                self.card("Recent projects", &projects_desc).children(recent_rows),
            )
            .child(
                self.card("Backup status", "Automated and on-demand backups.").child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::HardDrive).size_4().text_color(theme::TEXT_MUTED))
                        .child(div().text_sm().text_color(theme::TEXT).child(backup_text)),
                ),
            )
    }

    // ── Secrets overview section ────────────────────────────────────────

    fn render_secrets_overview(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.secrets_rows.clone();
        let revealed = self.secrets_revealed.clone();
        let loading = self.secrets_loading;
        let error = self.secrets_error.clone().unwrap_or_default();

        let header = h_flex()
            .px_3()
            .py_2()
            .gap_3()
            .border_b_1()
            .border_color(theme::BORDER)
            .child(
                div()
                    .w_11()
                    .text_xs()
                    .text_color(theme::TEXT_MUTED)
                    .child("Project"),
            )
            .child(div().flex_1().text_xs().text_color(theme::TEXT_MUTED).child("Key"))
            .child(
                div()
                    .w_16()
                    .text_xs()
                    .text_color(theme::TEXT_MUTED)
                    .child("Version"),
            )
            .child(
                div()
                    .w_7()
                    .text_xs()
                    .text_color(theme::TEXT_MUTED)
                    .child("Value"),
            );

        let mut body: Vec<AnyElement> = Vec::new();
        for (i, (project, secret)) in rows.iter().enumerate() {
            let uuid = secret.uuid.clone();
            let reveal_uuid = uuid.clone();
            let pname = project.name.clone();
            let ptype = project.kind.clone();
            let key = secret.key.clone();
            let version = secret.version;
            let is_revealed = revealed.contains_key(&uuid);
            body.push(
                h_flex()
                    .id(SharedString::from(format!("secret-row-{i}")))
                    .px_3()
                    .py_2()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme::BORDER)
                    .child(
                        v_flex()
                            .w_11()
                            .gap_0p5()
                            .child(div().text_sm().text_color(theme::TEXT).child(pname))
                            .child(div().text_xs().text_color(theme::TEXT_MUTED).child(ptype)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_family("ui-monospace")
                            .text_color(theme::TEXT)
                            .child(key),
                    )
                    .child(
                        div()
                            .w_16()
                            .text_sm()
                            .text_color(theme::TEXT_MUTED)
                            .child(version.to_string()),
                    )
                    .child(
                        h_flex().w_7().child(
                            Button::new(format!("sreveal-{i}"))
                                .compact()
                                .label(if is_revealed { "Hide" } else { "Reveal" })
                                .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                                    this.do_reveal_overview_secret(window, cx, reveal_uuid.clone());
                                })),
                        ),
                    )
                    .into_any_element(),
            );
        }

        self.page()
            .child(self.page_header(
                "Secrets",
                "Secrets are project-scoped. Revealing a value requires the secrets:reveal scope.",
            ))
            .when(!error.is_empty(), |this| {
                this.child(div().text_sm().text_color(theme::DANGER).child(error.clone()))
            })
            .when(loading, |this| {
                this.child(div().text_sm().text_color(theme::TEXT_MUTED).child("Loading…"))
            })
            .child(
                self.card("All secrets", "Every secret across your projects.")
                    .when(!loading && rows.is_empty(), |this| {
                        this.child(
                            div()
                                .py_6()
                                .w_full()
                                .text_center()
                                .text_sm()
                                .text_color(theme::TEXT_MUTED)
                                .child("No secrets found across your projects. Open a project to add one."),
                        )
                    })
                    .when(!rows.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .rounded_md()
                                .border_1()
                                .border_color(theme::BORDER)
                                .child(header)
                                .children(body),
                        )
                    })
                    .when(!revealed.is_empty(), |this| {
                        let mut c = this;
                        for (_, secret) in &rows {
                            if let Some(plain) = revealed.get(&secret.uuid) {
                                let key = secret.key.clone();
                                let val = plain.clone();
                                c = c.child(
                                    h_flex()
                                        .gap_2()
                                        .px_3()
                                        .py_2()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(theme::ACCENT_DIM)
                                        .bg(theme::ACCENT_DIM)
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme::TEXT_MUTED)
                                                .child(format!("{key}:")),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_family("ui-monospace")
                                                .text_color(theme::TEXT)
                                                .child(val),
                                        ),
                                );
                            }
                        }
                        c
                    }),
            )
    }

    /// Checkbox-style rows for the scope selector (used by the Machine
    /// accounts and Tokens create forms).
    fn render_scope_toggles(&mut self, cx: &mut Context<Self>, id_prefix: &str) -> Vec<AnyElement> {
        let scopes = self.settings_scopes.clone();
        SCOPES
            .iter()
            .map(|s| {
                let active = scopes.iter().any(|x| x == s);
                let label = s.to_string();
                let sval = s.to_string();
                div()
                    .id(SharedString::from(format!("{id_prefix}-scope-{sval}")))
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_md()
                    .when(active, |d| d.bg(theme::ACCENT_DIM))
                    .child(
                        div()
                            .size_4()
                            .rounded_sm()
                            .border_1()
                            .border_color(if active {
                                theme::ACCENT
                            } else {
                                theme::BORDER
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(if active {
                                Icon::new(IconName::Check)
                                    .size_3()
                                    .text_color(theme::ACCENT)
                                    .into_any_element()
                            } else {
                                div().into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_family("ui-monospace")
                            .text_color(theme::TEXT)
                            .child(label),
                    )
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _window, cx| {
                        this.do_toggle_scope(sval.clone(), cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }

    // ── Machine accounts section ────────────────────────────────────────

    fn render_machine_accounts(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let machines = self.machines.clone();
        let text = self.settings_text.clone();
        let scope_toggles = self.render_scope_toggles(cx, "ma");

        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, m) in machines.iter().enumerate() {
            let uuid = m.uuid.clone();
            let name = m.name.clone();
            let status = m.status.clone();
            let created = m.created_at;
            let is_active = status == "active";
            let toggle_uuid = uuid.clone();
            let delete_uuid = uuid.clone();
            let scopes = m.scopes.clone();
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("ma-row-{i}")))
                    .px_3()
                    .py_2()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme::BORDER)
                    .items_center()
                    .child(
                        v_flex()
                            .w_56()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::TEXT)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("ui-monospace")
                                    .text_color(theme::TEXT_MUTED)
                                    .child(uuid),
                            ),
                    )
                    .child(status_badge(&status))
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .children(scopes.iter().map(|s| scope_pill(s))),
                    )
                    .child(
                        div()
                            .w_7()
                            .text_sm()
                            .text_color(theme::TEXT_MUTED)
                            .child(fmt_time(created)),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new(format!("ma-toggle-{i}"))
                                    .compact()
                                    .label(if is_active { "Disable" } else { "Enable" })
                                    .on_click(cx.listener(
                                        move |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_toggle_machine(window, cx, toggle_uuid.clone());
                                        },
                                    )),
                            )
                            .child(
                                Button::new(format!("ma-del-{i}"))
                                    .compact()
                                    .label("Delete")
                                    .on_click(cx.listener(
                                        move |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_delete_machine(window, cx, delete_uuid.clone());
                                        },
                                    )),
                            ),
                    )
                    .into_any_element(),
            );
        }

        self.page()
            .child(self.page_header(
                "Machine accounts",
                "Non-human identities for CI/CD, apps, and agents.",
            ))
            .when(!text.is_empty(), |this| {
                this.child(div().text_sm().text_color(theme::WARN).child(text.clone()))
            })
            .child(
                self.card("Machine accounts", "Service identities with scoped API access.")
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::TEXT_MUTED)
                                    .child(format!("{} account(s)", machines.len())),
                            )
                            .child(
                                Button::new("ma-refresh")
                                    .compact()
                                    .label("Refresh")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_refresh_settings(window, cx);
                                        },
                                    )),
                            ),
                    )
                    .when(rows.is_empty(), |this| {
                        this.child(
                            div()
                                .py_6()
                                .w_full()
                                .text_center()
                                .text_sm()
                                .text_color(theme::TEXT_MUTED)
                                .child("No machine accounts yet."),
                        )
                    })
                    .when(!rows.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .rounded_md()
                                .border_1()
                                .border_color(theme::BORDER)
                                .children(rows),
                        )
                    }),
            )
            .child(
                self.card("New machine account", "Register a new service identity.")
                    .child(Input::new(&self.settings_name_input).w_full())
                    .child(v_flex().gap_1().children(scope_toggles))
                    .child(
                        h_flex().child(
                            Button::new("ma-create")
                                .primary()
                                .label("Create machine account")
                                .on_click(cx.listener(
                                    |this, _: &gpui::ClickEvent, window, cx| {
                                        this.do_create_machine(window, cx);
                                    },
                                )),
                        ),
                    ),
            )
    }

    // ── Tokens section ──────────────────────────────────────────────────

    fn render_tokens(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = self.tokens.clone();
        let text = self.settings_text.clone();
        let issued = self.issued_token.clone();
        let scope_toggles = self.render_scope_toggles(cx, "tok");

        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, t) in tokens.iter().enumerate() {
            let uuid = t.uuid.clone();
            let name = t.name.clone();
            let prefix = t.prefix.clone().unwrap_or_default();
            let revoke_uuid = uuid.clone();
            let scopes = t.scopes.clone();
            let expires = t.expires_at.map(fmt_time).unwrap_or_else(|| "never".into());
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("tok-row-{i}")))
                    .px_3()
                    .py_2()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme::BORDER)
                    .items_center()
                    .child(
                        v_flex()
                            .w_56()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::TEXT)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("ui-monospace")
                                    .text_color(theme::TEXT_MUTED)
                                    .child(uuid),
                            ),
                    )
                    .child(
                        div()
                            .w_32()
                            .text_sm()
                            .font_family("ui-monospace")
                            .text_color(theme::TEXT_MUTED)
                            .child(format!("{prefix}…")),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .children(scopes.iter().map(|s| scope_pill(s))),
                    )
                    .child(
                        div()
                            .w_24()
                            .text_sm()
                            .text_color(theme::TEXT_MUTED)
                            .child(expires),
                    )
                    .child(
                        Button::new(format!("tok-revoke-{i}"))
                            .compact()
                            .label("Revoke")
                            .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                                this.do_revoke_token(window, cx, revoke_uuid.clone());
                            })),
                    )
                    .into_any_element(),
            );
        }

        self.page()
            .child(self.page_header(
                "Access tokens",
                "Issue scoped tokens with expiration and revocation.",
            ))
            .when(!text.is_empty(), |this| {
                this.child(div().text_sm().text_color(theme::WARN).child(text.clone()))
            })
            .when(issued.is_some(), |this| {
                let tok = issued.clone().unwrap_or_default();
                this.child(
                    self.card("Save this token now", "The full token is shown only once. Store it somewhere safe.")
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .px_3()
                                        .py_2()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(theme::BORDER)
                                        .bg(theme::SURFACE_RAISED)
                                        .font_family("ui-monospace")
                                        .text_sm()
                                        .text_color(theme::TEXT)
                                        .child(tok),
                                )
                                .child(
                                    Button::new("issued-dismiss")
                                        .compact()
                                        .label("Dismiss")
                                        .on_click(cx.listener(
                                            |this, _: &gpui::ClickEvent, _window, cx| {
                                                this.issued_token = None;
                                                cx.notify();
                                            },
                                        )),
                                ),
                        ),
                )
            })
            .child(
                self.card("Tokens", "Long-lived scoped access tokens.")
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::TEXT_MUTED)
                                    .child(format!("{} token(s)", tokens.len())),
                            )
                            .child(
                                Button::new("tok-refresh")
                                    .compact()
                                    .label("Refresh")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_refresh_settings(window, cx);
                                        },
                                    )),
                            ),
                    )
                    .when(rows.is_empty(), |this| {
                        this.child(
                            div()
                                .py_6()
                                .w_full()
                                .text_center()
                                .text_sm()
                                .text_color(theme::TEXT_MUTED)
                                .child("No access tokens yet."),
                        )
                    })
                    .when(!rows.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .rounded_md()
                                .border_1()
                                .border_color(theme::BORDER)
                                .children(rows),
                        )
                    }),
            )
            .child(
                self.card("New token", "Issue a token with fine-grained scopes.")
                    .child(Input::new(&self.settings_name_input).w_full())
                    .child(v_flex().gap_1().children(scope_toggles))
                    .child(
                        h_flex().child(
                            Button::new("tok-create")
                                .primary()
                                .label("Create token")
                                .on_click(cx.listener(
                                    |this, _: &gpui::ClickEvent, window, cx| {
                                        this.do_create_token(window, cx);
                                    },
                                )),
                        ),
                    ),
            )
    }

    // ── Import / export section ─────────────────────────────────────────

    fn render_import_export(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let backup = self.backup.clone();
        let include = self.include_secrets;
        let text = self.import_text.clone();
        let busy = self.import_busy;

        self.page()
            .child(self.page_header(
                "Import / export",
                "Backup and restore your organization data against the live server.",
            ))
            .when(!text.is_empty(), |this| {
                this.child(div().text_sm().text_color(theme::WARN).child(text.clone()))
            })
            .when(backup.is_some(), |this| {
                let b = backup.clone().unwrap();
                let status = if b.enabled { "enabled" } else { "disabled" };
                let card = self
                    .card("Backup status", "Automated and on-demand backups.")
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(status_badge(status))
                            .when(b.last_backup_at.is_some(), |c| {
                                c.child(
                                    div()
                                        .text_sm()
                                        .text_color(theme::TEXT_MUTED)
                                        .child(format!(
                                            "Last backup {}",
                                            fmt_time(b.last_backup_at.unwrap())
                                        )),
                                )
                            })
                            .when(b.last_restore_test_status.is_some(), |c| {
                                c.child(
                                    div()
                                        .text_sm()
                                        .text_color(theme::TEXT_MUTED)
                                        .child(format!(
                                            "Last restore test: {}",
                                            b.last_restore_test_status.clone().unwrap()
                                        )),
                                )
                            }),
                    );
                this.child(card)
            })
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .child(
                        v_flex()
                            .flex_1()
                            .border_1()
                            .border_color(theme::BORDER)
                            .rounded_lg()
                            .bg(theme::SURFACE)
                            .p_5()
                            .gap_4()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::TEXT)
                                            .child("Export backup"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme::TEXT_MUTED)
                                            .child("Create an encrypted backup archive of the current state."),
                                    ),
                            )
                            .child(
                                div()
                                    .id("inc-secrets")
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .px_1()
                                    .py_0p5()
                                    .rounded_md()
                                    .when(include, |d| d.bg(theme::ACCENT_DIM))
                                    .child(
                                        div()
                                            .size_4()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(if include {
                                                theme::ACCENT
                                            } else {
                                                theme::BORDER
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(if include {
                                                Icon::new(IconName::Check)
                                                    .size_3()
                                                    .text_color(theme::ACCENT)
                                                    .into_any_element()
                                            } else {
                                                div().into_any_element()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme::TEXT)
                                            .child("Include secret values"),
                                    )
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, _window, cx| {
                                            this.do_toggle_include_secrets(cx);
                                        },
                                    )),
                            )
                            .child(
                                Button::new("export-backup")
                                    .primary()
                                    .label(if busy { "Exporting…" } else { "Export backup" })
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_export_backup(window, cx);
                                        },
                                    )),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .border_1()
                            .border_color(theme::BORDER)
                            .rounded_lg()
                            .bg(theme::SURFACE)
                            .p_5()
                            .gap_4()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::TEXT)
                                            .child("Restore backup"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme::TEXT_MUTED)
                                            .child("Restore from a base64 archive or a backup ID."),
                                    ),
                            )
                            .child(Input::new(&self.import_archive_input).w_full())
                            .child(
                                Button::new("restore-backup")
                                    .label("Restore")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_restore_backup(window, cx);
                                        },
                                    )),
                            ),
                    ),
            )
    }

    fn render_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let machines = self.machines.clone();
        let tokens = self.tokens.clone();
        let text = self.settings_text.clone();
        let mut machine_rows = Vec::new();
        for m in &machines {
            machine_rows.push(
                v_flex()
                    .p_2()
                    .rounded_md()
                    .bg(theme::SURFACE_RAISED)
                    .gap_1()
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child(m.name.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::TEXT_MUTED)
                            .child(format!("uuid: {}  scopes: {:?}", m.uuid, m.scopes)),
                    ),
            );
        }
        let mut token_rows = Vec::new();
        for t in &tokens {
            token_rows.push(
                v_flex()
                    .p_2()
                    .rounded_md()
                    .bg(theme::SURFACE_RAISED)
                    .gap_1()
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child(t.name.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::TEXT_MUTED)
                            .child(format!("uuid: {}  scopes: {:?}", t.uuid, t.scopes)),
                    ),
            );
        }
        let machine_empty = machine_rows.is_empty();
        let token_empty = token_rows.is_empty();

        self.page()
            .child(self.page_header(
                "Settings",
                "Organization and security administration.",
            ))
            .when(!text.is_empty(), |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(theme::WARN)
                        .child(text.clone()),
                )
            })
            .child(
                self.card("Machine accounts", "Service identities with scoped API access.")
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::TEXT_MUTED)
                                    .child(format!("{} account(s)", machines.len())),
                            )
                            .child(
                                Button::new("settings-refresh-btn")
                                    .compact()
                                    .label("Refresh")
                                    .on_click(cx.listener(
                                        |this, _: &gpui::ClickEvent, window, cx| {
                                            this.do_refresh_settings(window, cx);
                                        },
                                    )),
                            ),
                    )
                    .when(machine_empty, |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(theme::TEXT_MUTED)
                                .child("No machine accounts yet."),
                        )
                    })
                    .children(machine_rows),
            )
            .child(
                self.card("API tokens", "Long-lived tokens for API access.")
                    .when(token_empty, |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(theme::TEXT_MUTED)
                                .child("No API tokens yet."),
                        )
                    })
                    .children(token_rows),
            )
            .child(
                self.card("Create a machine account", "Register a new service identity.")
                    .child(
                        h_flex().gap_2().child(Input::new(&self.settings_name_input).w_full()),
                    )
                    .child(
                        h_flex().gap_2().child(
                            Button::new("settings-create-machine-btn")
                                .primary()
                                .label("Create machine account")
                                .on_click(cx.listener(
                                    |this, _: &gpui::ClickEvent, window, cx| {
                                        this.do_create_machine(window, cx);
                                    },
                                )),
                        ),
                    ),
            )
    }
}

impl Focusable for DesktopView {
    fn focus_handle(&self, _app: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Generate a cryptographically random password using characters safe for
/// most password rules. Mirrors the `@vautr/ui-logic` generator on the
/// Rust side (the desktop client has no JS runtime).
fn generate_password(length: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARS.len());
            CHARS[idx] as char
        })
        .collect()
}

/// Format a Unix-timestamp (seconds) as a compact local date/time string.
fn fmt_time(secs: i64) -> String {
    if secs <= 0 {
        return "n/a".into();
    }
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "n/a".into())
}

/// A small status pill ("active" uses the accent; everything else is muted).
fn status_badge(status: &str) -> Div {
    let (bg, ink) = if status == "active" {
        (theme::ACCENT_DIM, theme::ACCENT)
    } else {
        (theme::SURFACE_RAISED, theme::TEXT_MUTED)
    };
    div()
        .px_2()
        .py_0p5()
        .rounded_md()
        .bg(bg)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(ink)
        .child(status.to_string())
}

/// A mono pill rendering a single access scope (e.g. `secrets:read`).
fn scope_pill(scope: &str) -> Div {
    div()
        .px_2()
        .py_0p5()
        .rounded_md()
        .border_1()
        .border_color(theme::BORDER)
        .text_xs()
        .font_family("ui-monospace")
        .text_color(theme::TEXT_MUTED)
        .child(scope.to_string())
}

