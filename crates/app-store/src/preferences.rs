//! App preferences and the window's last size and position (docs/spec.md §7.4, D-022, D-027).

use library_core::{SortKey, SourceError};
use serde::{Deserialize, Serialize};

use crate::AppStore;
use crate::json_file::JsonFile;

const PREFERENCES: &str = "preferences.json";
const WINDOW: &str = "window.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Follow Windows.
    #[default]
    System,
    Light,
    Dark,
}

/// The accent colour (D-182): Windows' own, one of the presets, or any `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    /// Read the accent Windows itself uses; a fallback colour elsewhere.
    #[default]
    Windows,
    Blue,
    Teal,
    Green,
    Yellow,
    Orange,
    Red,
    Purple,
    Pink,
    Custom(String),
}

/// The preset accents, in the order Settings shows them; the folder palette's colours (D-159).
pub const ACCENT_PRESETS: [Accent; 8] = [
    Accent::Blue,
    Accent::Teal,
    Accent::Green,
    Accent::Yellow,
    Accent::Orange,
    Accent::Red,
    Accent::Purple,
    Accent::Pink,
];

impl Accent {
    /// The colour as `[r, g, b]`; `None` for Windows and for a custom value that is not hex.
    pub fn rgb(&self) -> Option<[u8; 3]> {
        let value = match self {
            Self::Windows => return None,
            Self::Blue => 0x3b82f6,
            Self::Teal => 0x06b6d4,
            Self::Green => 0x22c55e,
            Self::Yellow => 0xeab308,
            Self::Orange => 0xf97316,
            Self::Red => 0xef4444,
            Self::Purple => 0xa855f7,
            Self::Pink => 0xec4899,
            Self::Custom(hex) => return library_core::query::color::parse_hex(hex),
        };
        Some([(value >> 16) as u8, (value >> 8) as u8, value as u8])
    }

    /// A custom value that is not hex becomes Windows, so a broken file never paints garbage.
    pub fn normalized(self) -> Self {
        match &self {
            Self::Custom(hex) if library_core::query::color::parse_hex(hex).is_none() => {
                Self::Windows
            }
            _ => self,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreviewBackground {
    #[default]
    Theme,
    Black,
    Gray18,
    White,
}

impl PreviewBackground {
    pub const ALL: [Self; 4] = [Self::Theme, Self::Black, Self::Gray18, Self::White];

    pub fn rgb(self) -> Option<[u8; 3]> {
        match self {
            Self::Theme => None,
            Self::Black => Some([0; 3]),
            Self::Gray18 => Some([118; 3]),
            Self::White => Some([255; 3]),
        }
    }
}

/// UI language (D-069). Stored as BCP 47 tags; English is the source locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Locale {
    #[default]
    #[serde(rename = "en")]
    En,
    #[serde(rename = "ru")]
    Ru,
    /// Chinese (Simplified).
    #[serde(rename = "zh-CN")]
    ZhCn,
}

/// Widths and open state of the left and right bars (docs/spec.md §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

/// How the library screen shows items, the same in every library (docs/spec.md §5, §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowsePreferences {
    pub view_mode: ViewMode,
    /// The width of a grid cell in CSS pixels.
    pub thumbnail_size: u32,
    pub show_names: bool,
    /// The sort of every screen without its own (a folder's sort override comes with folders).
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub theme: Theme,
    /// The accent colour of primary controls and the selection (D-182).
    pub accent: Accent,
    pub preview_background: PreviewBackground,
    pub preview_background_on_grid: bool,
    /// UI language; English is the default (D-069).
    pub locale: Locale,
    pub reopen_last_library: bool,
    /// Where Create library starts; `None` means `%USERPROFILE%\egl-library` (D-021).
    pub default_library_parent: Option<String>,
    /// `Delete` asks before moving items to the trash; off means the trash takes them at once.
    /// The trash confirmation can turn this off for good, Settings turns it back on.
    pub confirm_before_trash: bool,
    /// Where the last Backup / Export wrote; the next one starts there (docs/spec.md §7.4 item 12).
    pub export_destination: Option<String>,
    pub layout: UiLayout,
    pub browse: BrowsePreferences,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            accent: Accent::Windows,
            preview_background: PreviewBackground::Theme,
            preview_background_on_grid: false,
            locale: Locale::En,
            reopen_last_library: true,
            default_library_parent: None,
            confirm_before_trash: true,
            export_destination: None,
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
        preferences.accent = preferences.accent.normalized();
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
        assert_eq!(preferences.locale, Locale::En);
        assert!(preferences.reopen_last_library);
        assert_eq!(preferences.default_library_parent, None);
        assert!(preferences.confirm_before_trash);
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
            accent: Accent::Custom("#7c3aed".into()),
            preview_background: PreviewBackground::Gray18,
            preview_background_on_grid: true,
            locale: Locale::Ru,
            reopen_last_library: false,
            default_library_parent: Some("D:\\Libraries".into()),
            confirm_before_trash: false,
            export_destination: Some("D:\\Backups".into()),
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
        assert_eq!(stored.accent, Accent::Custom("#7c3aed".into()));
        assert_eq!(stored.locale, Locale::Ru);
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
        assert_eq!(preferences.accent, Accent::Windows);
        assert_eq!(preferences.locale, Locale::En);
        assert!(preferences.reopen_last_library);
        assert!(preferences.confirm_before_trash);
    }

