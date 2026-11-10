//! Menus of the filter bar and the Sort button, opened under their button (D-146).
//!
//! Single choices (Sort, Type, Shape, Rating, the tag match) are dropdown menus that close on
//! a pick, with the current choice checked. Folders take several picks, so that list is a
//! popover that stays open while folders are checked. Every builder reads the library when it
//! opens or renders, so a menu always shows the state of the moment.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement,
    SharedString, Styled, Window, px,
};
use library_core::{Folder, SortKey, TagMode};

use crate::app::RootApp;
use crate::i18n::t;
use crate::ops_organize::rating_filter_label;

/// The menu builder a `dropdown_menu` takes.
pub type MenuBuilder =
    Box<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static>;

/// Sort keys in menu order; Manual only where items can be put in order (a folder).
pub fn sort_keys(folder_open: bool) -> &'static [(&'static str, SortKey)] {
    const KEYS: &[(&str, SortKey)] = &[
        ("sort.added", SortKey::DateAdded),
        ("sort.btime", SortKey::DateCreated),
        ("sort.mtime", SortKey::DateModified),
        ("sort.name", SortKey::Title),
        ("sort.ext", SortKey::Extension),
        ("sort.size", SortKey::FileSize),
        ("sort.resolution", SortKey::Resolution),
        ("sort.star", SortKey::Rating),
        ("sort.tag", SortKey::FirstTag),
        ("sort.random", SortKey::Random),
        ("sort.manual", SortKey::Manual),
    ];
    if folder_open {
        KEYS
    } else {
        &KEYS[..KEYS.len() - 1]
    }
}

/// Eagle's shapes, value and label key.
pub const SHAPES: &[(&str, &str)] = &[
    ("square", "filters.square"),
    ("landscape", "filters.landscape"),
    ("portrait", "filters.portrait"),
    ("panoramic-landscape", "filters.panoramicLandscape"),
    ("panoramic-portrait", "filters.panoramicPortrait"),
];

/// Tag match modes, value and label key.
pub const TAG_MODES: &[(TagMode, &str)] = &[
    (TagMode::Any, "filters.modeAny"),
    (TagMode::All, "filters.modeAll"),
    (TagMode::Exact, "filters.modeExact"),
    (TagMode::None, "filters.modeNone"),
];

/// One checked-or-not entry that runs `f` on the app when picked.
fn pick(
    label: impl Into<SharedString>,
    checked: bool,
    entity: &Entity<RootApp>,
    f: impl Fn(&mut RootApp, &mut Context<RootApp>) + 'static,
) -> PopupMenuItem {
    let entity = entity.clone();
    PopupMenuItem::new(label)
        .checked(checked)
        .on_click(move |_, _, cx| entity.update(cx, |this, cx| f(this, cx)))
}

/// Toolbar Sort: the keys, then the direction.
pub fn sort_menu(entity: Entity<RootApp>) -> MenuBuilder {
    Box::new(move |menu, _, cx| {
        let app = entity.read(cx);
        let locale = app.locale;
        let Some(lib) = app.library.as_ref() else {
            return menu;
        };
        let (current, ascending) = (lib.sort, lib.ascending);
        let folder_open = lib.place.open_folder_id().is_some();
        let menu = sort_keys(folder_open)
            .iter()
            .fold(menu, |menu, &(key, sort)| {
                menu.item(pick(
                    t(locale, key),
                    sort == current,
                    &entity,
                    move |this, cx| this.set_sort(sort, cx),
                ))
            })
            .separator();
        menu.item(pick(
            t(locale, "sort.ascending"),
            ascending,
            &entity,
            |this, cx| this.set_ascending(true, cx),
        ))
        .item(pick(
            t(locale, "sort.descending"),
            !ascending,
            &entity,
            |this, cx| this.set_ascending(false, cx),
        ))
    })
}

/// Type: any, images, then every extension of the library with its count. Long lists scroll.
pub fn type_menu(entity: Entity<RootApp>) -> MenuBuilder {
    Box::new(move |menu, window, cx| {
        let app = entity.read(cx);
        let locale = app.locale;
        let current = app.library.as_ref().and_then(|lib| lib.filters.ext.clone());
        let extensions = app.extensions.clone();
        let menu = menu
            .max_h(window.viewport_size().height * 0.6)
            .scrollable(true)
            .item(pick(
                t(locale, "filters.anyType"),
                current.is_none(),
                &entity,
                |this, cx| this.set_ext_filter(None, cx),
            ))
            .item(pick(
                t(locale, "filters.images"),
                current.as_deref() == Some("image"),
                &entity,
                |this, cx| this.set_ext_filter(Some("image".into()), cx),
            ));
        let menu = if extensions.is_empty() {
            menu
        } else {
            menu.separator()
        };
        extensions.into_iter().fold(menu, |menu, ext| {
            let on = current.as_deref() == Some(ext.ext.as_str());
            let label = format!("{}  ({})", ext.ext.to_uppercase(), ext.count);
            let value = ext.ext;
            menu.item(pick(label, on, &entity, move |this, cx| {
                this.set_ext_filter(Some(value.clone()), cx)
            }))
        })
    })
}

