//! App preferences and the window's last size and position (docs/spec.md §7.4, D-022, D-027).

use library_core::{SortKey, SourceError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AppStore;
use crate::json_file::JsonFile;

const PREFERENCES: &str = "preferences.json";
const WINDOW: &str = "window.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    /// Follow Windows.
    #[default]
    System,
    Light,
    Dark,
}

/// Widths and open state of the left and right bars (docs/spec.md §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct UiLayout {
    pub left_width: u32,
    pub right_width: u32,
    pub left_open: bool,
    pub right_open: bool,
}

impl Default for UiLayout {
    fn default() -> Self {
        Self {
            left_width: 240,
            right_width: 320,
            left_open: true,
            right_open: true,
        }
    }
}

impl UiLayout {
    /// Keeps widths inside the ranges of spec §7.3.
    pub fn clamped(self) -> Self {
        Self {
            left_width: self.left_width.clamp(200, 360),
            right_width: self.right_width.clamp(280, 480),
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

/// How the library screen shows items, the same in every library (docs/spec.md §5, §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct BrowsePreferences {
    pub view_mode: ViewMode,
    /// The width of a grid cell in CSS pixels.
    pub thumbnail_size: u32,
    pub show_names: bool,
    /// The sort of every screen without its own (a folder's sort override comes in phase 4).
    pub sort: SortKey,
    pub ascending: bool,
    /// A folder also shows the items of its subfolders, and its count includes them.
    pub include_subfolders: bool,
}

impl BrowsePreferences {
    pub const MIN_THUMBNAIL: u32 = 80;
    pub const MAX_THUMBNAIL: u32 = 320;

    pub fn clamped(self) -> Self {
        Self {
            thumbnail_size: self
                .thumbnail_size
                .clamp(Self::MIN_THUMBNAIL, Self::MAX_THUMBNAIL),
            ..self
        }
    }
}

impl Default for BrowsePreferences {
    /// A grid of 160 px cells with names, newest first.
    fn default() -> Self {
        Self {
            view_mode: ViewMode::Grid,
            thumbnail_size: 160,
            show_names: true,
            sort: SortKey::DateAdded,
            ascending: false,
            include_subfolders: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Preferences {
    pub theme: Theme,
    pub reopen_last_library: bool,
    /// Where Create library starts; `None` means `%USERPROFILE%\egl-library` (D-021).
    pub default_library_parent: Option<String>,
    pub layout: UiLayout,
    pub browse: BrowsePreferences,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            reopen_last_library: true,
            default_library_parent: None,
            layout: UiLayout::default(),
            browse: BrowsePreferences::default(),
        }
    }
}

/// The window's last outer position and inner size, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl AppStore {
    pub fn preferences(&self) -> Preferences {
        JsonFile::new(self.dir(), PREFERENCES).load()
    }

    /// Stores `preferences` as given; the caller validates the default folder first.
    pub fn save_preferences(&self, preferences: &Preferences) -> Result<(), SourceError> {
        let mut preferences = preferences.clone();
        preferences.layout = preferences.layout.clamped();
        preferences.browse = preferences.browse.clamped();
        JsonFile::new(self.dir(), PREFERENCES).update(|stored: &mut Preferences| {
            *stored = preferences;
        })
    }

    pub fn window_state(&self) -> Option<WindowState> {
        let state: Option<WindowState> = JsonFile::new(self.dir(), WINDOW).load();
        state.filter(|state| state.width >= 400 && state.height >= 300)
    }

    pub fn save_window_state(&self, state: WindowState) -> Result<(), SourceError> {
        JsonFile::new(self.dir(), WINDOW).update(|stored: &mut Option<WindowState>| {
            *stored = Some(state);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_windows_and_reopen_the_last_library() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        let preferences = store.preferences();
        assert_eq!(preferences.theme, Theme::System);
        assert!(preferences.reopen_last_library);
        assert_eq!(preferences.default_library_parent, None);
        assert_eq!(preferences.layout, UiLayout::default());
        assert_eq!(preferences.browse.view_mode, ViewMode::Grid);
        assert_eq!(preferences.browse.sort, SortKey::DateAdded);
        assert!(!preferences.browse.ascending);
    }

    #[test]
    fn preferences_round_trip_with_clamped_widths() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        let preferences = Preferences {
            theme: Theme::Dark,
            reopen_last_library: false,
            default_library_parent: Some("D:\\Libraries".into()),
            layout: UiLayout {
                left_width: 50,
                right_width: 999,
                left_open: false,
                right_open: true,
            },
            browse: BrowsePreferences {
                view_mode: ViewMode::List,
                thumbnail_size: 1000,
                show_names: false,
                sort: SortKey::Title,
                ascending: true,
                include_subfolders: true,
            },
        };
        store.save_preferences(&preferences).unwrap();
        let stored = store.preferences();
        assert_eq!(stored.theme, Theme::Dark);
        assert_eq!(
            stored.default_library_parent.as_deref(),
            Some("D:\\Libraries")
        );
        assert_eq!(
            (stored.layout.left_width, stored.layout.right_width),
            (200, 480)
        );
        assert!(!stored.layout.left_open);
        assert_eq!(
            stored.browse,
            BrowsePreferences {
                thumbnail_size: BrowsePreferences::MAX_THUMBNAIL,
                ..preferences.browse
            }
        );
    }

    #[test]
    fn missing_fields_keep_their_defaults() {
        let preferences: Preferences = serde_json::from_str(r#"{ "theme": "light" }"#).unwrap();
        assert_eq!(preferences.theme, Theme::Light);
        assert!(preferences.reopen_last_library);
    }

    #[test]
    fn window_state_round_trips_and_tiny_windows_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        assert_eq!(store.window_state(), None);
        let state = WindowState {
            x: 10,
            y: 20,
            width: 1280,
            height: 800,
            maximized: true,
        };
        store.save_window_state(state).unwrap();
        assert_eq!(store.window_state(), Some(state));
        store
            .save_window_state(WindowState { width: 10, ..state })
            .unwrap();
        assert_eq!(store.window_state(), None);
    }
}
