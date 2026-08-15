//! Desktop application bootstrap.
//!
//! Starts the GPUI event loop and opens the main window with `DesktopView`,
//! wrapped in a gpui-component `Root`.

use crate::desktop_view::DesktopView;
use gpui::*;
use gpui_component::{Root, Theme, ThemeMode, ThemeRegistry};
use gpui_component_assets::Assets;
use gpui_platform::application;

/// Server base URL. Read from `VAUTR_API_URL` env var, defaulting to localhost.
pub fn base_url() -> String {
    std::env::var("VAUTR_API_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

/// Entry point used by `cargo run --bin vautr-desktop`.
pub fn run() {
    // Register the icon/asset bundle BEFORE the event loop starts. Without
    // this, `IconName::*` SVGs have no AssetSource and render as nothing —
    // the entire app loses its visual hierarchy (VTR-089).
    application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            // Must be called before any gpui-component widgets are used.
            gpui_component::init(cx);

            // Load the Vautr Ledger theme and make dark the operating default,
            // regardless of the host OS appearance. Widgets (buttons, inputs,
            // tabs) then draw the same emerald-teal + graphite world as web/mobile.
            ThemeRegistry::global_mut(cx)
                .load_themes_from_str(include_str!("vautr-theme.json"))
                .expect("invalid Vautr theme json");
            if let Some(theme) = ThemeRegistry::global(cx)
                .themes()
                .get("Vautr Dark")
                .cloned()
            {
                Theme::global_mut(cx).dark_theme = theme;
            }
            Theme::change(ThemeMode::Dark, None, cx);

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
