//! Read-only details panel (Phase 2 / plan 2.1). Edits arrive in Phase 3.

use std::sync::Arc;

use app_store::Locale;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Entity, Image, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, StyledImage as _, div, img, px, rgb,
};
use library_core::{Folder, Item, ItemCell, SelectionTags};

use crate::app::RootApp;
use crate::browse::format::{format_bytes, format_date, format_dimensions};
use crate::i18n::{t, t_n};
use crate::library::OpenLibraryState;

#[derive(Clone)]
pub struct DetailsModel {
    pub count: usize,
    pub item: Option<Item>,
    pub thumb: Option<Arc<Image>>,
    pub selection: Option<SelectionTags>,
    pub stack_thumbs: Vec<Option<Arc<Image>>>,
}

pub fn render_details(
    state: &OpenLibraryState,
    model: &DetailsModel,
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
) -> gpui_kit::AnyElement {
    let in_trash = matches!(state.place, crate::library::Place::Trash);

    v_flex()
        .id("details-panel")
        .size_full()
        .p_3()
        .gap_3()
        .overflow_hidden()
        .child(match model.count {
            0 => empty_placeholder(locale, theme).into_any_element(),
            1 => {
                if let Some(item) = model.item.as_ref() {
                    single_item(item, model.thumb.clone(), &state.folders, locale, theme, entity, in_trash)
                        .into_any_element()
                } else {
                    Label::new(t(locale, "details.couldNotRead"))
                        .text_color(theme.muted_foreground)
                        .into_any_element()
                }
            }
            n => multi_select(n, model, locale, theme, entity, in_trash).into_any_element(),
        })
        .into_any_element()
}

fn empty_placeholder(
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
) -> impl IntoElement {
    v_flex()
        .id("details-empty")
        .size_full()
        .items_center()
        .justify_center()
        .child(
            Label::new(t(locale, "details.selectItem")).text_color(theme.muted_foreground),
        )
}

