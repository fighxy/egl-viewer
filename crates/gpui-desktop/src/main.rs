//! Native desktop shell: GPUI Kit + desktop-session (D-126 / D-129 Phase 1 browse).
//!
//! ```text
//! cargo run -p gpui-desktop
//! cargo run -p gpui-desktop -- path\to\Library.eglib
//! cargo run -p gpui-desktop -- --no-reopen
//! ```

mod app;
mod browse;
mod commands;
mod dialogs;
mod i18n;
mod justified;
mod library;
mod platform;
mod screens;
mod theme;

use gpui_kit::assets::Assets;
use gpui_kit::{AppContext as _, WindowBounds, WindowOptions, px, size};

use crate::app::RootApp;
use crate::commands::bind_keymap;
use crate::platform::init_logging;
use crate::theme::apply_theme;

fn main() {
    init_logging();

    let app = gpui_kit::application().with_assets(Assets);
    app.run(move |cx| {
        gpui_kit::init(cx);
        bind_keymap(cx);

        // Theme before first paint (prefs applied again inside RootApp::new).
        apply_theme(app_store::Theme::System, cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1100.), px(720.),), cx)),
            titlebar: Some(Default::default()),
            ..Default::default()
        };

        gpui_kit::open_window(window_options, cx, |window, cx| {
            cx.new(|cx| RootApp::new(window, cx))
        })
        .expect("failed to open GPUI desktop window");
    });
}
