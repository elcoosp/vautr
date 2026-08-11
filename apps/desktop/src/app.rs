//! Desktop application bootstrap. Starts a tokio runtime on a background
//! thread, opens the GPUI window, and runs the `DesktopView` (which switches
//! between login/register and the vault manager).

use gpui::{AppContext, Application, WindowOptions};
use tokio::runtime::Runtime;

use crate::desktop_view::DesktopView;

/// Server base URL. Read from `VAUTR_API_URL` env var, defaulting to localhost.
pub fn base_url() -> String {
    std::env::var("VAUTR_API_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

/// Entry point used by `cargo run --bin vautr-desktop`.
pub fn run() {
    // Start a multi-thread tokio runtime on the calling (main) thread.
    // GPUI runs its event loop on the same thread inside `Application::run()`.
    // Because `run()` is a blocking FnOnce, the runtime lives for the whole
    // process lifetime.
    let _rt = Runtime::new().expect("start tokio runtime");

    Application::new().run(move |cx| {
        let _ = cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|cx| DesktopView::new(cx))
        });
    });
}
