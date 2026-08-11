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

/// Which post-login section is active.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Vault,
    Projects,
    Generator,
    Mfa,
    Settings,
}

/// Which login form mode is active (mirrors the web UnlockScreen).
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoginMode {
    Login,
    Register,
}

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
                    .p_8()
                    .flex()
                    .flex_col()
                    .gap_4()
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
            .py_1_5()
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
            Section::Vault => self.render_vault_content(cx).into_any_element(),
            Section::Projects => self.render_projects(cx).into_any_element(),
            Section::Generator => self.render_generator(cx).into_any_element(),
            Section::Mfa => self.render_mfa(cx).into_any_element(),
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

        let items: [(Section, &'static str, IconName); 5] = [
            (Section::Vault, "Vault", IconName::Eye),
            (Section::Projects, "Projects", IconName::Folder),
            (Section::Generator, "Generator", IconName::Settings2),
            (Section::Mfa, "MFA & security", IconName::CircleCheck),
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
            Section::Projects => self.do_refresh_projects(window, cx),
            Section::Mfa => self.do_refresh_mfa(window, cx),
            Section::Settings => self.do_refresh_settings(window, cx),
        }
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

        let revealed_text = self
            .revealed
            .as_deref()
            .map(|s| {
                div()
                    .px_3()
                    .py_2()
                    .mt_2()
                    .bg(theme::SURFACE)
                    .rounded_md()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_xs().text_color(theme::TEXT_MUTED).child("Secret"))
                    .child(div().text_sm().child(s.to_string()))
            })
            .unwrap_or_else(|| div());

        let error = self.vault.error_message.clone().unwrap_or_default();
        let has_error = !error.is_empty();

        v_flex()
            .size_full()
            .child(
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
                        Button::new("sync-btn")
                            .label("Sync")
                            .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                                this.do_sync(window, cx);
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
                div()
                    .when(has_error, |this| {
                        this.px_6()
                            .child(
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
                    .id("item-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_4()
                    .py_2()
                    .children(rows),
            )
            .child(revealed_text)
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
            .child(
                div()
                    .when(has_error, |this| {
                        this.px_6()
                            .pt_2()
                            .child(
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
                        this.px_6()
                            .pt_1()
                            .child(
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
        v_flex()
            .px_6()
            .py_4()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child("Password Generator"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme::TEXT_MUTED)
                    .child("Generate a strong, random password for a new account."),
            )
            .child(
                div()
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .bg(theme::SURFACE)
                    .border_1()
                    .border_color(theme::BORDER)
                    .font_family("ui-monospace")
                    .text_color(theme::WARN)
                    .child(password),
            )
            .child(
                h_flex().gap_2().child(
                    Button::new("generator-btn")
                        .primary()
                        .label("Generate")
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _window, cx| {
                            this.do_regenerate_generator(cx);
                        })),
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
        v_flex()
            .px_6()
            .py_4()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child("Two-Factor Authentication"),
            )
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
            })
            .when(enrolled.is_some(), |this| {
                let issued = enrolled.clone().unwrap();
                this.child(
                    v_flex().gap_2().child(
                        div()
                            .text_sm()
                            .text_color(theme::TEXT_MUTED)
                            .child("Scan the QR code / enter this TOTP secret into your authenticator:"),
                    ).child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme::SURFACE)
                            .font_family("ui-monospace")
                            .child(issued.secret.clone()),
                    ).child(
                        h_flex().gap_2().child(
                            Input::new(&self.mfa_code_input).w_full(),
                        ).child(
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
                )
            })
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
            )
    }

    // ── Settings section ──────────────────────────────────────────────────

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
                    .bg(theme::SURFACE)
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
                    .bg(theme::SURFACE)
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
        v_flex()
            .px_6()
            .py_4()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child("Settings"),
            )
            .child(
                h_flex().gap_2().child(
                    Button::new("settings-refresh-btn")
                        .label("Refresh")
                        .on_click(cx.listener(
                            |this, _: &gpui::ClickEvent, window, cx| {
                                this.do_refresh_settings(window, cx);
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
            })
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child("Machine accounts"),
                    )
                    .children(machine_rows),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child("API tokens"),
                    )
                    .children(token_rows),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme::TEXT_MUTED)
                    .child("Create a machine account"),
            )
            .child(
                h_flex().gap_2().child(
                    Input::new(&self.settings_name_input).w_full(),
                ),
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

