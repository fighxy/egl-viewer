//! Details panel with in-place / dialog edits (Phase 2 read + Phase 3 write).

use std::sync::Arc;

use app_store::Locale;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::color_picker::ColorPicker;
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::label::Label;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    Disableable as _, Icon, Selectable as _, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Entity, Image, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, StyledImage as _, div, img, px, rgb,
};
use library_core::{Folder, Item, ItemCell, SelectionTags, SortKey};

use crate::app::RootApp;
use crate::browse::format::{format_bytes, format_date, format_dimensions};
use crate::browse::picker::{Picker, PickerHandles, PickerKind};
use crate::browse::style;
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
    pickers: &PickerHandles,
) -> gpui_kit::AnyElement {
    let in_trash = matches!(state.place, crate::library::Place::Trash);
    let read_only = state.info.read_only;
    let open_folder = match &state.place {
        crate::library::Place::Folder { id } => find_folder(state.folders.as_slice(), id),
        _ => None,
    };

    v_flex()
        .id("details-panel")
        .size_full()
        .p_3()
        .gap_3()
        .overflow_y_scrollbar()
        .child(match model.count {
            0 => {
                if let Some(folder) = open_folder {
                    folder_properties(
                        folder,
                        locale,
                        theme,
                        entity,
                        read_only,
                        state.browse.include_subfolders,
                        // Eagle knows only its eight colours: no picker there (D-159).
                        (state.info.kind == library_core::LibraryKind::Native)
                            .then(|| pickers.folder_color.clone()),
                    )
                    .into_any_element()
                } else {
                    empty_placeholder(locale, theme).into_any_element()
                }
            }
            1 => {
                if let Some(item) = model.item.as_ref() {
                    single_item(
                        item,
                        model.thumb.clone(),
                        &state.folders,
                        locale,
                        theme,
                        entity,
                        in_trash,
                        read_only,
                        pickers,
                    )
                    .into_any_element()
                } else {
                    Label::new(t(locale, "details.couldNotRead"))
                        .text_color(theme.muted_foreground)
                        .into_any_element()
                }
            }
            n => multi_select(
                n, model, locale, theme, entity, in_trash, read_only, pickers,
            )
            .into_any_element(),
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
        .child(Label::new(t(locale, "details.selectItem")).text_color(theme.muted_foreground))
}

#[allow(clippy::too_many_arguments)]
fn multi_select(
    count: usize,
    model: &DetailsModel,
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    in_trash: bool,
    read_only: bool,
    pickers: &PickerHandles,
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
                format!("{base} · {}", format_bytes(Some(size.unwrap_or(0))))
            })
            .text_sm()
            .text_color(theme.muted_foreground),
        )
        .when(!read_only && !in_trash, |el| {
            el.child(rating_editor(
                locale,
                None,
                theme.muted_foreground,
                entity.clone(),
            ))
        })
        .when_some(model.selection.as_ref(), |el, sel| {
            el.child(tags_editor(
                locale,
                theme,
                &sel.tags,
                sel.items,
                entity.clone(),
                read_only || in_trash,
                pickers,
            ))
        })
}

