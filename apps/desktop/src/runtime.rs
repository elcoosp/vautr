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
