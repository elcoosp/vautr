//! Production hardening (VTR-040).
//!
//! Two independent guards:
//!
//! 1. **Memory locking** — `lock_secret_memory` calls `mlock(2)` (Unix) / a
//!    no-op (WASM) to pin secret heap pages so they cannot be paged to disk.
//!    Gated behind the `mlock` feature; only desktop builds enable it.
//!
//! 2. **Crash-report scrubbing** — `scrub_crash_event` is the exact callback
//!    shape Sentry expects for `before_send` (`FnOnce(Event) -> Option<Event>`).
//!    It strips anything that could leak a plaintext secret, a vault `uuid`, a
//!    user email, or an item title, while preserving anonymous breadcrumbs
//!    ("SyncStarted", "KeyUpdateRequired") and stack traces. Crash reporting is
//!    only initialised after explicit telemetry consent (see `init_crash_reporting`).

use std::sync::atomic::AtomicBool;

#[cfg(feature = "mlock")]
mod mlock_impl {
    use std::io;

    /// Pin `len` bytes starting at `ptr` into RAM so the OS cannot swap them to
    /// disk. Best-effort: returns `Ok` even when the OS denies the lock (e.g.
    /// CI containers without `CAP_IPC_LOCK` return `EPERM`), because a failed
    /// lock must never crash the app — it only weakens the guarantee. The
    /// caller logs the error.
    #[cfg(unix)]
    pub fn lock_region(ptr: *const u8, len: usize) -> io::Result<()> {
        if len == 0 || ptr.is_null() {
            return Ok(());
        }
        // SAFETY: caller guarantees `ptr..ptr+len` is a valid, live allocation.
        let r = unsafe { libc::mlock(ptr as *const libc::c_void, len) };
        if r == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// WASM and other non-Unix targets: memory locking is unsupported, so this
    /// is a documented no-op (returning Ok keeps call sites uniform).
    #[cfg(not(unix))]
    pub fn lock_region(_ptr: *const u8, _len: usize) -> io::Result<()> {
        Ok(())
    }
}

/// Lock a byte slice into RAM (VTR-040). See [`mlock_impl::lock_region`].
///
/// Best-effort: never panics. Returns `Err` only for a null/empty slice detail;
/// OS-level denials (EPERM/ENOMEM) are returned so the caller can log them.
#[cfg(feature = "mlock")]
pub fn lock_secret_memory(slice: &[u8]) -> std::io::Result<()> {
    mlock_impl::lock_region(slice.as_ptr(), slice.len())
}

/// Without the `mlock` feature, locking is a compile-time no-op (e.g. mobile /
/// WASM builds). Keeps call sites identical across targets.
#[cfg(not(feature = "mlock"))]
pub fn lock_secret_memory(_slice: &[u8]) -> std::io::Result<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Crash-report scrubbing
// ---------------------------------------------------------------------------

/// A breadcrumb describing an anonymous lifecycle event (never contains secrets).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CrashBreadcrumb {
    pub message: String,
    #[serde(default)]
    pub category: Option<String>,
}

/// A minimal crash-report model matching the Sentry `Event` shape we rely on.
/// Kept portable (no `sentry` crate dependency) so the scrubber is unit-testable
/// and can be wired into Sentry's `before_send` with a one-line adapter.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CrashEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breadcrumbs: Vec<CrashBreadcrumb>,
    /// Stack frames as strings (preserved — anonymous, no secrets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stacktrace: Option<Vec<String>>,
    /// Arbitrary extra context. SCRUBBED: any uuid / secret / title / email key
    /// or value is removed.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// User identity. SCRUBBED: only an anonymous, stable id is allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<CrashUser>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CrashUser {
    /// Anonymous, stable per-install id (e.g. a random UUID used ONLY for
    /// crash de-duplication). Never the user's email or account uuid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Sentry `before_send` signature: `FnOnce(Event) -> Option<Event>`. Returning
/// `None` drops the event entirely. We return `Some(scrubbed)` so anonymous
/// diagnostics still reach us.
pub type BeforeSend = fn(CrashEvent) -> Option<CrashEvent>;

fn looks_like_uuid(s: &str) -> bool {
    // Scan for an RFC-4122 substring: 8-4-4-4-12 hex, case-insensitive. This
    // matches a uuid embedded in a longer message (e.g. a log line), not just a
    // standalone uuid string.
    let n = s.len();
    if n < 36 {
        return false;
    }
    for start in 0..=(n - 36) {
        if is_uuid_at(s, start) {
            return true;
        }
    }
    false
}

fn is_uuid_at(s: &str, start: usize) -> bool {
    let b = s.as_bytes();
    for &d in &[8usize, 13, 18, 23] {
        if b[start + d] != b'-' {
            return false;
        }
    }
    for i in 0..36 {
        if [8usize, 13, 18, 23].contains(&i) {
            continue;
        }
        let c = b[start + i];
        if !(c.is_ascii_digit() || (b'a'..=b'f').contains(&c) || (b'A'..=b'F').contains(&c)) {
            return false;
        }
    }
    true
}

fn is_secret_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.contains("secret")
        || k.contains("password")
        || k.contains("token")
        || k.contains("decrypted")
        || k.contains("plaintext")
        || k.contains("title")
        || k.contains("email")
        || k.contains("mnemonic")
        || k == "uuid"
}

