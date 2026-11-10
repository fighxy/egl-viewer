//! Shared look of the library chrome (D-141): rows of the left bar, section headers, the
//! selection colour, captions of the grid.

use app_store::Locale;
use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Div, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString, Stateful, Styled, div,
    px, rgb,
};
use library_core::{Folder, SmartFolder};

use crate::i18n::t;
use crate::library::Place;

pub type Kit = gpui_kit::component::theme::Theme;

/// Corner radius of thumbnails.
pub const THUMB_RADIUS: f32 = 4.0;

/// Right padding of a scrolled pane's content, the width of the overlay scrollbar and a gap.
pub const SCROLLBAR_GUTTER: f32 = 10.0;

/// The colour of a selected thumbnail's ring and caption. The kit's `accent` is a quiet grey
/// meant for hover, so a selection drawn with it cannot be seen on a dark grid.
pub fn selection_color(theme: &Kit) -> Hsla {
    theme.blue
}

/// One row of the left bar: full width, a hover tint, a filled background when selected.
pub fn nav_row(id: SharedString, selected: bool, theme: &Kit) -> Stateful<Div> {
    let hover = theme.list_hover;
    h_flex()
        .id(id)
        .w_full()
        .h(px(26.))
        .flex_none()
        .items_center()
        .gap(px(6.))
        .px(px(6.))
        .rounded(px(5.))
        .text_sm()
        .cursor_pointer()
        .when(selected, |el| el.bg(theme.list_active))
        .when(!selected, |el| el.hover(move |el| el.bg(hover)))
}

pub fn row_icon(icon: IconName, theme: &Kit) -> impl IntoElement {
    Icon::new(icon)
        .small()
        .flex_none()
        .text_color(theme.muted_foreground)
}

/// A folder glyph in the folder's colour, or muted when it has none.
pub fn folder_icon(color: Option<&str>, open: bool, theme: &Kit) -> impl IntoElement {
    let tint = color
        .and_then(folder_color)
        .map(|hex| rgb(hex).into())
        .unwrap_or(theme.muted_foreground);
    Icon::new(if open {
        IconName::FolderOpen
    } else {
        IconName::Folder
    })
    .small()
    .flex_none()
    .text_color(tint)
}

/// The eight folder colours, the same values as `FOLDER_COLORS` of the React UI.
pub fn folder_color(name: &str) -> Option<u32> {
    Some(match name {
        "red" => 0xef4444,
        "orange" => 0xf97316,
        "yellow" => 0xeab308,
        "green" => 0x22c55e,
        "aqua" => 0x06b6d4,
        "blue" => 0x3b82f6,
        "purple" => 0xa855f7,
        "pink" => 0xec4899,
        _ => return None,
    })
}

/// The name of a row: takes the free width and ends in an ellipsis.
pub fn row_label(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(px(0.))
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .child(text.into())
}

pub fn row_count(count: u32, theme: &Kit) -> impl IntoElement {
    div()
        .flex_none()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(count.to_string())
}

/// A quiet caption over a group of rows; buttons of the group are added as children.
pub fn section_header(id: &'static str, label: String, theme: &Kit) -> Stateful<Div> {
    h_flex()
        .id(id)
        .w_full()
        .h(px(26.))
        .flex_none()
        .items_center()
        .mt_3()
        .px(px(6.))
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label),
        )
}

/// What the toolbar calls the open place: the scope, the folder or the smart folder.
pub fn place_title(
    place: &Place,
    folders: &[Folder],
    smart_folders: &[SmartFolder],
    locale: Locale,
) -> String {
    match place {
        Place::All => t(locale, "shell.all"),
        Place::Uncategorized => t(locale, "shell.uncategorized"),
        Place::Untagged => t(locale, "shell.untagged"),
        Place::Trash => t(locale, "shell.trash"),
        Place::Folder { id } => {
            folder_title(folders, id).unwrap_or_else(|| t(locale, "shell.folders"))
        }
        Place::Smart { id } => {
            smart_title(smart_folders, id).unwrap_or_else(|| t(locale, "shell.smartFolders"))
        }
    }
}

fn folder_title(folders: &[Folder], id: &str) -> Option<String> {
    folders.iter().find_map(|folder| {
        if folder.id == id {
            // A locked folder is shown by its lock's caption when it has one (D-018).
            Some(
                folder
                    .lock_label
                    .clone()
                    .filter(|label| folder.is_locked && !label.is_empty())
                    .unwrap_or_else(|| folder.name.clone()),
            )
        } else {
            folder_title(&folder.children, id)
        }
    })
}

fn smart_title(folders: &[SmartFolder], id: &str) -> Option<String> {
    folders.iter().find_map(|folder| {
        if folder.id == id {
            Some(folder.name.clone())
        } else {
            smart_title(&folder.children, id)
        }
    })
}

/// `4032 × 3024` under a thumbnail; nothing when the size is unknown.
pub fn dimensions_caption(width: u32, height: u32) -> Option<String> {
    (width > 0 && height > 0).then(|| format!("{width} × {height}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(id: &str, name: &str, children: Vec<Folder>) -> Folder {
        Folder {
            id: id.into(),
            name: name.into(),
            children,
            ..Folder::default()
        }
    }

    #[test]
    fn place_title_names_the_scope() {
        assert_eq!(place_title(&Place::All, &[], &[], Locale::En), "All");
        assert_eq!(
            place_title(&Place::Untagged, &[], &[], Locale::Ru),
            "Без тегов"
        );
    }

    #[test]
    fn place_title_finds_a_nested_folder() {
        let tree = vec![folder("a", "Art", vec![folder("a1", "Sketches", vec![])])];
        let place = Place::Folder { id: "a1".into() };
        assert_eq!(place_title(&place, &tree, &[], Locale::En), "Sketches");
        let gone = Place::Folder { id: "zz".into() };
        assert_eq!(place_title(&gone, &tree, &[], Locale::En), "Folders");
    }

    #[test]
    fn place_title_uses_the_lock_caption_of_a_locked_folder() {
        let mut locked = folder("s", "Real name", vec![]);
        locked.is_locked = true;
        locked.lock_label = Some("Secret".into());
        let place = Place::Folder { id: "s".into() };
        assert_eq!(place_title(&place, &[locked], &[], Locale::En), "Secret");
    }

    #[test]
    fn place_title_finds_a_smart_folder() {
        let smart = SmartFolder {
            id: "q".into(),
            name: "Wide".into(),
            children: Vec::new(),
            conditions: serde_json::Value::Null,
            order_by: None,
            sort_ascending: None,
            unsupported: Vec::new(),
        };
        let place = Place::Smart { id: "q".into() };
        assert_eq!(place_title(&place, &[], &[smart], Locale::En), "Wide");
    }

    #[test]
    fn dimensions_caption_needs_both_sides() {
        assert_eq!(
            dimensions_caption(4032, 3024).as_deref(),
            Some("4032 × 3024")
        );
        assert_eq!(dimensions_caption(0, 3024), None);
        assert_eq!(dimensions_caption(10, 0), None);
    }

    #[test]
    fn folder_colours_match_the_react_palette() {
        assert_eq!(folder_color("red"), Some(0xef4444));
        assert_eq!(folder_color("aqua"), Some(0x06b6d4));
        assert_eq!(folder_color("teal"), None);
    }
}
