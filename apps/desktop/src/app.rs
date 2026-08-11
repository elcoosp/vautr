//! Desktop application bootstrap: opens the main window hosting the
//! `VaultManagerView` and runs the GPUI event loop.

use gpui::{AppContext, Application, WindowOptions};
use gpui_component::input::InputState;
use crate::VaultManagerView;

/// Entry point used by `cargo run --bin vautr-desktop`.
pub fn run() {
    Application::new().run(|cx| {
        let _ = cx.open_window(WindowOptions::default(), |window, cx| {
            // gpui-component `InputState` needs a window to construct; build the
            // search box here and hand it to the view.
            let search_state = cx.new(|cx| {
                InputState::new(window, cx).placeholder("Search vault")
            });
            cx.new(|cx| VaultManagerView::new(cx, search_state))
        });
    });
}