#[allow(clippy::too_many_arguments)]
fn single_item(
    item: &Item,
    thumb: Option<Arc<Image>>,
    folders: &[Folder],
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    in_trash: bool,
    read_only: bool,
    pickers: &PickerHandles,
) -> impl IntoElement {
    let preview_entity = entity.clone();
    let open_link_url = item.url.clone();
    let has_link = library_core::is_web_link(&item.url);
    let can_edit = !read_only && !in_trash && !item.is_deleted;
    let exif = exif_rows(item, locale, theme);

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
                    t(locale, "details.inTrash")
                })
                .text_xs()
                .text_color(theme.muted_foreground),
            )
        })
        .child(inline_field(
            locale,
            theme,
            "details.name",
            if can_edit {
                Input::new(&pickers.editors.name).small().into_any_element()
            } else {
                Label::new(item.name.clone()).text_sm().into_any_element()
            },
        ))
        .child(if can_edit {
            rating_editor(
                locale,
                Some(item.star),
                theme.muted_foreground,
                entity.clone(),
            )
            .into_any_element()
        } else {
            field(
                locale,
                theme,
                "details.rating",
                if item.star == 0 {
                    t(locale, "details.none")
                } else {
                    "★".repeat(item.star as usize)
                },
            )
            .into_any_element()
        })
        .child(tags_editor(
            locale,
            theme,
            &item
                .tags
                .iter()
                .map(|name| library_core::Tag {
                    name: name.clone(),
                    image_count: Some(1),
                })
                .collect::<Vec<_>>(),
            1,
            entity.clone(),
            !can_edit,
            pickers,
        ))
        .child(folders_editor(
            locale,
            theme,
            folders,
            &item.folders,
            entity.clone(),
            !can_edit,
            pickers,
        ))
        .child(inline_field(
            locale,
            theme,
            "details.note",
            if can_edit {
                Textarea::new(&pickers.editors.note)
                    .small()
                    .into_any_element()
            } else {
                read_only_value(locale, theme, &item.annotation)
            },
        ))
        .child(inline_field(
            locale,
            theme,
            "details.link",
            if can_edit {
                Input::new(&pickers.editors.link).small().into_any_element()
            } else {
                read_only_value(locale, theme, &item.url)
            },
        ))
        .when(has_link, |el| {
            let entity = entity.clone();
            el.child(
                Button::new("details-open-link")
                    .ghost()
                    .label(t(locale, "details.openLink"))
                    .on_click(move |_, _, cx| {
                        let url = open_link_url.clone();
                        entity.update(cx, |this, cx| this.open_item_link(&url, cx));
                    }),
            )
        })
        .child(section_header(
            "details-info-header",
            t(locale, "details.info"),
            pickers.info_collapsed,
            theme,
            {
                let entity = entity.clone();
                move |cx| entity.update(cx, |this, cx| this.toggle_details_section(false, cx))
            },
        ))
        .when(!pickers.info_collapsed, |el| {
            el.child(
                v_flex()
                    .gap_1()
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
                    .child(field(
                        locale,
                        theme,
                        "details.added",
                        format_date(item.added_at),
                    ))
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
        })
        .when(!exif.is_empty(), |el| {
            el.child(section_header(
                "details-exif-header",
                "EXIF".to_owned(),
                pickers.exif_collapsed,
                theme,
                {
                    let entity = entity.clone();
                    move |cx| entity.update(cx, |this, cx| this.toggle_details_section(true, cx))
                },
            ))
            .when(!pickers.exif_collapsed, |el| {
                el.child(v_flex().gap_1().children(exif))
            })
        })
}

/// A section title of the right bar that folds its section (D-151): a chevron and the name,
/// a click anywhere on the row.
fn section_header(
    id: &'static str,
    title: String,
    collapsed: bool,
    theme: &gpui_kit::component::theme::Theme,
    on_toggle: impl Fn(&mut gpui_kit::App) + 'static,
) -> impl IntoElement {
    let hover = theme.list_hover;
    h_flex()
        .id(id)
        .w_full()
        .gap_1()
        .py(px(3.))
        .rounded(theme.radius)
        .cursor_pointer()
        .hover(move |el| el.bg(hover))
        .child(
            Icon::new(if collapsed {
                IconName::ChevronRight
            } else {
                IconName::ChevronDown
            })
            .small()
            .text_color(theme.muted_foreground),
        )
        .child(Label::new(title).text_sm().font_semibold())
        .on_click(move |_, _, cx| on_toggle(cx))
}

/// A labelled field whose value is edited in place (D-151).
fn inline_field(
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    label_key: &'static str,
    content: gpui_kit::AnyElement,
) -> impl IntoElement {
    h_flex()
        .id(SharedString::from(format!("ifield-{label_key}")))
        .gap_2()
        .items_start()
        .child(
            Label::new(t(locale, label_key))
                .text_sm()
                .text_color(theme.muted_foreground)
                .w(px(72.))
                .pt(px(4.)),
        )
        .child(div().flex_1().min_w_0().child(content))
}

/// A value that cannot be edited (read-only library, the trash): the text, or "None".
fn read_only_value(
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    value: &str,
) -> gpui_kit::AnyElement {
    if value.is_empty() {
        Label::new(t(locale, "details.none"))
            .text_sm()
            .text_color(theme.muted_foreground)
            .into_any_element()
    } else {
        Label::new(value.to_owned()).text_sm().into_any_element()
    }
}

