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
mod dnd;
mod export_rules;
mod i18n;
mod justified;
mod library;
mod menus;
mod ops_details;
mod ops_dnd;
mod ops_export;
mod ops_items;
mod ops_mutations;
mod ops_organize;
mod ops_preview;
mod ops_scroll;
mod platform;
#[cfg(windows)]
mod reveal_win;
mod screens;
mod settings_ui;
mod theme;
mod updater;

use gpui_kit::assets::AllAssets;
use gpui_kit::{AppContext as _, Pixels, Size, WindowBounds, WindowOptions, px, size};

use crate::app::RootApp;
use crate::commands::bind_keymap;
use crate::platform::init_logging;
use crate::theme::apply_theme;

/// Smallest GPUI frame, width × height. Not a square. Tauri stays 900×600 (D-138).
pub const MIN_WINDOW_WIDTH: f32 = 960.;
pub const MIN_WINDOW_HEIGHT: f32 = 600.;

const DEFAULT_WINDOW_WIDTH: f32 = 1100.;
const DEFAULT_WINDOW_HEIGHT: f32 = 720.;

/// Raises each axis that is below the minimum. A larger axis stays as it is.
pub fn clamp_window_size(width: f32, height: f32) -> Size<Pixels> {
    size(
        px(width.max(MIN_WINDOW_WIDTH)),
        px(height.max(MIN_WINDOW_HEIGHT)),
    )
}

fn main() {
    init_logging();

    let app = gpui_kit::application().with_assets(AllAssets);
    app.run(move |cx| {
        gpui_kit::init(cx);
        bind_keymap(cx);

        // Theme before first paint (prefs applied again inside RootApp::new).
        apply_theme(app_store::Theme::System, cx);

        let open_size = clamp_window_size(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(open_size, cx)),
            window_min_size: Some(size(px(MIN_WINDOW_WIDTH), px(MIN_WINDOW_HEIGHT))),
            titlebar: Some(Default::default()),
            ..Default::default()
        };

        gpui_kit::open_window(window_options, cx, |window, cx| {
            cx.new(|cx| RootApp::new(window, cx))
        })
        .expect("failed to open GPUI desktop window");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_window_is_960_by_600_not_square() {
        let min = clamp_window_size(0., 0.);
        assert_eq!(min.width, px(960.));
        assert_eq!(min.height, px(600.));
        assert_ne!(min.width, min.height);
    }

    #[test]
    fn smaller_size_clamps_up_on_each_axis() {
        let clamped = clamp_window_size(800., 500.);
        assert_eq!(clamped.width, px(MIN_WINDOW_WIDTH));
        assert_eq!(clamped.height, px(MIN_WINDOW_HEIGHT));
        let wide_short = clamp_window_size(1200., 400.);
        assert_eq!(wide_short.width, px(1200.));
        assert_eq!(wide_short.height, px(MIN_WINDOW_HEIGHT));
        let narrow_tall = clamp_window_size(700., 800.);
        assert_eq!(narrow_tall.width, px(MIN_WINDOW_WIDTH));
        assert_eq!(narrow_tall.height, px(800.));
    }

    #[test]
    fn default_open_size_is_already_above_the_minimum() {
        let open = clamp_window_size(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT);
        assert_eq!(open.width, px(DEFAULT_WINDOW_WIDTH));
        assert_eq!(open.height, px(DEFAULT_WINDOW_HEIGHT));
    }
}
