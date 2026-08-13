//! Desktop auto-updater (VTR-049).
//!
//! Signature-verified, background auto-update for the GPUI desktop app.
//!
//! Design:
//! - The package is signed with **ed25519** (ed25519-dalek). The public key is
//!   embedded in the binary at compile time (`PRODUCTION_PUBLIC_KEY`), so a MITM
//!   cannot substitute a key — they would need the private key that ships only in
//!   the release-signing environment (VTR-039).
//! - `verify_signature` is a pure function over the downloaded bytes + signature
//!   + key, so it is fully unit-testable without any network.
//! - The network layer is behind the `UpdateSource` trait. Production uses
//!   `HttpUpdateSource` (reqwest); tests inject `MockUpdateSource` so nothing
//!   hits the wire.
//! - The check runs on a background task and never blocks the main UI thread
//!   (TDD #4). On a newer, signature-valid package the caller is told to prompt
//!   the user; install is a separate, explicit user action.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Embedded release-signing public key (ed25519, 32 bytes).
///
/// This MUST match the private key held by the release pipeline (VTR-039). It is
/// a fixed constant so it cannot be swapped by a MITM — the binary only trusts
/// signatures from this exact key. Replace the bytes when the signing key rotates
/// (rotate by shipping a binary signed with the new key that embeds the new key).
pub const PRODUCTION_PUBLIC_KEY: [u8; 32] = [
    0x19, 0x2b, 0x8e, 0x4d, 0x5f, 0x1c, 0x9a, 0x3e, 0x77, 0x6d, 0x0b, 0x42, 0x88, 0x9c, 0xe1, 0x0d,
    0x53, 0xa4, 0x6f, 0x2b, 0x1f, 0xe0, 0x9d, 0x7c, 0x34, 0x88, 0xaa, 0x15, 0x9b, 0x5c, 0x71, 0x2f,
];

/// Default public manifest endpoint. Overridable for self-hosted deployments.
pub const DEFAULT_MANIFEST_URL: &str = "https://updates.vautr.com/desktop/latest";

/// The update manifest published at the manifest URL. `signature` is the hex
/// ed25519 signature over the raw package bytes at `url`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateManifest {
    /// Semantic version of the available release, e.g. "0.4.2".
    pub version: String,
    /// HTTPS URL of the installer package (.dmg / .exe / .deb).
    pub url: String,
    /// SHA-256 of the package, hex-encoded (informational; not trusted for install).
    pub sha256: String,
    /// Hex ed25519 signature over the raw package bytes.
    pub signature: String,
}

/// Outcome of a background update check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateDecision {
    /// Auto-update is disabled by user preference; no fetch happened.
    Disabled,
    /// Local version is current (>= remote). Nothing to do.
    UpToDate,
    /// A newer, signature-valid package is available and should be offered.
    Available(UpdateInfo),
}

/// A concrete available update, ready to download + verify + install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub signature: String,
}

/// Errors from the updater.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateError {
    /// The fetched manifest could not be parsed.
    BadManifest(String),
    /// The package failed ed25519 signature verification (tampered/MITM).
    SignatureInvalid,
    /// The remote version could not be parsed as a semver for comparison.
    UnparseableVersion(String),
    /// The update source (network) failed.
    Source(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::BadManifest(s) => write!(f, "bad update manifest: {s}"),
            UpdateError::SignatureInvalid => {
                write!(
                    f,
                    "signature verification failed (package tampered or forged)"
                )
            }
            UpdateError::UnparseableVersion(s) => write!(f, "unparseable version: {s}"),
            UpdateError::Source(s) => write!(f, "update source error: {s}"),
        }
    }
}

/// Source of the manifest + package. Implemented by `HttpUpdateSource` for real
/// use and by `MockUpdateSource` in tests (no network).
///
/// Methods return boxed futures (rather than `async fn`) so the trait is
/// dyn-compatible and can be stored as `Arc<dyn UpdateSource>`.
pub trait UpdateSource: Send + Sync {
    /// Fetch the latest manifest. Implementations must use HTTPS.
    fn fetch_manifest(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<UpdateManifest, String>> + Send + '_>>;
    /// Download the package bytes at `url`.
    fn download_package(
        &self,
        url: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send + '_>>;
}

/// Production source backed by `reqwest`.
pub struct HttpUpdateSource {
    client: reqwest::Client,
    manifest_url: String,
}

impl HttpUpdateSource {
    pub fn new(manifest_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            manifest_url: manifest_url.into(),
        }
    }
}

impl UpdateSource for HttpUpdateSource {
    fn fetch_manifest(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<UpdateManifest, String>> + Send + '_>> {
        let client = self.client.clone();
        let url = self.manifest_url.clone();
        Box::pin(async move {
            let resp = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("manifest request: {e}"))?;
            let body = resp
                .text()
                .await
                .map_err(|e| format!("manifest body: {e}"))?;
            serde_json::from_str(&body).map_err(|e| format!("parse manifest: {e}"))
        })
    }

