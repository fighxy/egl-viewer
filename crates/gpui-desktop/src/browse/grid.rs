//! Grid / list of item cells with justified packing, virtualization, thumbs, a11y.

use std::rc::Rc;
use std::sync::Arc;

use app_store::{Locale, ViewMode};
use desktop_session::Session;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{VirtualListScrollHandle, h_flex, label::Label, v_flex, v_virtual_list};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Entity, Hsla, Image, ImageFormat, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, StyledImage as _,
    Window, div, img, px, rgb, size,
};
use library_core::{ItemCell, ItemId, SortKey};

use crate::app::{RootApp, RubberBand};
use crate::browse::preview::animation_frame;
use crate::browse::style;
use crate::dnd::{self, DragPayload};
use crate::i18n::t;
use crate::justified::{self, CellDimensions, JustifiedLayout, PADDING, pack_justified};
use crate::library::{OpenLibraryState, Place, Selection, cell_thumbnail};
use crate::ops_dnd::DropIntent;
use crate::ops_items;
use crate::ops_media::{SCRUB_STRIPS, scrubs_on_hover};
use crate::ops_preview::plays_on_hover;

#[derive(Clone)]
pub struct ListedItem {
    pub cell: ItemCell,
    pub thumb: Option<Arc<Image>>,
}

/// The thumbnails of the view's pages that are loaded (D-167).
pub type ListedPages = crate::view_window::Paged<ListedItem>;