fn multi_select(
    count: usize,
    model: &DetailsModel,
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    in_trash: bool,
) -> impl IntoElement {
    let size = model.selection.as_ref().map(|s| s.size);
    v_flex()
        .id("details-multi")
        .gap_3()
        .overflow_hidden()
        .child(stack_preview(model, theme))
        .child(
            Label::new({
                let base = t_n(locale, "details.itemsSelected", count as u32);
                match size {
                    Some(bytes) => format!("{base} · {}", format_bytes(Some(bytes))),
                    None => base,
                }
            })
            .text_sm()
            .text_color(theme.muted_foreground),
        )
        .when_some(model.selection.as_ref(), |el, sel| {
            el.child(field(
                locale,
                theme,
                "details.tags",
                if sel.tags.is_empty() {
                    t(locale, "details.none")
                } else {
                    sel.tags
                        .iter()
                        .map(|tag| tag.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            ))
        })
        .child(action_buttons(locale, entity, true, in_trash))
}

fn single_item(
    item: &Item,
    thumb: Option<Arc<Image>>,
    folders: &[Folder],
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    in_trash: bool,
) -> impl IntoElement {
    let preview_entity = entity.clone();
    let open_link_url = item.url.clone();
    let has_link = library_core::is_web_link(&item.url);

    v_flex()
        .id("details-single")
        .gap_3()
        .overflow_hidden()
        .child(
            div()
                .id("details-thumb")
                .w_full()
                .h(px(180.))
                .rounded(theme.radius)
                .bg(theme.muted.opacity(0.35))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    preview_entity.update(cx, |this, cx| this.open_preview(cx));
                })
                .child(match thumb {
                    Some(image) => img(image)
                        .id("details-thumb-img")
                        .object_fit(gpui_kit::ObjectFit::Contain)
                        .max_w_full()
                        .max_h_full()
                        .into_any_element(),
                    None => Label::new(item.ext.to_ascii_uppercase())
                        .text_color(theme.muted_foreground)
                        .into_any_element(),
                }),
        )
        .when(!item.palettes.is_empty(), |el| {
            el.child(
                h_flex()
                    .id("details-palette")
                    .h(px(14.))
                    .w_full()
                    .rounded(theme.radius)
                    .overflow_hidden()
                    .children(item.palettes.iter().map(|entry| {
                        let [r, g, b] = entry.color;
                        let grow = entry.ratio.max(1.0) as f32;
                        div()
                            .id(SharedString::from(format!("pal-{r}-{g}-{b}")))
                            .h_full()
                            .flex_grow(grow)
                            .bg(rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32))
                    })),
            )
        })
        .when(item.is_deleted, |el| {
            el.child(
                Label::new(if item.deleted_at.is_some() {
                    format!(
                        "{} {}",
                        t(locale, "details.inTrashSince"),
                        format_date(item.deleted_at)
                    )
                } else {
                    t(locale, "details.inTrash").into()
                })
                .text_xs()
                .text_color(theme.muted_foreground),
            )
        })
        .child(field(locale, theme, "details.name", item.name.clone()))
        .child(field(
            locale,
            theme,
            "details.rating",
            if item.star == 0 {
                t(locale, "details.none")
            } else {
                "★".repeat(item.star as usize)
            },
        ))
        .child(field(
            locale,
            theme,
            "details.tags",
            if item.tags.is_empty() {
                t(locale, "details.none")
            } else {
                item.tags.join(", ")
            },
        ))
        .child(field(
            locale,
            theme,
            "details.folders",
            folder_names(folders, &item.folders, locale),
        ))
        .when(!item.annotation.is_empty(), |el| {
            el.child(field(
                locale,
                theme,
                "details.note",
                item.annotation.clone(),
            ))
        })
        .when(!item.url.is_empty(), |el| {
            let entity = entity.clone();
            el.child(
                v_flex()
                    .gap_1()
                    .child(field(locale, theme, "details.link", item.url.clone()))
                    .when(has_link, |el| {
                        el.child(
                            Button::new("details-open-link")
                                .ghost()
                                .label(t(locale, "details.openLink"))
                                .on_click(move |_, _, cx| {
                                    let url = open_link_url.clone();
                                    entity.update(cx, |this, cx| this.open_item_link(&url, cx));
                                }),
                        )
                    }),
            )
        })
        .child(action_buttons(locale, entity, false, in_trash || item.is_deleted))
        .child(
            v_flex()
                .gap_1()
                .child(
                    Label::new(t(locale, "details.info"))
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    Label::new(
                        [
                            item.ext.to_ascii_uppercase(),
                            format_bytes(item.size),
                            format_dimensions(item.width, item.height),
                        ]
                        .join(" · "),
                    )
                    .text_sm(),
                )
                .child(field(locale, theme, "details.added", format_date(item.added_at)))
                .child(field(
                    locale,
                    theme,
                    "details.created",
                    format_date(item.file_created_at),
                ))
                .child(field(
                    locale,
                    theme,
                    "details.modified",
                    format_date(item.file_modified_at),
                )),
        )
        .children(exif_rows(item, locale, theme))
}

fn action_buttons(
    locale: Locale,
    entity: Entity<RootApp>,
    several: bool,
    in_trash: bool,
) -> impl IntoElement {
    h_flex()
        .id("details-actions")
        .gap_1()
        .flex_wrap()
        .when(!in_trash, |el| {
            let reveal = entity.clone();
            let open = entity.clone();
            let copy = entity.clone();
            el.child(
                Button::new("details-reveal")
                    .outline()
                    .label(t(locale, "details.showExplorer"))
                    .on_click(move |_, _, cx| {
                        reveal.update(cx, |this, cx| this.reveal_selection(cx));
                    }),
            )
            .when(!several, |el| {
                el.child(
                    Button::new("details-open")
                        .outline()
                        .label(t(locale, "details.openDefault"))
                        .on_click(move |_, _, cx| {
                            open.update(cx, |this, cx| this.open_original(cx));
                        }),
                )
            })
            .child(
                Button::new("details-copy")
                    .outline()
                    .label(if several {
                        t(locale, "details.copyFiles")
                    } else {
                        t(locale, "details.copyFile")
                    })
                    .on_click(move |_, _, cx| {
                        copy.update(cx, |this, cx| this.copy_selection(cx));
                    }),
            )
        })
        .when(in_trash, |el| {
            el.child(
                Label::new(t(locale, "details.inTrash"))
                    .text_xs()
                    .text_color(gpui_kit::rgb(0x888888)),
            )
        })
}

