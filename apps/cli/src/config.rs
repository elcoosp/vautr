//! Client configuration for the Vautr CLI.
//!
//! Wave 0.3 keeps this a minimal safe container. Persistence (config file
//! discovery, machine-account credentials, server URL, active profile) lands in
//! Wave B4 once the machine-account/token model (Wave A2) and client SDK exist.

/// The Vautr server base URL. Wave B4 replaces this with config-file resolution.
pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8080";

/// Planned, lightweight in-memory configuration snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Base URL of the Vautr server this CLI talks to.
    pub server_url: String,
}

impl Config {
    /// Construct a configuration backed by the default server URL.
    pub fn new() -> Self {
        Self {
            server_url: DEFAULT_SERVER_URL.to_string(),
        }
    }

    /// Construct a configuration with an explicit server URL.
    pub fn with_server_url(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
}