/// A placeholder row of the list for a position whose page is not here yet.
fn placeholder_row(
    index: usize,
    height: Pixels,
    theme: &gpui_kit::component::theme::Theme,
) -> gpui_kit::AnyElement {
    h_flex()
        .id(SharedString::from(format!("list-{index}")))
        .w_full()
        .h(height)
        .px_2()
        .py_1()
        .child(div().size(px(40.)).rounded(theme.radius).bg(theme.muted))
        .into_any_element()
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

pub fn build_justified(state: &OpenLibraryState, container_width: f32) -> JustifiedLayout {
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

#[allow(clippy::too_many_arguments)]
pub fn render_items(
    state: &OpenLibraryState,
    items: &ListedPages,
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    layout: Option<&JustifiedLayout>,
    rubber: Option<&RubberBand>,
    container_width: f32,
    scroll: &VirtualListScrollHandle,
) -> gpui_kit::AnyElement {
    if state.cells.is_empty() {
        return div()
            .id("items-empty")
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(Label::new(t(locale, "shell.noItems")).text_color(theme.muted_foreground))
            .into_any_element();
    }

    let reorderable = state.sort == SortKey::Manual
        && matches!(state.place, Place::Folder { .. })
        && !state.info.read_only;
    let folder_id = state.place.open_folder_id().map(str::to_owned);
    match state.browse.view_mode {
        ViewMode::List => render_virtual_list(
            state.cells.len(),
            &state.selection,
            theme,
            entity,
            rubber,
            reorderable,
            folder_id,
            Rc::new(state.dimensions.clone()),
            scroll,
        ),
        ViewMode::Grid => {
            if let Some(layout) = layout {
                render_justified_virtual(
                    state.cells.len(),
                    &state.selection,
                    state.browse.show_names,
                    theme,
                    entity,
                    layout,
                    rubber,
                    container_width,
                    reorderable,
                    folder_id,
                    Rc::new(state.dimensions.clone()),
                    scroll,
                )
            } else {
                render_wrap_grid(
                    state,
                    items,
                    &state.selection,
                    theme,
                    entity,
                    reorderable,
                    folder_id,
                )
            }
        }
    }
}

fn rubber_overlay(
    rubber: Option<&RubberBand>,
    theme: &gpui_kit::component::theme::Theme,
    scroll_y: f32,
) -> Option<gpui_kit::AnyElement> {
    let band = rubber?;
    let rect = justified::band_rect(band.start, band.current);
    Some(
        div()
            .id("rubber-band")
            .absolute()
            .left(px(rect.left))
            // The band is kept in content coordinates; draw it where the content is now.
            .top(px(rect.top - scroll_y))
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

#[allow(clippy::too_many_arguments)]
fn render_virtual_list(
    total: usize,
    selection: &Selection,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    rubber: Option<&RubberBand>,
    reorderable: bool,
    folder_id: Option<String>,
    dimensions: Rc<Vec<[u32; 2]>>,
    scroll: &VirtualListScrollHandle,
) -> gpui_kit::AnyElement {
    let row_h = px(48.);
    let sizes = Rc::new(vec![size(px(100.), row_h); total]);
    let overlay = rubber_overlay(rubber, theme, (-scroll.offset().y.as_f32()).max(0.0));
    let list_entity = entity.clone();
    let list = v_virtual_list(entity.clone(), "item-list-virtual", sizes, {
        let selection = selection.clone();
        let theme = theme.clone();
        let folder_id = folder_id.clone();
        move |this, range, _, cx| {
            // These rows are on screen: fetch their pages if they are not here yet (D-167).
            this.show_range(range.clone(), cx);
            range
                .map(|index| {
                    let Some(item) = this.items.get(index).cloned() else {
                        return placeholder_row(index, row_h, &theme);
                    };
                    let item = &item;
                    let selected = selection.contains(&item.cell.id);
                    let focused = selection.focus == Some(index);
                    let entity = list_entity.clone();
                    let title = format!("{}.{}", item.cell.name, item.cell.ext);
                    let preview = thumb_fill(item, 40.0, 40.0, theme.muted, theme.muted_foreground);
                    let payload = this.drag_payload_for_index(index);
                    let mut row = h_flex()
                        .id(SharedString::from(format!("list-{index}")))
                        .w_full()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .h(row_h)
                        .rounded(theme.radius)
                        .aria_label(title.clone())
                        .when(selected, |el| el.bg(theme.list_active))
                        .when(!selected, |el| {
                            let hover = theme.list_hover;
                            el.hover(move |el| el.bg(hover))
                        })
                        .when(focused && !selected, |el| {
                            el.border_1().border_color(style::selection_color(&theme))
                        })
                        .cursor_pointer()
                        .on_click(move |event, _window: &mut Window, cx: &mut App| {
                            let mods = event.modifiers();
                            let double = event.click_count() >= 2;
                            entity.update(cx, |this, cx| {
                                this.select_item(index, mods.control, mods.shift, cx);
                                if double && !mods.control && !mods.shift {
                                    this.open_preview_at(index, cx);
                                }
                            });
                        });
                    row = attach_item_drag(row, payload, list_entity.clone());
                    row = attach_context_select(row, list_entity.clone(), index);
                    if reorderable && let Some(folder_id) = folder_id.clone() {
                        row = attach_cell_drop(
                            row,
                            list_entity.clone(),
                            folder_id,
                            Some(item.cell.id.clone()),
                        );
                    }
                    row.child(preview)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_sm()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(title),
                        )
                        .when_some(
                            dimensions
                                .get(index)
                                .and_then(|d| style::dimensions_caption(d[0], d[1])),
                            |el, size| {
                                el.child(
                                    div()
                                        .flex_none()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(size),
                                )
                            },
                        )
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
            .child(list.track_scroll(scroll))
            .vertical_scrollbar(scroll)
            .when_some(overlay, |el, overlay| el.child(overlay)),
        entity.clone(),
    )
    .context_menu(ops_items::context_menu_builder(
        entity,
        ops_items::item_menu_model,
    ))
    .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_justified_virtual(
    _total: usize,
    selection: &Selection,
    show_names: bool,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    layout: &JustifiedLayout,
    rubber: Option<&RubberBand>,
    container_width: f32,
    reorderable: bool,
    folder_id: Option<String>,
    dimensions: Rc<Vec<[u32; 2]>>,
    scroll: &VirtualListScrollHandle,
) -> gpui_kit::AnyElement {
    let sizes: Rc<Vec<gpui_kit::Size<Pixels>>> = Rc::new(
        layout
            .rows
            .iter()
            .map(|row| size(px(container_width.max(1.0)), px(row.stride)))
            .collect(),
    );
    let overlay = rubber_overlay(rubber, theme, (-scroll.offset().y.as_f32()).max(0.0));
    let rows = layout.rows.clone();
    let list_entity = entity.clone();
    let list = v_virtual_list(entity.clone(), "justified-rows", sizes, {
        let selection = selection.clone();
        let theme = theme.clone();
        let folder_id = folder_id.clone();
        move |this, range, _, cx| {
            // The rows on screen hold these positions: fetch their pages if needed (D-167).
            let first = rows
                .get(range.start)
                .and_then(|row| row.cells.first())
                .map(|cell| cell.index);
            let last = range
                .end
                .checked_sub(1)
                .and_then(|row_ix| rows.get(row_ix))
                .and_then(|row| row.cells.last())
                .map(|cell| cell.index + 1);
            if let (Some(first), Some(last)) = (first, last) {
                this.show_range(first..last, cx);
            }
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
                            let Some(item) = this.items.get(index).cloned() else {
                                // Not fetched yet: an empty tile of the final size.
                                return div()
                                    .id(SharedString::from(format!("grid-{index}")))
                                    .absolute()
                                    .left(px(PADDING + cell.left))
                                    .top(px(0.))
                                    .w(px(cell.width))
                                    .h(px(cell.height))
                                    .rounded(px(style::THUMB_RADIUS))
                                    .bg(theme.muted)
                                    .into_any_element();
                            };
                            let item = &item;
                            let selected = selection.contains(&item.cell.id);
                            let focused = selection.focus == Some(index);
                            let entity = entity.clone();
                            let title = format!("{}.{}", item.cell.name, item.cell.ext);
                            // A video under the pointer shows the frame there (D-170).
                            let scrub = this.scrub_frame(&item.cell);
                            let scrub_frame = scrub.as_ref().and_then(|(frame, _)| frame.clone());
                            // A GIF under the pointer plays in its cell (D-154).
                            let preview = match this.hover_frames(&item.cell) {
                                _ if scrub_frame.is_some() => div()
                                    .w(px(cell.width))
                                    .h(px(cell.height))
                                    .children(scrub_frame.clone().map(|frame| {
                                        animation_frame(
                                            frame,
                                            0,
                                            gpui_kit::ObjectFit::Cover,
                                            style::THUMB_RADIUS,
                                        )
                                    }))
                                    .into_any_element(),
                                Some((frames, frame)) => div()
                                    .w(px(cell.width))
                                    .h(px(cell.height))
                                    .child(animation_frame(
                                        frames,
                                        frame,
                                        gpui_kit::ObjectFit::Cover,
                                        style::THUMB_RADIUS,
                                    ))
                                    .into_any_element(),
                                None => thumb_fill(
                                    item,
                                    cell.width,
                                    cell.height,
                                    theme.muted,
                                    theme.muted_foreground,
                                ),
                            };
                            let ring = style::selection_color(&theme);
                            let size_caption = dimensions
                                .get(index)
                                .and_then(|d| style::dimensions_caption(d[0], d[1]));
                            let payload = this.drag_payload_for_index(index);
                            let mut cell_el = v_flex()
                                .id(SharedString::from(format!("grid-{index}")))
                                .absolute()
                                .left(px(PADDING + cell.left))
                                .top(px(0.))
                                .w(px(cell.width))
                                .gap(px(3.))
                                .aria_label(title.clone())
                                .cursor_pointer()
                                .on_click(move |event, _, cx| {
                                    let mods = event.modifiers();
                                    let double = event.click_count() >= 2;
                                    entity.update(cx, |this, cx| {
                                        this.select_item(index, mods.control, mods.shift, cx);
                                        if double && !mods.control && !mods.shift {
                                            this.open_preview_at(index, cx);
                                        }
                                    });
                                });
                            cell_el = attach_item_drag(cell_el, payload, list_entity.clone());
                            cell_el = attach_context_select(cell_el, list_entity.clone(), index);
                            if plays_on_hover(&item.cell) {
                                let hover = list_entity.clone();
                                let hovered_cell = item.cell.clone();
                                let edge = cell.width.max(cell.height);
                                cell_el = cell_el.on_hover(move |hovered, _, cx| {
                                    hover.update(cx, |this, cx| {
                                        this.hover_gif(&hovered_cell, edge, *hovered, cx);
                                    });
                                });
                            }
                            let scrubs = scrubs_on_hover(&item.cell);
                            if scrubs {
                                let hover = list_entity.clone();
                                let hovered_cell = item.cell.clone();
                                cell_el = cell_el.on_hover(move |hovered, _, cx| {
                                    if !*hovered {
                                        hover.update(cx, |this, cx| {
                                            this.hover_scrub(&hovered_cell, None, 0.0, cx);
                                        });
                                    }
                                });
                            }
                            let strips = scrubs.then(|| {
                                scrub_strips(
                                    &item.cell,
                                    cell.width.max(cell.height),
                                    list_entity.clone(),
                                )
                            });
                            let badge = item.cell.duration_ms.map(duration_badge);
                            let progress = scrub.map(|(_, fraction)| fraction);
                            if reorderable && let Some(folder_id) = folder_id.clone() {
                                cell_el = attach_cell_drop(
                                    cell_el,
                                    list_entity.clone(),
                                    folder_id,
                                    Some(item.cell.id.clone()),
                                );
                            }
                            // The ring is drawn over the image, so selecting a cell never
                            // moves or resizes its thumbnail.
                            cell_el
                                .child(
                                    div()
                                        .relative()
                                        .w(px(cell.width))
                                        .h(px(cell.height))
                                        .overflow_hidden()
                                        .child(preview)
                                        .children(badge)
                                        .when_some(progress, |el, fraction| {
                                            el.child(scrub_progress(fraction))
                                        })
                                        .children(strips)
                                        .when(selected || focused, |el| {
                                            el.child(
                                                div()
                                                    .absolute()
                                                    .inset_0()
                                                    .rounded(px(style::THUMB_RADIUS))
                                                    .when(selected, |el| el.border_2())
                                                    .when(!selected, |el| el.border_1())
                                                    .border_color(ring),
                                            )
                                        }),
                                )
                                .when(show_names, |el| {
                                    el.child(
                                        h_flex().w_full().justify_center().child(
                                            div()
                                                .max_w_full()
                                                .px_1()
                                                .rounded(px(3.))
                                                .text_xs()
                                                .whitespace_nowrap()
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .when(selected, |el| {
                                                    el.bg(ring).text_color(gpui_kit::white())
                                                })
                                                .child(title),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .text_center()
                                            .text_size(px(10.))
                                            .line_height(px(12.))
                                            .text_color(theme.muted_foreground)
                                            .child(size_caption.unwrap_or_default()),
                                    )
                                })
                                .into_any_element()
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
            .child(list.track_scroll(scroll))
            .vertical_scrollbar(scroll)
            .when_some(overlay, |el, overlay| el.child(overlay)),
        entity.clone(),
    )
    .context_menu(ops_items::context_menu_builder(
        entity,
        ops_items::item_menu_model,
    ))
    .into_any_element()
}

fn render_wrap_grid(
    state: &OpenLibraryState,
    items: &ListedPages,
    selection: &Selection,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    reorderable: bool,
    folder_id: Option<String>,
) -> gpui_kit::AnyElement {
    let size = state.browse.thumbnail_size as f32;
    let show_names = state.browse.show_names;
    let view_id = state.view_id;
    div()
        .id("item-grid-wrap")
        .w_full()
        .flex()
        .flex_wrap()
        .gap_2()
        .p_2()
        .children(items.loaded().map(|(index, item)| {
            let selected = selection.contains(&item.cell.id);
            let entity_click = entity.clone();
            let entity_drag = entity.clone();
            let entity_drop = entity.clone();
            let title = format!("{}.{}", item.cell.name, item.cell.ext);
            let preview = thumb_or_palette(item, size, theme.muted, theme.muted_foreground);
            // A cell of a larger selection drags the selection (a Select All as a flag, D-167).
            let payload = match selection.target(view_id) {
                Some(target) if selected && selection.count() > 1 => {
                    DragPayload::selection(target, selection.count(), title.clone())
                }
                _ => DragPayload::items(view_id, vec![item.cell.id.clone()], title.clone()),
            }
            .with_thumb(item.thumb.clone());
            let mut cell = v_flex()
                .id(SharedString::from(format!("grid-{index}")))
                .w(px(size + 8.0))
                .gap_1()
                .rounded(theme.radius)
                .aria_label(title.clone())
                .when(selected, |el| el.border_2().border_color(theme.accent))
                .cursor_pointer()
                .on_click(move |event, _, cx| {
                    let mods = event.modifiers();
                    let double = event.click_count() >= 2;
                    entity_click.update(cx, |this, cx| {
                        this.select_item(index, mods.control, mods.shift, cx);
                        if double && !mods.control && !mods.shift {
                            this.open_preview_at(index, cx);
                        }
                    });
                });
            cell = attach_item_drag(cell, Some(payload), entity_drag);
            cell = attach_context_select(cell, entity_drop.clone(), index);
            if reorderable && let Some(folder_id) = folder_id.clone() {
                cell = attach_cell_drop(cell, entity_drop, folder_id, Some(item.cell.id.clone()));
            }
            cell.child(preview)
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
        .context_menu(ops_items::context_menu_builder(
            entity,
            ops_items::item_menu_model,
        ))
        .into_any_element()
}

/// A right click selects the cell before the grid's menu opens (D-140).
fn attach_context_select<E>(el: E, entity: Entity<RootApp>, index: usize) -> E
where
    E: InteractiveElement,
{
    el.on_mouse_down(MouseButton::Right, move |_, _, cx| {
        entity.update(cx, |this, cx| this.select_for_context(index, cx));
    })
}

fn attach_item_drag<E>(el: E, payload: Option<DragPayload>, entity: Entity<RootApp>) -> E
where
    E: InteractiveElement + StatefulInteractiveElement,
{
    let Some(payload) = payload else {
        return el;
    };
    el.on_drag(payload, {
        let entity = entity.clone();
        move |payload, point, window, cx| {
            entity.update(cx, |this, cx| {
                this.drag_active = true;
                this.rubber = None;
                cx.notify();
            });
            dnd::ghost(payload, point, window, cx)
        }
    })
}

fn attach_cell_drop<E>(
    el: E,
    entity: Entity<RootApp>,
    folder_id: String,
    before_id: Option<ItemId>,
) -> E
where
    E: InteractiveElement,
{
    let folder_for_can = folder_id.clone();
    let before_for_can = before_id.clone();
    let entity_can = entity.clone();
    el.can_drop(move |dragged, _, cx| {
        let Some(payload) = dragged.downcast_ref::<DragPayload>() else {
            return false;
        };
        entity_can.read(cx).can_accept_drop(
            payload,
            &DropIntent::Cell {
                folder_id: folder_for_can.clone(),
                before_id: before_for_can.clone(),
            },
        )
    })
    .on_drop(move |payload: &DragPayload, _, cx| {
        let intent = DropIntent::Cell {
            folder_id: folder_id.clone(),
            before_id: before_id.clone(),
        };
        entity.update(cx, |this, cx| {
            this.drag_active = false;
            this.handle_drop(payload, intent, cx);
        });
    })
}

/// The length of a video or audio item in the bottom right corner
/// of its cell (D-169).
fn duration_badge(ms: u64) -> gpui_kit::AnyElement {
    div()
        .absolute()
        .right(px(4.))
        .bottom(px(4.))
        .px(px(4.))
        .rounded(px(3.))
        .bg(gpui_kit::black().opacity(0.6))
        .text_color(gpui_kit::white())
        .text_size(px(10.))
        .line_height(px(14.))
        .child(crate::browse::media_bar::format_time(ms as f64 / 1000.0))
        .into_any_element()
}

/// How far the pointer is along a scrubbed video: a thin line at the bottom of the cell.
fn scrub_progress(fraction: f32) -> gpui_kit::AnyElement {
    div()
        .absolute()
        .left_0()
        .bottom_0()
        .h(px(2.))
        .w(gpui_kit::relative(fraction.clamp(0.0, 1.0)))
        .bg(gpui_kit::white().opacity(0.9))
        .into_any_element()
}

/// Invisible strips over a video's cell: the one under the pointer picks the frame shown
/// (D-170). Strips need no bounds or layout maths and leave clicks to the cell.
fn scrub_strips(cell: &ItemCell, edge: f32, entity: Entity<RootApp>) -> gpui_kit::AnyElement {
    h_flex()
        .absolute()
        .inset_0()
        .children((0..SCRUB_STRIPS).map(|strip| {
            let entity = entity.clone();
            let cell = cell.clone();
            div()
                .id(("scrub-strip", strip))
                .flex_1()
                .h_full()
                .on_hover(move |hovered, _, cx| {
                    if *hovered {
                        entity.update(cx, |this, cx| {
                            this.hover_scrub(&cell, Some(strip), edge, cx);
                        });
                    }
                })
        }))
        .into_any_element()
}

/// A thumbnail that covers its whole cell. The justified layout gives every cell the
/// item's aspect ratio, so drawing a square here leaves empty bands between cells.
fn thumb_fill(
    item: &ListedItem,
    width: f32,
    height: f32,
    muted: Hsla,
    muted_foreground: Hsla,
) -> gpui_kit::AnyElement {
    match &item.thumb {
        Some(image) => img(image.clone())
            .w(px(width))
            .h(px(height))
            .rounded(px(style::THUMB_RADIUS))
            .object_fit(gpui_kit::ObjectFit::Cover)
            .into_any_element(),
        None => {
            let color = item.cell.palette0.map_or(muted, |[r, g, b]| {
                rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)).into()
            });
            div()
                .w(px(width))
                .h(px(height))
                .rounded(px(style::THUMB_RADIUS))
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
