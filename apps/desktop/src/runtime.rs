//! Desktop Tokio runtime.
//!
//! GPUI's `cx.spawn` runs on GPUI's own executor, which is **not** a Tokio
//! runtime. The desktop's HTTP layer (`reqwest`/`hyper`) needs a Tokio reactor,
//! so any live network call made directly inside `cx.spawn` panics with
//! "there is no reactor running" (VTR: desktop live-server).
//!
//! `runtime::enter()` is a one-line guard placed at the top of each
//! network-bearing `cx.spawn` async block. It makes the shared multi-threaded
//! runtime the thread-local Tokio context for the rest of the block, so
//! `reqwest` futures resolve against its reactor. Because GPUI's spawn closure
//! is not `Send`-bound, the `EnterGuard` may be held across `.await` points.

use std::sync::OnceLock;

/// The single desktop Tokio runtime shared by every HTTP operation.
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn rt() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build the Vautr desktop Tokio runtime")
    })
}

/// Enter the desktop Tokio runtime for the current scope. Bind the returned
/// guard to a local (e.g. `let _rt = crate::runtime::enter();`) so it stays
/// alive across the block's `.await` points, giving `reqwest` a reactor.
pub fn enter() -> tokio::runtime::EnterGuard<'static> {
    rt().enter()
}

#[cfg(test)]
mod tests {
    use super::*;

    // VTR-087 regression: GPUI's `cx.spawn` runs on GPUI's own executor, which
    // is NOT a Tokio runtime. `reqwest`'s DNS resolver calls
    // `tokio::runtime::Handle::current()` — on a foreign thread with no Tokio
    // context that panics ("no reactor running"), aborting the whole desktop
    // app (SIGABRT) on every register/login (the update check fires
    // unconditionally). `enter()` installs the shared multi-threaded runtime as
    // the thread-local Tokio context so the resolver finds a reactor.
    //
    // This test mimics that exact condition: a fresh OS thread with no Tokio
    // context. Without `enter()`, `Handle::current()` would panic and `join()`
    // would return `Err`. With it, the call succeeds.
    #[test]
    fn enter_installs_tokio_context_on_foreign_thread() {
        let handle = std::thread::spawn(|| {
            let _rt = crate::runtime::enter();
            // Must not panic on a non-tokio-spawned thread.
            let _flavor = tokio::runtime::Handle::current().runtime_flavor();
        });
        assert!(
            handle.join().is_ok(),
            "runtime::enter() failed to install a Tokio context on a foreign thread \
             (this is what caused the VTR-087 SIGABRT in check_for_updates)"
        );
    }
}
