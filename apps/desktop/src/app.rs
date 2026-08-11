//! Desktop application bootstrap.
//!
//! Starts the GPUI event loop and opens the main window with `DesktopView`,
//! wrapped in a gpui-component `Root`.

use crate::desktop_view::DesktopView;
use gpui::*;
use gpui_component::Root;
use gpui_platform::application;

/// Server base URL. Read from `VAUTR_API_URL` env var, defaulting to localhost.
pub fn base_url() -> String {
    std::env::var("VAUTR_API_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

/// Entry point used by `cargo run --bin vautr-desktop`.
pub fn run() {
    application().run(move |cx: &mut App| {
        // Must be called before any gpui-component widgets are used.
        gpui_component::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| DesktopView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}