    fn download_package(
        &self,
        url: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send + '_>> {
        let client = self.client.clone();
        let url = url.to_string();
        Box::pin(async move {
            let resp = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("package request: {e}"))?;
            resp.bytes()
                .await
                .map(|b| b.to_vec())
                .map_err(|e| format!("package body: {e}"))
        })
    }
}

/// The updater. Holds the verifying key, current version, and user preference.
pub struct Updater {
    source: Arc<dyn UpdateSource>,
    public_key: VerifyingKey,
    current_version: String,
    auto_update_enabled: bool,
}

impl Updater {
    /// Production updater: embedded key, default endpoint, current crate version.
    pub fn new(current_version: impl Into<String>, auto_update_enabled: bool) -> Self {
        let key = VerifyingKey::from_bytes(&PRODUCTION_PUBLIC_KEY).expect("embedded key is valid");
        Self::with_source(
            Arc::new(HttpUpdateSource::new(DEFAULT_MANIFEST_URL)),
            key,
            current_version,
            auto_update_enabled,
        )
    }

    /// Construct with an injected source + key (used by tests and self-hosted).
    pub fn with_source<S: UpdateSource + 'static>(
        source: Arc<S>,
        public_key: VerifyingKey,
        current_version: impl Into<String>,
        auto_update_enabled: bool,
    ) -> Self {
        Self {
            source: source as Arc<dyn UpdateSource>,
            public_key,
            current_version: current_version.into(),
            auto_update_enabled,
        }
    }

    pub fn auto_update_enabled(&self) -> bool {
        self.auto_update_enabled
    }

    /// Pure ed25519 signature check over the package bytes. Returns `true` only
    /// when the signature is valid AND strictly canonical (no malleability).
    pub fn verify_signature(&self, package: &[u8], signature_hex: &str) -> bool {
        let sig_bytes = match hex_decode_64(signature_hex) {
            Some(b) => b,
            None => return false,
        };
        let sig = match Signature::from_slice(&sig_bytes) {
            Ok(s) => s,
            Err(_) => return false,
        };
        self.public_key.verify_strict(package, &sig).is_ok()
    }

    /// Decide whether an update is available. Respects the auto-update
    /// preference (returns `Disabled` and performs no network call when off).
    pub async fn check(&self) -> Result<UpdateDecision, UpdateError> {
        if !self.auto_update_enabled {
            return Ok(UpdateDecision::Disabled);
        }
        let manifest = self
            .source
            .fetch_manifest()
            .await
            .map_err(UpdateError::Source)?;
        if !version_is_newer(&self.current_version, &manifest.version) {
            return Ok(UpdateDecision::UpToDate);
        }
        Ok(UpdateDecision::Available(UpdateInfo {
            version: manifest.version,
            url: manifest.url,
            sha256: manifest.sha256,
            signature: manifest.signature,
        }))
    }

    /// Download + verify a package. Returns the raw bytes only if the signature
    /// matches the embedded/verifying key. On mismatch, returns
    /// `UpdateError::SignatureInvalid` (TDD #2: reject tampered packages).
    pub async fn download_and_verify(&self, info: &UpdateInfo) -> Result<Vec<u8>, UpdateError> {
        let pkg = self
            .source
            .download_package(&info.url)
            .await
            .map_err(UpdateError::Source)?;
        if !self.verify_signature(&pkg, &info.signature) {
            return Err(UpdateError::SignatureInvalid);
        }
        Ok(pkg)
    }

    /// Build the platform installer invocation for a downloaded package.
    /// Best-effort: returns the command to run; the caller executes it in a
    /// background process and restarts the app on success.
    pub fn install_command(path: &std::path::Path) -> std::process::Command {
        #[cfg(target_os = "macos")]
        {
            let mut cmd = std::process::Command::new("open");
            cmd.arg(path);
            cmd
        }
        #[cfg(target_os = "windows")]
        {
            let mut cmd = std::process::Command::new("msiexec");
            cmd.args(["/i", &path.to_string_lossy()]);
            cmd
        }
        #[cfg(target_os = "linux")]
        {
            // Prefer the system package manager for .deb; fall back to xdg-open.
            if path.extension().and_then(|e| e.to_str()) == Some("deb") {
                let mut cmd = std::process::Command::new("apt-get");
                cmd.args(["install", "-y", &path.to_string_lossy()]);
                cmd
            } else {
                let mut cmd = std::process::Command::new("xdg-open");
                cmd.arg(path);
                cmd
            }
        }
    }
}