    /// A preferences.json written before the accent existed (D-182) still opens.
    #[test]
    fn a_pre_accent_preferences_file_opens_with_the_windows_accent() {
        let old = r#"{
            "theme": "dark",
            "locale": "ru",
            "reopenLastLibrary": false,
            "defaultLibraryParent": "D:\\Libraries",
            "confirmBeforeTrash": false,
            "exportDestination": "D:\\Backups",
            "layout": { "leftWidth": 260, "rightWidth": 340, "leftOpen": true, "rightOpen": false },
            "browse": {
                "viewMode": "list", "thumbnailSize": 200, "showNames": false,
                "sort": "name", "ascending": true, "includeSubfolders": true
            }
        }"#;
        let preferences: Preferences = serde_json::from_str(old).unwrap();
        assert_eq!(preferences.theme, Theme::Dark);
        assert_eq!(preferences.accent, Accent::Windows);
        assert_eq!(preferences.locale, Locale::Ru);
        assert!(!preferences.reopen_last_library);
        assert_eq!(preferences.browse.thumbnail_size, 200);
    }

    #[test]
    fn accent_round_trips_and_presets_have_colours() {
        for accent in ACCENT_PRESETS {
            assert!(accent.rgb().is_some(), "{accent:?}");
            let json = serde_json::to_string(&accent).unwrap();
            assert_eq!(serde_json::from_str::<Accent>(&json).unwrap(), accent);
        }
        assert_eq!(Accent::Windows.rgb(), None);
        let custom = Accent::Custom("#0f766e".into());
        assert_eq!(custom.rgb(), Some([0x0f, 0x76, 0x6e]));
        let json = serde_json::to_string(&custom).unwrap();
        assert_eq!(json, r##"{"custom":"#0f766e"}"##);
        assert_eq!(serde_json::from_str::<Accent>(&json).unwrap(), custom);
    }

    #[test]
    fn a_broken_custom_accent_falls_back_to_windows_on_save() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        let mut preferences = store.preferences();
        preferences.accent = Accent::Custom("not a colour".into());
        store.save_preferences(&preferences).unwrap();
        assert_eq!(store.preferences().accent, Accent::Windows);
        assert_eq!(
            Accent::Custom("#abc".into()).normalized(),
            Accent::Custom("#abc".into())
        );
    }

    #[test]
    fn preview_backgrounds_round_trip_and_old_files_keep_the_theme() {
        let old: Preferences = serde_json::from_str(r#"{"theme":"dark","accent":"blue"}"#).unwrap();
        assert_eq!(old.preview_background, PreviewBackground::Theme);
        assert!(!old.preview_background_on_grid);
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        for background in PreviewBackground::ALL {
            let prefs = Preferences {
                preview_background: background,
                preview_background_on_grid: true,
                ..old.clone()
            };
            store.save_preferences(&prefs).unwrap();
            assert_eq!(store.preferences(), prefs);
        }
    }

    #[test]
    fn locale_round_trips_including_zh_cn() {
        let preferences: Preferences = serde_json::from_str(r#"{ "locale": "zh-CN" }"#).unwrap();
        assert_eq!(preferences.locale, Locale::ZhCn);
        let json = serde_json::to_string(&preferences).unwrap();
        assert!(json.contains(r#""locale":"zh-CN""#));
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
