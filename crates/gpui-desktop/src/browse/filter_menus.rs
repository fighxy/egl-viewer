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
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use library_core::{ColorFilter, Folder, SortKey, TagMode};

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

/// The swatches of the colour filter (D-176): Eagle-like hues plus neutrals, as `0xrrggbb`.
pub const COLOR_SWATCHES: &[u32] = &[
    0xe53935, 0xfb8c00, 0xfdd835, 0x7cb342, 0x43a047, 0x00897b, 0x00acc1, 0x1e88e5, 0x3949ab,
    0x8e24aa, 0xd81b60, 0x795548, 0xd7c4a3, 0xffffff, 0x9e9e9e, 0x212121,
];

/// How much of the image a picked colour must cover, as the user chooses it.
pub const COLOR_SHARES: &[u8] = &[5, 15, 30, 50];

fn rgb_of(hex: u32) -> [u8; 3] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

/// The Colour filter (D-176): Any colour, Documents (light, colourless images: photographed
/// and scanned papers), a grid of swatches, and how much of the image the colour must cover.
/// A popover, so a swatch and then a share can be picked in one go.
pub fn color_filter_content(entity: &Entity<RootApp>, cx: &App) -> AnyElement {
    let app = entity.read(cx);
    let locale = app.locale;
    let Some(lib) = app.library.as_ref() else {
        return v_flex().into_any_element();
    };
    let current = lib.filters.color;
    let pick = |id: &'static str, label: String, on: bool, filter: Option<ColorFilter>| {
        let entity = entity.clone();
        Button::new(id)
            .ghost()
            .small()
            .label(label)
            .selected(on)
            .on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| this.set_color_filter(filter, cx));
            })
    };
    let picked_rgb = match current {
        Some(ColorFilter::Similar { rgb, .. }) => Some(rgb),
        _ => None,
    };
    let share = match current {
        Some(ColorFilter::Similar { min_share, .. }) => min_share,
        _ => library_core::query::color::SIMILAR_SHARE,
    };
    let border = cx.theme().border;
    let ring = cx.theme().foreground;
    v_flex()
        .w(px(268.))
        .gap_2()
        .child(
            h_flex()
                .gap_1()
                .child(pick(
                    "fc-any",
                    t(locale, "filters.anyColour"),
                    current.is_none(),
                    None,
                ))
                .child(
                    pick(
                        "fc-paper",
                        t(locale, "filters.documents"),
                        matches!(current, Some(ColorFilter::Paper { .. })),
                        Some(ColorFilter::paper()),
                    )
                    .icon(gpui_kit::assets::IconName::FileText)
                    .tooltip(t(locale, "filters.documentsHint")),
                ),
        )
        .child(
            h_flex()
                .flex_wrap()
                .gap(px(6.))
                .children(COLOR_SWATCHES.iter().map(|&hex| {
                    let rgb = rgb_of(hex);
                    let entity = entity.clone();
                    let on = picked_rgb == Some(rgb);
                    div()
                        .id(SharedString::from(format!("fc-swatch-{hex:06x}")))
                        .size(px(26.))
                        .rounded_full()
                        .bg(gpui_kit::rgb(hex))
                        .border_2()
                        .border_color(if on { ring } else { border })
                        .cursor_pointer()
                        .aria_label(library_core::query::color::hex(rgb))
                        .on_click(move |_, window, cx| {
                            entity.update(cx, |this, cx| {
                                this.pick_filter_colour(rgb, window, cx);
                            });
                        })
                })),
        )
        .child(custom_colour_controls(app, locale, cx))
        .when_some(picked_rgb, |el, rgb| {
            el.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(locale, "filters.colourShare")),
                    )
                    .children(COLOR_SHARES.iter().map(|&min_share| {
                        let entity = entity.clone();
                        Button::new(SharedString::from(format!("fc-share-{min_share}")))
                            .ghost()
                            .xsmall()
                            .label(format!("{min_share}%"))
                            .selected(share == min_share)
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.set_color_filter(
                                        Some(ColorFilter::Similar { rgb, min_share }),
                                        cx,
                                    );
                                });
                            })
                    })),
            )
        })
        .into_any_element()
}

/// Any colour (D-178): a preview with the hex field, then hue, saturation and lightness
/// sliders over tracks that show what each one does.
fn custom_colour_controls(app: &RootApp, locale: app_store::Locale, cx: &App) -> AnyElement {
    let custom = &app.custom_colour;
    let color = app.slider_colour(cx);
    let label = cx.theme().muted_foreground;
    let steps = 48usize;
    let ramp = |make: &dyn Fn(f32) -> gpui_kit::Hsla| -> AnyElement {
        h_flex()
            .absolute()
            .left_0()
            .right_0()
            .h(px(8.))
            .rounded(px(4.))
            .overflow_hidden()
            .children((0..steps).map(|i| {
                div()
                    .flex_1()
                    .h_full()
                    .bg(make(i as f32 / (steps - 1) as f32))
            }))
            .into_any_element()
    };
    let row = |key: &'static str,
               slider: &Entity<gpui_kit::component::slider::SliderState>,
               track: AnyElement| {
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .w(px(78.))
                    .flex_none()
                    .text_xs()
                    .text_color(label)
                    .child(t(locale, key)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .flex_1()
                    .h(px(24.))
                    .child(track)
                    .child(
                        gpui_kit::component::slider::Slider::new(slider)
                            .flex_1()
                            .bg(gpui_kit::transparent_black()),
                    ),
            )
    };
    let (h, s, l) = (color.h, color.s, color.l);
    v_flex()
        .gap_1()
        .pt_1()
        .border_t_1()
        .border_color(cx.theme().border)
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(label)
                        .flex_1()
                        .child(t(locale, "filters.customColour")),
                )
                .child(
                    div()
                        .size(px(22.))
                        .rounded_full()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(color),
                )
                .child(
                    div()
                        .w(px(96.))
                        .child(gpui_kit::component::input::Input::new(&custom.hex).small()),
                ),
        )
        .child(row(
            "filters.hue",
            &custom.hue,
            ramp(&|x| gpui_kit::hsla(x, 1., 0.5, 1.)),
        ))
        .child(row(
            "filters.saturation",
            &custom.saturation,
            ramp(&move |x| gpui_kit::hsla(h, x, l, 1.)),
        ))
        .child(row(
            "filters.lightness",
            &custom.lightness,
            ramp(&move |x| gpui_kit::hsla(h, s, x, 1.)),
        ))
        .into_any_element()
}

/// The Colour button's label: the filter's name when one is set.
pub fn color_filter_label(filter: Option<ColorFilter>, locale: app_store::Locale) -> String {
    match filter {
        None => t(locale, "filters.colour"),
        Some(ColorFilter::Paper { .. }) => t(locale, "filters.documents"),
        Some(ColorFilter::Similar { rgb, min_share }) => format!(
            "{} {} ≥{min_share}%",
            t(locale, "filters.colour"),
            library_core::query::color::hex(rgb)
        ),
    }
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
