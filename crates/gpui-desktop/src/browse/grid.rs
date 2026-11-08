//! Grid / list of item cells with thumbs or palette placeholders.

use std::sync::Arc;

use app_store::{Locale, ViewMode};
use desktop_session::Session;
use gpui_kit::component::{StyledExt as _, h_flex, label::Label, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Entity, Hsla, Image, ImageFormat, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, StyledImage as _, Window, div, img, px, rgb,
};
use library_core::ItemCell;

use crate::app::RootApp;
use crate::i18n::t;
use crate::library::{OpenLibraryState, Selection, cell_thumbnail};

pub struct ListedItem {
    pub cell: ItemCell,
    pub thumb: Option<Arc<Image>>,
}

pub fn load_listed(session: &Session, cells: &[ItemCell]) -> Vec<ListedItem> {
    cells
        .iter()
        .cloned()
        .map(|cell| {
            let thumb = cell_thumbnail(session, &cell).and_then(|thumb| {
                let format = match thumb.content_type.as_str() {
                    "image/webp" => ImageFormat::Webp,
                    "image/png" => ImageFormat::Png,
                    "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
                    "image/gif" => ImageFormat::Gif,
                    _ => return None,
                };
                Some(Arc::new(Image::from_bytes(format, thumb.bytes)))
            });
            ListedItem { cell, thumb }
        })
        .collect()
}

pub fn render_items(
    state: &OpenLibraryState,
    items: &[ListedItem],
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
) -> gpui_kit::AnyElement {
    if items.is_empty() {
        return div()
            .id("items-empty")
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(Label::new(t(locale, "shell.noItems")).text_color(theme.muted_foreground))
            .into_any_element();
    }

    match state.browse.view_mode {
        ViewMode::List => render_list(items, &state.selection, theme, entity),
        ViewMode::Grid => render_wrap_grid(state, items, &state.selection, theme, entity),
    }
}

fn render_list(
    items: &[ListedItem],
    selection: &Selection,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
) -> gpui_kit::AnyElement {
    v_flex()
        .id("item-list")
        .w_full()
        .gap_1()
        .p_2()
        .children(items.iter().enumerate().map(|(index, item)| {
            let selected = selection.contains(&item.cell.id);
            let entity = entity.clone();
            let title = format!("{}.{}", item.cell.name, item.cell.ext);
            let preview = thumb_or_palette(item, 40.0, theme.muted, theme.muted_foreground);
            h_flex()
                .id(SharedString::from(format!("list-{index}")))
                .w_full()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded(theme.radius)
                .when(selected, |el| {
                    el.bg(theme.accent).text_color(theme.accent_foreground)
                })
                .cursor_pointer()
                .on_click(move |event, _window: &mut Window, cx: &mut App| {
                    let mods = event.modifiers();
                    entity
                        .update(cx, |this, cx| {
                            this.select_item(index, mods.control, mods.shift, cx);
                        })
                        ;
                })
                .child(preview)
                .child(Label::new(title).text_sm())
                .into_any_element()
        }))
        .into_any_element()
}

fn render_wrap_grid(
    state: &OpenLibraryState,
    items: &[ListedItem],
    selection: &Selection,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
) -> gpui_kit::AnyElement {
    let size = state.browse.thumbnail_size as f32;
    let show_names = state.browse.show_names;
    div()
        .id("item-grid")
        .w_full()
        .flex()
        .flex_wrap()
        .gap_2()
        .p_2()
        .children(items.iter().enumerate().map(|(index, item)| {
            let selected = selection.contains(&item.cell.id);
            let entity = entity.clone();
            let title = format!("{}.{}", item.cell.name, item.cell.ext);
            let preview = thumb_or_palette(item, size, theme.muted, theme.muted_foreground);
            v_flex()
                .id(SharedString::from(format!("grid-{index}")))
                .w(px(size + 8.0))
                .gap_1()
                .rounded(theme.radius)
                .when(selected, |el| el.border_2().border_color(theme.accent))
                .cursor_pointer()
                .on_click(move |event, _, cx| {
                    let mods = event.modifiers();
                    entity
                        .update(cx, |this, cx| {
                            this.select_item(index, mods.control, mods.shift, cx);
                        })
                        ;
                })
                .child(preview)
                .when(show_names, |el| {
                    el.child(
                        Label::new(title)
                            .text_xs()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis(),
                    )
                })
                .into_any_element()
        }))
        .into_any_element()
}

fn thumb_or_palette(
    item: &ListedItem,
    size: f32,
    muted: Hsla,
    muted_foreground: Hsla,
) -> gpui_kit::AnyElement {
    match &item.thumb {
        Some(image) => img(image.clone())
            .w(px(size))
            .h(px(size))
            .object_fit(gpui_kit::ObjectFit::Cover)
            .into_any_element(),
        None => {
            let color = item.cell.palette0.map_or(muted, |[r, g, b]| {
                rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)).into()
            });
            div()
                .w(px(size))
                .h(px(size))
                .bg(color)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Label::new(item.cell.ext.to_uppercase())
                        .text_xs()
                        .text_color(muted_foreground),
                )
                .into_any_element()
        }
    }
}