pub fn shape_menu(entity: Entity<RootApp>) -> MenuBuilder {
    Box::new(move |menu, _, cx| {
        let app = entity.read(cx);
        let locale = app.locale;
        let current = app
            .library
            .as_ref()
            .and_then(|lib| lib.filters.shape.clone());
        let menu = menu.item(pick(
            t(locale, "filters.anyShape"),
            current.is_none(),
            &entity,
            |this, cx| this.set_shape_filter(None, cx),
        ));
        SHAPES.iter().fold(menu, |menu, &(value, key)| {
            menu.item(pick(
                t(locale, key),
                current.as_deref() == Some(value),
                &entity,
                move |this, cx| this.set_shape_filter(Some(value.to_owned()), cx),
            ))
        })
    })
}

/// Rating: any, not rated, then exactly 1–5 stars.
pub fn rating_menu(entity: Entity<RootApp>) -> MenuBuilder {
    Box::new(move |menu, _, cx| {
        let app = entity.read(cx);
        let locale = app.locale;
        let current = app.library.as_ref().and_then(|lib| lib.filters.rating);
        std::iter::once(None)
            .chain((0..=5).map(Some))
            .fold(menu, |menu, rating| {
                let label = match rating {
                    None => t(locale, "filters.anyRating"),
                    Some(r) => rating_filter_label(Some(r), locale),
                };
                menu.item(pick(label, current == rating, &entity, move |this, cx| {
                    this.set_rating_filter(rating, cx)
                }))
            })
    })
}

/// How the picked tags match.
pub fn tag_mode_menu(entity: Entity<RootApp>) -> MenuBuilder {
    Box::new(move |menu, _, cx| {
        let app = entity.read(cx);
        let locale = app.locale;
        let current = app.library.as_ref().map(|lib| lib.filters.tag_mode);
        let menu = menu.label(t(locale, "filters.tagMode"));
        TAG_MODES.iter().fold(menu, |menu, &(mode, key)| {
            menu.item(pick(
                t(locale, key),
                current == Some(mode),
                &entity,
                move |this, cx| this.set_tag_mode(mode, cx),
            ))
        })
    })
}

/// Folders as rows, parents before children: id, name, depth.
pub fn flatten_folders(folders: &[Folder]) -> Vec<(String, String, u32)> {
    fn walk(folders: &[Folder], depth: u32, out: &mut Vec<(String, String, u32)>) {
        for folder in folders {
            out.push((folder.id.clone(), folder.name.clone(), depth));
            walk(&folder.children, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(folders, 0, &mut out);
    out
}

/// Left padding of a folder row: one step per level, like the left bar's tree.
pub fn row_indent(depth: u32) -> f32 {
    4.0 + depth as f32 * 16.0
}

/// The Folders popover: Any / All, then every folder to check or uncheck. It stays open while
/// folders are picked and shows the filter as it is now on every render.
pub fn folder_filter_content(entity: &Entity<RootApp>, cx: &App) -> AnyElement {
    let app = entity.read(cx);
    let locale = app.locale;
    let Some(lib) = app.library.as_ref() else {
        return v_flex().into_any_element();
    };
    let mode = lib.filters.folder_mode;
    let picked = lib.filters.folders.clone();
    let rows = flatten_folders(&lib.folders);
    let mode_button = |id: &'static str, key: &'static str, value: TagMode| {
        let entity = entity.clone();
        Button::new(id)
            .ghost()
            .small()
            .label(t(locale, key))
            .selected(mode == value)
            .on_click(move |_, _, cx| entity.update(cx, |this, cx| this.set_folder_mode(value, cx)))
    };
    v_flex()
        .w(px(300.))
        .gap_1()
        .child(
            h_flex()
                .gap_1()
                .child(mode_button("ff-mode-any", "filters.modeAny", TagMode::Any))
                .child(mode_button("ff-mode-all", "filters.modeAll", TagMode::All)),
        )
        .child(
            v_flex()
                .id("folder-filter-rows")
                .max_h(px(360.))
                .overflow_y_scrollbar()
                .children(rows.into_iter().map(|(id, name, depth)| {
                    let entity = entity.clone();
                    let on = picked.contains(&id);
                    // Left-aligned rows, children indented under their parent.
                    h_flex()
                        .w_full()
                        .py(px(3.))
                        .pr_2()
                        .pl(px(row_indent(depth)))
                        .child(
                            Checkbox::new(SharedString::from(format!("ff-{id}")))
                                .checked(on)
                                .label(name)
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.toggle_folder_filter(id.clone(), cx)
                                    })
                                }),
                        )
                })),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_sort_is_offered_only_in_a_folder() {
        assert!(sort_keys(true).iter().any(|(_, k)| *k == SortKey::Manual));
        assert!(!sort_keys(false).iter().any(|(_, k)| *k == SortKey::Manual));
        assert_eq!(sort_keys(true).len(), sort_keys(false).len() + 1);
    }

    #[test]
    fn deeper_folders_sit_further_right() {
        assert!(row_indent(1) > row_indent(0));
        assert_eq!(row_indent(2) - row_indent(1), row_indent(1) - row_indent(0));
    }

    #[test]
    fn folders_flatten_parents_first() {
        let child = Folder {
            id: "c".into(),
            name: "Child".into(),
            ..Default::default()
        };
        let parent = Folder {
            id: "p".into(),
            name: "Parent".into(),
            children: vec![child],
            ..Default::default()
        };
        assert_eq!(
            flatten_folders(&[parent]),
            vec![
                ("p".to_owned(), "Parent".to_owned(), 0),
                ("c".to_owned(), "Child".to_owned(), 1)
            ]
        );
    }
}