fn rating_editor(
    locale: Locale,
    current: Option<u8>,
    muted: gpui_kit::Hsla,
    entity: Entity<RootApp>,
) -> impl IntoElement {
    h_flex()
        .id("rating-editor")
        .gap_1()
        .items_center()
        .child(Label::new(t(locale, "details.rating")).text_sm().w(px(72.)))
        .children((1..=5u8).map(|star| {
            // Five stars in a row, filled up to the rating. A click on the current rating
            // removes it, as in Eagle.
            let filled = current.is_some_and(|rating| star <= rating);
            let is_current = current == Some(star);
            let entity = entity.clone();
            Button::new(SharedString::from(format!("star-{star}")))
                .ghost()
                .small()
                .compact()
                .text_color(if filled {
                    gpui_kit::rgb(0xf5b301).into()
                } else {
                    muted
                })
                .label(if filled { "★" } else { "☆" })
                .on_click(move |_, _, cx| {
                    let next = if is_current { 0 } else { star };
                    entity.update(cx, |this, cx| this.set_rating(next, cx));
                })
        }))
}

/// Removes a chip's value from the selection.
type RemoveFn = Box<dyn Fn(&mut gpui_kit::App) + 'static>;

/// A value of the selection as a chip; × removes it (D-149).
fn chip(
    id: SharedString,
    label: String,
    dimmed: bool,
    theme: &gpui_kit::component::theme::Theme,
    on_remove: Option<RemoveFn>,
) -> impl IntoElement {
    h_flex()
        .id(id.clone())
        .gap_1()
        .pl_2()
        .pr(px(if on_remove.is_some() { 3. } else { 8. }))
        .py(px(2.))
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.secondary)
        .child(
            Label::new(label)
                .text_sm()
                .when(dimmed, |el| el.text_color(theme.muted_foreground)),
        )
        .when_some(on_remove, |el, remove| {
            el.child(
                Button::new(SharedString::from(format!("{id}-x")))
                    .ghost()
                    .xsmall()
                    .icon(IconName::X)
                    .on_click(move |_, _, cx| remove(cx)),
            )
        })
}

/// The "+" of a section: a popover with the picker under it, opening to the left.
fn picker_popover(
    id: &'static str,
    kind: PickerKind,
    trigger: Button,
    picker: Entity<Picker>,
    open: bool,
    entity: Entity<RootApp>,
) -> impl IntoElement {
    Popover::new(id)
        .anchor(gpui_kit::Anchor::TopRight)
        .open(open)
        .on_open_change(move |open, window, cx| {
            let open = *open;
            entity.update(cx, |this, cx| this.set_picker_open(kind, open, window, cx));
        })
        .trigger(trigger)
        .content(move |_, _, _| picker.clone())
}

#[allow(clippy::too_many_arguments)]
fn tags_editor(
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    tags: &[library_core::Tag],
    selected_items: u32,
    entity: Entity<RootApp>,
    read_only: bool,
    pickers: &PickerHandles,
) -> impl IntoElement {
    let add = picker_popover(
        "tag-picker-popover",
        PickerKind::Tags,
        Button::new("add-tag")
            .ghost()
            .small()
            .icon(IconName::Plus)
            .tooltip(t(locale, "details.addTag")),
        pickers.tags.clone(),
        pickers.tags_open,
        entity.clone(),
    );
    v_flex()
        .id("tags-editor")
        .gap_1p5()
        .child(
            Label::new(t(locale, "details.tags"))
                .text_xs()
                .text_color(theme.muted_foreground),
        )
        .when(tags.is_empty() && read_only, |el| {
            el.child(
                Label::new(t(locale, "details.none"))
                    .text_sm()
                    .text_color(theme.muted_foreground),
            )
        })
        .when(!tags.is_empty(), |el| {
            el.child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .items_center()
                    .children(tags.iter().map(|tag| {
                        let name = tag.name.clone();
                        // On some of the selected items only.
                        let dimmed = tag.image_count.is_some_and(|n| n > 0 && n < selected_items);
                        let remove: Option<RemoveFn> = (!read_only).then(|| {
                            let entity = entity.clone();
                            let name = name.clone();
                            Box::new(move |cx: &mut gpui_kit::App| {
                                entity.update(cx, |this, cx| {
                                    this.remove_tag_from_selection(name.clone(), cx)
                                });
                            }) as RemoveFn
                        });
                        chip(
                            SharedString::from(format!("tag-{name}")),
                            name,
                            dimmed,
                            theme,
                            remove,
                        )
                    }))
                    .when(!read_only, |el| el.child(add)),
            )
        })
        .when(tags.is_empty() && !read_only, |el| {
            el.child(
                div().w_full().child(picker_popover(
                    "tag-picker-popover",
                    PickerKind::Tags,
                    Button::new("add-tag")
                        .ghost()
                        .w_full()
                        .icon(IconName::Plus)
                        .label(t(locale, "details.addTag")),
                    pickers.tags.clone(),
                    pickers.tags_open,
                    entity.clone(),
                )),
            )
        })
}

