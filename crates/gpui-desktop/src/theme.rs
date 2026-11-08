//! Map product [`Theme`] prefs onto GPUI Kit [`ThemeMode`] (D-129).

use app_store::Theme;
use gpui_kit::App;
use gpui_kit::component::theme::{Theme as KitTheme, ThemeMode};

/// Applies the stored preference. `System` follows the OS appearance.
pub fn apply_theme(preference: Theme, cx: &mut App) {
    match preference {
        Theme::System => KitTheme::sync_system_appearance(None, cx),
        Theme::Light => KitTheme::change(ThemeMode::Light, None, cx),
        Theme::Dark => KitTheme::change(ThemeMode::Dark, None, cx),
    }
}

pub fn theme_label_key(theme: Theme) -> &'static str {
    match theme {
        Theme::System => "themes.system",
        Theme::Light => "themes.light",
        Theme::Dark => "themes.dark",
    }
}
