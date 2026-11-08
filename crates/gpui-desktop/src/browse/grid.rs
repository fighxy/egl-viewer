//! Grid / list of item cells with justified packing, virtualization, thumbs, a11y.

use std::rc::Rc;
use std::sync::Arc;

use app_store::{Locale, ViewMode};
use desktop_session::Session;
use gpui_kit::component::{h_flex, label::Label, v_flex, v_virtual_list};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Entity, Hsla, Image, ImageFormat, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, StyledImage as _,
    Window, div, img, px, rgb, size,
};
use library_core::ItemCell;

use crate::app::{RootApp, RubberBand};
use crate::i18n::t;
use crate::justified::{self, CellDimensions, JustifiedLayout, PADDING, pack_justified};
use crate::library::{OpenLibraryState, Selection, cell_thumbnail};

#[derive(Clone)]
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

pub fn build_justified(
    state: &OpenLibraryState,
    container_width: f32,
) -> JustifiedLayout {
    let dims = &state.dimensions;
    pack_justified(
        state.cells.len(),
        container_width.max(120.0),
        state.browse.thumbnail_size as f32,
        state.browse.show_names,
        |index| {
            let (w, h) = dims
                .get(index)
                .copied()
                .map(|d| (d[0], d[1]))
                .unwrap_or((0, 0));
            CellDimensions {
                width: w,
                height: h,
            }
        },
    )
}

pub fn render_items(
    state: &OpenLibraryState,
    items: &[ListedItem],
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    layout: Option<&JustifiedLayout>,
    rubber: Option<&RubberBand>,
    container_width: f32,
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
        ViewMode::List => render_virtual_list(items, &state.selection, theme, entity, rubber),
        ViewMode::Grid => {
            if let Some(layout) = layout {
                render_justified_virtual(
                    items,
                    &state.selection,
                    state.browse.show_names,
                    theme,
                    entity,
                    layout,
                    rubber,
                    container_width,
                )
            } else {
                render_wrap_grid(state, items, &state.selection, theme, entity)
            }
        }
    }
}

fn rubber_overlay(rubber: Option<&RubberBand>, theme: &gpui_kit::component::theme::Theme) -> Option<gpui_kit::AnyElement> {
    let band = rubber?;
    let rect = justified::band_rect(band.start, band.current);
    Some(
        div()
            .id("rubber-band")
            .absolute()
            .left(px(rect.left))
            .top(px(rect.top))
            .w(px((rect.right - rect.left).max(1.0)))
            .h(px((rect.bottom - rect.top).max(1.0)))
            .border_1()
            .border_color(theme.accent)
            .bg(theme.accent.opacity(0.18))
            .into_any_element(),
    )
}

fn attach_rubber_handlers(
    el: gpui_kit::Stateful<gpui_kit::Div>,
    entity: Entity<RootApp>,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    el.on_mouse_down(MouseButton::Left, {
        let entity = entity.clone();
        move |event, _window: &mut Window, cx: &mut App| {
            if event.modifiers.control || event.modifiers.shift {
                return;
            }
            let pos = (event.position.x.as_f32(), event.position.y.as_f32());
            entity.update(cx, |this, cx| {
                this.begin_rubber(pos, cx);
            });
        }
    })
    .on_mouse_move({
        let entity = entity.clone();
        move |event, _window: &mut Window, cx: &mut App| {
            let pos = (event.position.x.as_f32(), event.position.y.as_f32());
            entity.update(cx, |this, cx| {
                this.update_rubber(pos, cx);
            });
        }
    })
    .on_mouse_up(MouseButton::Left, {
        let entity = entity.clone();
        move |_event, _window: &mut Window, cx: &mut App| {
            entity.update(cx, |this, cx| {
                this.end_rubber(cx);
            });
        }
    })
}