/// The item's folders as chips; "+" opens the folder picker (D-149).
fn folders_editor(
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    folders: &[Folder],
    ids: &[String],
    entity: Entity<RootApp>,
    read_only: bool,
    pickers: &PickerHandles,
) -> impl IntoElement {
    v_flex()
        .id("folders-editor")
        .gap_1p5()
        .child(
            Label::new(t(locale, "details.folders"))
                .text_xs()
                .text_color(theme.muted_foreground),
        )
        .child(
            h_flex()
                .gap_1()
                .flex_wrap()
                .items_center()
                .when(ids.is_empty() && read_only, |el| {
                    el.child(
                        Label::new(t(locale, "details.none"))
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                })
                .children(ids.iter().map(|id| {
                    let name = find_folder_name(folders, id)
                        .unwrap_or_else(|| t(locale, "details.unknownFolder"));
                    let remove: Option<RemoveFn> = (!read_only).then(|| {
                        let entity = entity.clone();
                        let id = id.clone();
                        Box::new(move |cx: &mut gpui_kit::App| {
                            entity.update(cx, |this, cx| {
                                this.apply_edit_to_selection(
                                    library_core::ItemEdit::RemoveFromFolders(vec![id.clone()]),
                                    cx,
                                )
                            });
                        }) as RemoveFn
                    });
                    chip(
                        SharedString::from(format!("folder-{id}")),
                        name,
                        false,
                        theme,
                        remove,
                    )
                }))
                .when(!read_only, |el| {
                    el.child(picker_popover(
                        "folder-picker-popover",
                        PickerKind::Folders,
                        Button::new("add-folder")
                            .ghost()
                            .small()
                            .icon(IconName::Plus)
                            .tooltip(t(locale, "details.addToFolder")),
                        pickers.folders.clone(),
                        pickers.folders_open,
                        entity.clone(),
                    ))
                }),
        )
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

fn find_folder_name(folders: &[Folder], id: &str) -> Option<String> {
    find_folder(folders, id).map(|f| f.name.clone())
}

fn find_folder<'a>(folders: &'a [Folder], id: &str) -> Option<&'a Folder> {
    for folder in folders {
        if folder.id == id {
            return Some(folder);
        }
        if let Some(found) = find_folder(&folder.children, id) {
            return Some(found);
        }
    }
    None
}

