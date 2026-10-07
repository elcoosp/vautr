//! Client configuration for the Vautr CLI.
//!
//! Persists a single JSON config file (default `~/.config/vautr/config.json`,
//! overridable with `VAUTR_CONFIG`) holding the server URL, the active user
//! session, machine-account credential metadata, and the client-side project
//! keys the CLI needs to encrypt/decrypt secret values (the zero-knowledge
//! server stores only ciphertext and never distributes project keys, so the
//! CLI keeps the keys it generates locally, keyed by project UUID).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The Vautr server base URL used when no config / `--server` is supplied.
pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8080";

/// Machine-account credential metadata stored alongside the session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MachineAccountInfo {
    /// The machine-account UUID.
    pub uuid: String,
    /// The machine-account display name.
    pub name: String,
    /// The access-token display prefix (first 8 chars).
    pub token_prefix: String,
}

/// Persistent CLI configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// Base URL of the Vautr server this CLI talks to.
    pub server_url: String,
    /// Active user session token (from `login`).
    pub session_token: Option<String>,
    /// Email/username of the logged-in user.
    pub username: Option<String>,
    /// Provisioned machine-account credential (from `login --machine-account`).
    pub machine_account: Option<MachineAccountInfo>,
    /// Client-side project keys: project UUID -> base64 32-byte key.
    pub project_keys: BTreeMap<String, String>,
}

impl Config {
    /// A config backed by the default server URL with no credentials.
    pub fn new() -> Self {
        Self {
            server_url: DEFAULT_SERVER_URL.to_string(),
            session_token: None,
            username: None,
            machine_account: None,
            project_keys: BTreeMap::new(),
        }
    }

    /// A config with an explicit server URL.
    pub fn with_server_url(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            ..Self::new()
        }
    }

    /// Locate the config file path.
    ///
    /// Honors `VAUTR_CONFIG` if set; otherwise uses `~/.config/vautr/config.json`
    /// (falling back to `~/.vautr/config.json` if `XDG_CONFIG_HOME`/`HOME` are
    /// unavailable). Pure function so it is unit-testable.
    pub fn config_path() -> PathBuf {
        if let Ok(p) = std::env::var("VAUTR_CONFIG") {
            if !p.is_empty() {
                return PathBuf::from(p);
            }
        }
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&home).join(".config"));
        base.join("vautr").join("config.json")
    }

    /// Load configuration from disk, defaulting to [`Config::new`] on a missing
    /// file. A malformed file surfaces as an error.
    pub fn load() -> CliResult<Self> {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(serde_json::from_str(&text)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::new()),
            Err(e) => Err(e.into()),
        }
    }

    /// Persist the configuration to disk, creating parent directories.
    ///
    /// VTRFIX-SEC-H15: the config file holds the session token and project
    /// encryption keys, so it must be 0600 on unix and written atomically
    /// (temp + rename) so a crash cannot leave a half-written file that drops
    /// the only copy of those keys.
    pub fn save(&self) -> CliResult<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }
        let text = serde_json::to_string_pretty(self)?;
        write_private_atomic(&path, text.as_bytes())?;
        Ok(())
    }

    /// The active credential token used for data-plane requests.
    ///
    /// The CLI uses the user **session** token for project/secret operations
    /// (the current server authenticates those endpoints with sessions). The
    /// machine-account access token is provisioned and stored as metadata.
    pub fn credential(&self) -> Option<&str> {
        self.session_token.as_deref()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

use crate::error::CliResult;

#[cfg(test)]
mod tests {
    // VTRFIX: the CLI tests mutate a process-global env var (VAUTR_CONFIG),
    // which races under the default parallel test runner. Serialize the
    // affected tests through this mutex.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    use super::*;

    #[test]
    fn default_config_points_at_localhost() {
        assert_eq!(Config::new().server_url, DEFAULT_SERVER_URL);
    }

    #[test]
    fn explicit_server_url_is_kept() {
        let cfg = Config::with_server_url("https://vault.example.com");
        assert_eq!(cfg.server_url, "https://vault.example.com");
    }

    #[test]
    fn honors_vautr_config_env_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let p = std::env::var("VAUTR_CONFIG").ok();
        std::env::set_var("VAUTR_CONFIG", "/tmp/custom-vautr-config.json");
        assert_eq!(
            Config::config_path(),
            PathBuf::from("/tmp/custom-vautr-config.json")
        );
        match p {
            Some(v) => std::env::set_var("VAUTR_CONFIG", v),
            None => std::env::remove_var("VAUTR_CONFIG"),
        }
    }

    #[test]
    fn round_trips_via_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path =
            std::env::temp_dir().join(format!("vautr_cli_cfg_test_{}.json", uuid::Uuid::new_v4()));
        let prev = std::env::var("VAUTR_CONFIG").ok();
        std::env::set_var("VAUTR_CONFIG", path.to_str().unwrap());

        let mut cfg = Config::with_server_url("http://x:8080");
        cfg.session_token = Some("tok".to_string());
        cfg.project_keys
            .insert("p1".to_string(), "a2V5".to_string());
        cfg.save().unwrap();

        let loaded = Config::load().unwrap();
        assert_eq!(loaded.server_url, "http://x:8080");
        assert_eq!(loaded.session_token.as_deref(), Some("tok"));
        assert_eq!(
            loaded.project_keys.get("p1").map(String::as_str),
            Some("a2V5")
        );

        let _ = std::fs::remove_file(&path);
        match prev {
            Some(v) => std::env::set_var("VAUTR_CONFIG", v),
            None => std::env::remove_var("VAUTR_CONFIG"),
        }
    }
}

/// VTRFIX-SEC-H15: write `bytes` to `path` with mode 0600 (unix) using an
/// atomic temp+rename so a crash cannot corrupt the config.
fn write_private_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("json.tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    // Best-effort: enforce 0600 on the final file even if the FS ignored mode
    // on the temp (NFS / certain containers).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