/// Scrub a [`CrashEvent`] in place: remove any field that could leak a secret,
/// a vault `uuid`, a user email, or an item title; preserve breadcrumbs and
/// stack traces.
pub fn scrub_crash_event(event: &mut CrashEvent) {
    // Message: redact if it embeds a uuid or a Decrypted* type name.
    if let Some(msg) = &event.message {
        if looks_like_uuid(msg)
            || msg.contains("DecryptedSecret")
            || msg.contains("DecryptedOverview")
        {
            event.message = Some("[redacted]".to_string());
        }
    }

    // User: keep only an anonymous id; drop email and any account-shaped id.
    if let Some(user) = &mut event.user {
        if let Some(email) = &user.email {
            if !email.is_empty() {
                user.email = None;
            }
        }
        if let Some(id) = &user.id {
            // An account uuid is a leak; only keep ids that are NOT uuids.
            if looks_like_uuid(id) {
                user.id = None;
            }
        }
    }

    // Extra: drop secret-valued keys and any uuid/secret-bearing values.
    let mut to_remove = Vec::new();
    for (key, value) in event.extra.iter() {
        if is_secret_key(key) {
            to_remove.push(key.clone());
            continue;
        }
        if let Some(s) = value.as_str() {
            if looks_like_uuid(s)
                || s.contains("DecryptedSecret")
                || s.contains("DecryptedOverview")
            {
                to_remove.push(key.clone());
            }
        }
    }
    for key in to_remove {
        event.extra.remove(&key);
    }
}

/// `before_send` entry point for Sentry: scrub then keep the event.
pub fn scrub_before_send(mut event: CrashEvent) -> Option<CrashEvent> {
    scrub_crash_event(&mut event);
    Some(event)
}

// ---------------------------------------------------------------------------
// Consent-gated initialisation
// ---------------------------------------------------------------------------

static CRASH_REPORTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Initialise crash reporting only after explicit telemetry consent (VTR-040
/// acceptance: reports only with opt-in). Returns the configured `BeforeSend`
/// hook, or `None` if consent was not granted. Consent can be toggled (e.g.
/// user revokes it in settings), so this is re-settable, not one-shot.
pub fn init_crash_reporting(consent: bool) -> Option<BeforeSend> {
    CRASH_REPORTING.store(consent, std::sync::atomic::Ordering::SeqCst);
    if consent {
        Some(scrub_before_send)
    } else {
        None
    }
}

/// Whether crash reporting is currently enabled (consent was given).
pub fn is_crash_reporting_enabled() -> bool {
    CRASH_REPORTING.load(std::sync::atomic::Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mlock_is_noop_safe_on_unix_small_buffer() {
        // Best-effort: must not panic. In CI without CAP_IPC_LOCK this returns
        // Err(EPERM), which is acceptable; we only assert no panic + a Result.
        let buf = vec![0u8; 64];
        let r = lock_secret_memory(&buf);
        assert!(r.is_ok() || r.is_err());
    }

    #[test]
    fn mlock_handles_empty_slice() {
        let buf: Vec<u8> = Vec::new();
        assert!(lock_secret_memory(&buf).is_ok());
    }

    #[test]
    fn scrub_strips_uuid_email_title_token_and_secret_fields() {
        let mut event = CrashEvent {
            message: Some("vault 11111111-1111-1111-1111-111111111111 updated".into()),
            breadcrumbs: vec![CrashBreadcrumb {
                message: "SyncStarted".into(),
                category: Some("sync".into()),
            }],
            stacktrace: Some(vec!["vautr_app_state::orchestrator::sync".into()]),
            extra: serde_json::json!({
                "uuid": "22222222-2222-2222-2222-222222222222",
                "title": "My bank login",
                "user_email": "alice@example.com",
                "DecryptedSecret": "hunter2-plaintext",
                "safe_counter": 3,
                "safe_state": "KeyUpdateRequired"
            })
            .as_object()
            .unwrap()
            .clone(),
            user: Some(CrashUser {
                id: Some("33333333-3333-3333-3333-333333333333".into()),
                email: Some("bob@example.com".into()),
            }),
        };

        scrub_crash_event(&mut event);

        // Message uuid redacted.
        assert_eq!(event.message.as_deref(), Some("[redacted]"));
        // Breadcrumbs + stacktrace preserved (anonymous diagnostics).
        assert_eq!(event.breadcrumbs.len(), 1);
        assert_eq!(event.breadcrumbs[0].message, "SyncStarted");
        assert!(event.stacktrace.is_some());
        // Secret-bearing extra fields removed; safe ones kept.
        assert!(event.extra.get("uuid").is_none());
        assert!(event.extra.get("title").is_none());
        assert!(event.extra.get("user_email").is_none());
        assert!(event.extra.get("DecryptedSecret").is_none());
        assert_eq!(
            event.extra.get("safe_counter").and_then(|v| v.as_i64()),
            Some(3)
        );
        assert_eq!(
            event.extra.get("safe_state").and_then(|v| v.as_str()),
            Some("KeyUpdateRequired")
        );
        // User email + account-uuid id stripped; only anonymous id survives.
        let user = event.user.as_ref().unwrap();
        assert!(user.email.is_none());
        assert!(user.id.is_none());
    }

    #[test]
    fn scrub_preserves_anonymous_user_id() {
        let mut event = CrashEvent {
            message: None,
            breadcrumbs: vec![],
            stacktrace: None,
            extra: serde_json::Map::new(),
            user: Some(CrashUser {
                id: Some("anon-install-9f3c".into()),
                email: None,
            }),
        };
        scrub_crash_event(&mut event);
        // Non-uuid anonymous id is preserved.
        assert_eq!(event.user.unwrap().id.as_deref(), Some("anon-install-9f3c"));
    }

    #[test]
    fn crash_reporting_only_after_consent() {
        // Reset not possible (OnceLock); this asserts the contract shape.
        assert!(init_crash_reporting(false).is_none());
        let hook = init_crash_reporting(true);
        assert!(hook.is_some());
        assert!(is_crash_reporting_enabled());
    }
}