fn folder_properties(
    folder: &Folder,
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    read_only: bool,
    include_subfolders: bool,
    color_picker: Option<Entity<gpui_kit::component::color_picker::ColorPickerState>>,
) -> impl IntoElement {
    let id = folder.id.clone();
    let color = folder.icon_color.clone();
    // A custom colour gets its own ringed dot after the eight (D-159).
    let custom = color
        .as_deref()
        .filter(|c| !library_core::FOLDER_COLORS.contains(c))
        .and_then(style::folder_color);
    let order = folder.order_by;
    v_flex()
        .id("folder-properties")
        .gap_3()
        .child(Label::new(folder.name.clone()).font_bold())
        .child(
            Label::new(format!(
                "{} · {}",
                t_n(locale, "folder.itemsHere", folder.image_count.unwrap_or(0)),
                t_n(
                    locale,
                    "folder.itemsWithSubs",
                    folder.descendant_image_count.unwrap_or(0)
                )
            ))
            .text_xs()
            .text_color(theme.muted_foreground),
        )
        .when(!folder.description.is_empty(), |el| {
            el.child(Label::new(folder.description.clone()).text_sm())
        })
        .when(!read_only, |el| {
            let desc = entity.clone();
            let fid = id.clone();
            el.child(
                Button::new("fp-desc")
                    .ghost()
                    .label(t(locale, "folder.editDescription"))
                    .on_click(move |_, window, cx| {
                        desc.update(cx, |this, cx| {
                            this.edit_folder_description(window, cx, fid.clone());
                        });
                    }),
            )
        })
        .child(Label::new(t(locale, "folder.colour")).text_xs())
        .child(
            // Colour dots, the chosen one ringed. The first, hollow dot is "no colour".
            h_flex()
                .gap_2()
                .flex_wrap()
                .items_center()
                .children(
                    std::iter::once(None)
                        .chain(library_core::FOLDER_COLORS.iter().copied().map(Some))
                        .map(|c| {
                            let entity = entity.clone();
                            let fid = id.clone();
                            let selected = color.as_deref() == c;
                            let label = c.unwrap_or("none");
                            let fill = c.and_then(style::folder_color);
                            div()
                                .id(SharedString::from(format!("fp-color-{label}")))
                                .size(px(18.))
                                .rounded_full()
                                .border_2()
                                .border_color(if selected {
                                    theme.foreground
                                } else if fill.is_some() {
                                    gpui_kit::transparent_black()
                                } else {
                                    theme.muted_foreground
                                })
                                .when_some(fill, |el, hex| el.bg(gpui_kit::rgb(hex)))
                                .when(!read_only, |el| el.cursor_pointer())
                                .aria_label(label)
                                .on_click(move |_, _, cx| {
                                    if read_only {
                                        return;
                                    }
                                    entity.update(cx, |this, cx| {
                                        this.set_folder_color(
                                            fid.clone(),
                                            c.map(str::to_owned),
                                            cx,
                                        );
                                    });
                                })
                        }),
                )
                .when_some(custom, |el, hex| {
                    el.child(
                        div()
                            .id("fp-color-custom")
                            .size(px(18.))
                            .rounded_full()
                            .border_2()
                            .border_color(theme.foreground)
                            .bg(gpui_kit::rgb(hex))
                            .aria_label(color.clone().unwrap_or_default()),
                    )
                })
                .when_some(color_picker.filter(|_| !read_only), |el, picker| {
                    el.child(
                        ColorPicker::new(&picker)
                            .xsmall()
                            .icon(IconName::Palette)
                            .accessibility_label(t(locale, "folder.customColour")),
                    )
                }),
        )
        .when(!folder.tags.is_empty(), |el| {
            el.child(
                Label::new(format!(
                    "{}: {}",
                    t(locale, "folder.tags"),
                    folder.tags.join(", ")
                ))
                .text_sm(),
            )
        })
        .child(Label::new(t(locale, "sort.title")).text_xs())
        .child(
            h_flex().gap_1().flex_wrap().children(
                [
                    (None, "sort.global"),
                    (Some(SortKey::Manual), "sort.manual"),
                    (Some(SortKey::DateAdded), "sort.added"),
                    (Some(SortKey::Title), "sort.name"),
                    (Some(SortKey::Rating), "sort.star"),
                ]
                .into_iter()
                .map(|(key, label)| {
                    let entity = entity.clone();
                    let fid = id.clone();
                    let selected = order == key;
                    Button::new(SharedString::from(format!("fp-sort-{label}")))
                        .ghost()
                        .small()
                        .selected(selected)
                        .label(t(locale, label))
                        .disabled(read_only)
                        .on_click(move |_, _, cx| {
                            if read_only {
                                return;
                            }
                            entity.update(cx, |this, cx| {
                                this.set_folder_sort(fid.clone(), key, key.map(|_| true), cx);
                            });
                        })
                }),
            ),
        )
        .when(order == Some(SortKey::Manual) && !read_only, |el| {
            el.child(
                Label::new(if include_subfolders {
                    t(locale, "folder.manualHintSubs")
                } else {
                    t(locale, "folder.manualHint")
                })
                .text_xs()
                .text_color(theme.muted_foreground),
            )
        })
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
                .w(px(72.))
                .h(px(96.))
                .mr_1()
                .rounded(theme.radius)
                .bg(theme.muted.opacity(0.35))
                .overflow_hidden()
                .child(match thumb {
                    Some(image) => img(image)
                        .object_fit(gpui_kit::ObjectFit::Cover)
                        .size_full()
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
    type ExifFormat = fn(&str) -> String;
    type ExifDef = (&'static str, &'static str, Option<ExifFormat>);
    let defs: [ExifDef; 6] = [
        ("camera", "details.camera", None),
        ("isoSpeed", "details.iso", None),
        ("aperture", "details.aperture", Some(|v| format!("ƒ/{v}"))),
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
    rows
}