fn field(
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    label_key: &'static str,
    value: impl Into<SharedString>,
) -> impl IntoElement {
    h_flex()
        .id(SharedString::from(format!("field-{label_key}")))
        .gap_2()
        .items_start()
        .child(
            Label::new(t(locale, label_key))
                .text_sm()
                .text_color(theme.muted_foreground)
                .w(px(72.)),
        )
        .child(Label::new(value).text_sm().flex_1())
}

fn folder_names(folders: &[Folder], ids: &[String], locale: Locale) -> String {
    if ids.is_empty() {
        return t(locale, "details.none");
    }
    ids.iter()
        .map(|id| find_folder_name(folders, id).unwrap_or_else(|| t(locale, "details.unknownFolder")))
        .collect::<Vec<_>>()
        .join(", ")
}

fn find_folder_name(folders: &[Folder], id: &str) -> Option<String> {
    for folder in folders {
        if folder.id == id {
            return Some(folder.name.clone());
        }
        if let Some(name) = find_folder_name(&folder.children, id) {
            return Some(name);
        }
    }
    None
}

fn stack_preview(
    model: &DetailsModel,
    theme: &gpui_kit::component::theme::Theme,
) -> impl IntoElement {
    let cells: Vec<ItemCell> = model
        .selection
        .as_ref()
        .map(|s| s.stack.clone())
        .unwrap_or_default();
    h_flex()
        .id("details-stack")
        .justify_center()
        .h(px(120.))
        .children(cells.iter().enumerate().take(3).map(|(index, cell)| {
            let thumb = model.stack_thumbs.get(index).cloned().flatten();
            div()
                .id(SharedString::from(format!("stack-{index}")))
                .w(px(96.))
                .h(px(96.))
                .ml(if index == 0 { px(0.) } else { px(-48.) })
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .bg(theme.muted.opacity(0.4))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .child(match thumb {
                    Some(image) => img(image)
                        .id(SharedString::from(format!("stack-img-{index}")))
                        .object_fit(gpui_kit::ObjectFit::Contain)
                        .max_w_full()
                        .max_h_full()
                        .into_any_element(),
                    None => Label::new(cell.ext.to_ascii_uppercase())
                        .text_xs()
                        .into_any_element(),
                })
        }))
}

fn exif_rows(
    item: &Item,
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
) -> Vec<gpui_kit::AnyElement> {
    let Some(exif) = item.exif.as_ref().and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let defs: [(&str, &str, Option<fn(&str) -> String>); 6] = [
        ("camera", "details.camera", None),
        ("isoSpeed", "details.iso", None),
        (
            "aperture",
            "details.aperture",
            Some(|v| format!("ƒ/{v}")),
        ),
        ("shutter", "details.shutter", None),
        (
            "focalLength",
            "details.focalLength",
            Some(|v| format!("{v} mm")),
        ),
        ("timestamp", "details.taken", None),
    ];
    let mut rows = Vec::new();
    for (key, label, format) in defs {
        let Some(value) = exif.get(key) else {
            continue;
        };
        let text = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => continue,
        };
        let shown = format.map(|f| f(&text)).unwrap_or(text);
        rows.push(field(locale, theme, label, shown).into_any_element());
    }
    if !rows.is_empty() {
        rows.insert(
            0,
            Label::new("EXIF")
                .text_xs()
                .text_color(theme.muted_foreground)
                .into_any_element(),
        );
    }
    rows
}