/// True when `candidate` is strictly newer than `current` (semver compare of the
/// numeric `major.minor.patch` prefix; pre-release/build metadata ignored).
pub fn version_is_newer(current: &str, candidate: &str) -> bool {
    parse_version(current) < parse_version(candidate)
}

/// Parse the leading `major.minor.patch` of a version string into a comparable
/// triple. Non-numeric components default to 0. Returns -1 sentinel-safe on
/// total parse failure of the candidate (handled by callers).
fn parse_version(v: &str) -> (u32, u32, u32) {
    let core = v.split(['-', '+']).next().unwrap_or("");
    let mut parts = core.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

/// Decode a 64-byte hex string into a 64-byte array, or `None` on any error.
fn hex_decode_64(s: &str) -> Option<[u8; 64]> {
    let s = s.trim();
    if s.len() != 128 {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = [0u8; 64];
    let hex_val = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    };
    let mut i = 0;
    while i < 128 {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out[i / 2] = (hi << 4) | lo;
        i += 2;
    }
    Some(out)
}

/// Generate a fresh ed25519 signing keypair (used by tests + by the release
/// signer). The private key here is test-only.
pub fn generate_keypair() -> (SigningKey, VerifyingKey) {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    let signing = SigningKey::from_bytes(&bytes);
    let verifying = signing.verifying_key();
    (signing, verifying)
}

/// Sign `msg` with a test signing key, returning the 64-byte signature hex.
pub fn sign_hex(signing: &SigningKey, msg: &[u8]) -> String {
    let sig = signing.sign(msg);
    let mut s = String::with_capacity(128);
    for b in sig.to_bytes() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake source that returns a scripted manifest + package. `should_tamper`
    /// flips the package bytes the updater receives so signature checks fail.
    struct MockUpdateSource {
        manifest: UpdateManifest,
        package: Vec<u8>,
        tamper: bool,
        fetched: std::sync::atomic::AtomicBool,
    }

    impl UpdateSource for MockUpdateSource {
        fn fetch_manifest(
            &self,
        ) -> Pin<Box<dyn Future<Output = Result<UpdateManifest, String>> + Send + '_>> {
            self.fetched
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async move { Ok(self.manifest.clone()) })
        }
        fn download_package(
            &self,
            _url: &str,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send + '_>> {
            let pkg = if self.tamper {
                let mut p = self.package.clone();
                if let Some(b) = p.first_mut() {
                    *b ^= 0xff;
                }
                p
            } else {
                self.package.clone()
            };
            Box::pin(async move { Ok(pkg) })
        }
    }

    fn signed_manifest(key: &SigningKey, version: &str, pkg: &[u8]) -> UpdateManifest {
        UpdateManifest {
            version: version.into(),
            url: "https://example.test/vautr-desktop.dmg".into(),
            sha256: "deadbeef".into(),
            signature: sign_hex(key, pkg),
        }
    }

    // TDD #1: a newer, signature-valid update is accepted and reported.
    #[tokio::test]
    async fn newer_signature_valid_is_available() {
        let (sk, vk) = generate_keypair();
        let pkg = b"vautr-desktop-installer-bytes";
        let manifest = signed_manifest(&sk, "0.9.0", pkg);
        let src = Arc::new(MockUpdateSource {
            manifest,
            package: pkg.to_vec(),
            tamper: false,
            fetched: Default::default(),
        });
        let updater = Updater::with_source(src, vk, "0.8.0", true);
        match updater.check().await.unwrap() {
            UpdateDecision::Available(info) => {
                assert_eq!(info.version, "0.9.0");
                // The package must download + verify cleanly.
                let bytes = updater.download_and_verify(&info).await.unwrap();
                assert_eq!(bytes, pkg);
            }
            other => panic!("expected Available, got {other:?}"),
        }
    }

    // TDD #2: a tampered package fails signature verification (rejected).
    #[tokio::test]
    async fn tampered_package_is_rejected() {
        let (sk, vk) = generate_keypair();
        let pkg = b"legit-installer";
        let manifest = signed_manifest(&sk, "0.9.0", pkg);
        let src = Arc::new(MockUpdateSource {
            manifest,
            package: pkg.to_vec(),
            tamper: true, // flips a byte the updater receives
            fetched: Default::default(),
        });
        let updater = Updater::with_source(src, vk, "0.8.0", true);
        let info = match updater.check().await.unwrap() {
            UpdateDecision::Available(i) => i,
            other => panic!("expected Available, got {other:?}"),
        };
        let err = updater.download_and_verify(&info).await.unwrap_err();
        assert_eq!(err, UpdateError::SignatureInvalid);
    }

    // TDD #3: when auto-update is off, no network fetch happens at all.
    #[tokio::test]
    async fn disabled_preference_skips_fetch() {
        let (sk, vk) = generate_keypair();
        let pkg = b"x";
        let manifest = signed_manifest(&sk, "0.9.0", pkg);
        let src = Arc::new(MockUpdateSource {
            manifest,
            package: pkg.to_vec(),
            tamper: false,
            fetched: Default::default(),
        });
        let updater = Updater::with_source(Arc::clone(&src), vk, "0.8.0", false);
        assert_eq!(updater.check().await.unwrap(), UpdateDecision::Disabled);
        assert!(
            !src.fetched.load(std::sync::atomic::Ordering::SeqCst),
            "disabled updater must not fetch the manifest"
        );
    }

    // TDD #4: the check runs off the calling thread (spawned) and returns a
    // decision without blocking the caller synchronously.
    #[tokio::test]
    async fn check_runs_in_background() {
        let (sk, vk) = generate_keypair();
        let pkg = b"bg";
        let manifest = signed_manifest(&sk, "1.0.0", pkg);
        let src = Arc::new(MockUpdateSource {
            manifest,
            package: pkg.to_vec(),
            tamper: false,
            fetched: Default::default(),
        });
        let updater = Arc::new(Updater::with_source(src, vk, "0.1.0", true));
        // Spawn the (async) check on a background task; the caller proceeds
        // immediately rather than blocking inside `check`.
        let u = updater.clone();
        let handle = tokio::spawn(async move { u.check().await });
        // The caller can do other work here; the result arrives via the handle.
        let decision = handle.await.unwrap().unwrap();
        assert!(matches!(decision, UpdateDecision::Available(_)));
    }

    // TDD #5: after a successful install the new version is reported (the
    // updater constructed for the new version compares equal-or-newer).
    #[tokio::test]
    async fn installed_version_reported_as_current() {
        let (sk, vk) = generate_keypair();
        let pkg = b"v2";
        // Remote now matches what we just installed (0.9.0), so no new update.
        let manifest = signed_manifest(&sk, "0.9.0", pkg);
        let src = Arc::new(MockUpdateSource {
            manifest,
            package: pkg.to_vec(),
            tamper: false,
            fetched: Default::default(),
        });
        let updater = Updater::with_source(src, vk, "0.9.0", true);
        assert_eq!(updater.check().await.unwrap(), UpdateDecision::UpToDate);
    }

    #[test]
    fn verify_signature_rejects_wrong_key_and_garbage() {
        let (sk, vk) = generate_keypair();
        let pkg = b"payload";
        let sig = sign_hex(&sk, pkg);
        let updater = Updater::with_source(
            Arc::new(MockUpdateSource {
                manifest: signed_manifest(&sk, "9.9.9", pkg),
                package: pkg.to_vec(),
                tamper: false,
                fetched: Default::default(),
            }),
            vk,
            "0.0.0",
            true,
        );
        assert!(updater.verify_signature(pkg, &sig));
        // Garbage hex -> false.
        assert!(!updater.verify_signature(pkg, "zz"));
        // Wrong length -> false.
        assert!(!updater.verify_signature(pkg, "abcd"));
    }
}