fn render_virtual_list(
    items: &[ListedItem],
    selection: &Selection,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    rubber: Option<&RubberBand>,
) -> gpui_kit::AnyElement {
    let row_h = px(48.);
    let sizes = Rc::new(vec![size(px(100.), row_h); items.len()]);
    let overlay = rubber_overlay(rubber, theme);
    let list_entity = entity.clone();
    let list = v_virtual_list(entity.clone(), "item-list-virtual", sizes, {
        let items = items.to_vec();
        let selection = selection.clone();
        let theme = theme.clone();
        move |_, range, _, _| {
            range
                .map(|index| {
                    let item = &items[index];
                    let selected = selection.contains(&item.cell.id);
                    let focused = selection.focus == Some(index);
                    let entity = list_entity.clone();
                    let title = format!("{}.{}", item.cell.name, item.cell.ext);
                    let preview = thumb_or_palette(item, 40.0, theme.muted, theme.muted_foreground);
                    h_flex()
                        .id(SharedString::from(format!("list-{index}")))
                        .w_full()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .h(row_h)
                        .rounded(theme.radius)
                        .aria_label(title.clone())
                        .when(selected, |el| {
                            el.bg(theme.accent).text_color(theme.accent_foreground)
                        })
                        .when(focused && !selected, |el| el.border_1().border_color(theme.accent))
                        .cursor_pointer()
                        .on_click(move |event, _window: &mut Window, cx: &mut App| {
                            let mods = event.modifiers();
                            entity.update(cx, |this, cx| {
                                this.select_item(index, mods.control, mods.shift, cx);
                            });
                        })
                        .child(preview)
                        .child(Label::new(title).text_sm())
                        .into_any_element()
                })
                .collect()
        }
    });

    attach_rubber_handlers(
        div()
            .id("item-list")
            .relative()
            .size_full()
            .child(list)
            .when_some(overlay, |el, overlay| el.child(overlay)),
        entity,
    )
    .into_any_element()
}

fn render_justified_virtual(
    items: &[ListedItem],
    selection: &Selection,
    show_names: bool,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    layout: &JustifiedLayout,
    rubber: Option<&RubberBand>,
    container_width: f32,
) -> gpui_kit::AnyElement {
    let sizes: Rc<Vec<gpui_kit::Size<Pixels>>> = Rc::new(
        layout
            .rows
            .iter()
            .map(|row| size(px(container_width.max(1.0)), px(row.stride)))
            .collect(),
    );
    let overlay = rubber_overlay(rubber, theme);
    let rows = layout.rows.clone();
    let list_entity = entity.clone();
    let list = v_virtual_list(entity.clone(), "justified-rows", sizes, {
        let items = items.to_vec();
        let selection = selection.clone();
        let theme = theme.clone();
        move |_, range, _, _| {
            range
                .filter_map(|row_ix| {
                    let row = rows.get(row_ix)?;
                    let entity = list_entity.clone();
                    let row_el = div()
                        .id(SharedString::from(format!("jrow-{row_ix}")))
                        .relative()
                        .w_full()
                        .h(px(row.stride))
                        .children(row.cells.iter().map(|cell| {
                            let index = cell.index;
                            let item = &items[index];
                            let selected = selection.contains(&item.cell.id);
                            let focused = selection.focus == Some(index);
                            let entity = entity.clone();
                            let title = format!("{}.{}", item.cell.name, item.cell.ext);
                            let preview = thumb_or_palette(
                                item,
                                cell.height,
                                theme.muted,
                                theme.muted_foreground,
                            );
                            let cell_el = v_flex()
                                .id(SharedString::from(format!("grid-{index}")))
                                .absolute()
                                .left(px(PADDING + cell.left))
                                .top(px(0.))
                                .w(px(cell.width))
                                .gap_1()
                                .rounded(theme.radius)
                                .aria_label(title.clone())
                                .when(selected, |el| el.border_2().border_color(theme.accent))
                                .when(focused && !selected, |el| {
                                    el.border_1().border_color(theme.accent)
                                })
                                .cursor_pointer()
                                .on_click(move |event, _, cx| {
                                    let mods = event.modifiers();
                                    entity.update(cx, |this, cx| {
                                        this.select_item(index, mods.control, mods.shift, cx);
                                    });
                                })
                                .child(
                                    div()
                                        .w(px(cell.width))
                                        .h(px(cell.height))
                                        .overflow_hidden()
                                        .child(preview),
                                )
                                .when(show_names, |el| {
                                    el.child(
                                        Label::new(title)
                                            .text_xs()
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .text_ellipsis(),
                                    )
                                });
                            cell_el.into_any_element()
                        }));
                    Some(row_el.into_any_element())
                })
                .collect()
        }
    });

    attach_rubber_handlers(
        div()
            .id("item-grid")
            .relative()
            .size_full()
            .pt(px(PADDING))
            .child(list)
            .when_some(overlay, |el, overlay| el.child(overlay)),
        entity,
    )
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
        .id("item-grid-wrap")
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
                .aria_label(title.clone())
                .when(selected, |el| el.border_2().border_color(theme.accent))
                .cursor_pointer()
                .on_click(move |event, _, cx| {
                    let mods = event.modifiers();
                    entity.update(cx, |this, cx| {
                        this.select_item(index, mods.control, mods.shift, cx);
                    });
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

