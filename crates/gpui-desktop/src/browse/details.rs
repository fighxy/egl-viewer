//! Details panel with in-place / dialog edits (Phase 2 read + Phase 3 write).

use std::sync::Arc;

use app_store::Locale;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::{Disableable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Entity, Image, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, StyledImage as _, div, img, px, rgb,
};
use library_core::{Folder, Item, ItemCell, SelectionTags, SortKey};

use crate::app::RootApp;
use crate::browse::format::{format_bytes, format_date, format_dimensions};
use crate::i18n::{t, t_n};
use crate::library::OpenLibraryState;
use crate::ops::EditField;

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
        .overflow_y_scroll()
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
                    )
                    .into_any_element()
                } else {
                    Label::new(t(locale, "details.couldNotRead"))
                        .text_color(theme.muted_foreground)
                        .into_any_element()
                }
            }
            n => multi_select(n, model, locale, theme, entity, in_trash, read_only)
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
    read_only: bool,
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
            el.child(rating_editor(locale, None, entity.clone()))
        })
        .when_some(model.selection.as_ref(), |el, sel| {
            el.child(tags_editor(
                locale,
                theme,
                &sel.tags,
                sel.items,
                entity.clone(),
                read_only || in_trash,
            ))
        })
        .child(action_buttons(locale, entity, true, in_trash, read_only))
}

fn single_item(
    item: &Item,
    thumb: Option<Arc<Image>>,
    folders: &[Folder],
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
    in_trash: bool,
    read_only: bool,
) -> impl IntoElement {
    let preview_entity = entity.clone();
    let open_link_url = item.url.clone();
    let has_link = library_core::is_web_link(&item.url);
    let can_edit = !read_only && !in_trash && !item.is_deleted;

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
        .child(editable_field(
            locale,
            theme,
            "details.name",
            item.name.clone(),
            can_edit,
            EditField::Name,
            entity.clone(),
        ))
        .child(if can_edit {
            rating_editor(locale, Some(item.star), entity.clone()).into_any_element()
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
        ))
        .child(field(
            locale,
            theme,
            "details.folders",
            folder_names(folders, &item.folders, locale),
        ))
        .child(editable_field(
            locale,
            theme,
            "details.note",
            if item.annotation.is_empty() {
                t(locale, "details.none")
            } else {
                item.annotation.clone()
            },
            can_edit,
            EditField::Note,
            entity.clone(),
        ))
        .child(editable_field(
            locale,
            theme,
            "details.link",
            if item.url.is_empty() {
                t(locale, "details.none")
            } else {
                item.url.clone()
            },
            can_edit,
            EditField::Link,
            entity.clone(),
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
        .child(action_buttons(
            locale,
            entity,
            false,
            in_trash || item.is_deleted,
            read_only,
        ))
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

fn editable_field(
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    label_key: &'static str,
    value: impl Into<SharedString>,
    can_edit: bool,
    field_kind: EditField,
    entity: Entity<RootApp>,
) -> impl IntoElement {
    h_flex()
        .id(SharedString::from(format!("efield-{label_key}")))
        .gap_2()
        .items_start()
        .child(
            Label::new(t(locale, label_key))
                .text_sm()
                .text_color(theme.muted_foreground)
                .w(px(72.)),
        )
        .child(Label::new(value).text_sm().flex_1())
        .when(can_edit, |el| {
            el.child(
                Button::new(SharedString::from(format!("edit-{label_key}")))
                    .ghost()
                    .label(t(locale, "common.edit"))
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| {
                            this.open_edit_field_dialog(window, cx, field_kind);
                        });
                    }),
            )
        })
}

fn rating_editor(locale: Locale, current: Option<u8>, entity: Entity<RootApp>) -> impl IntoElement {
    h_flex()
        .id("rating-editor")
        .gap_1()
        .items_center()
        .child(
            Label::new(t(locale, "details.rating"))
                .text_sm()
                .w(px(72.)),
        )
        .children((0..=5).map(|star| {
            let selected = current == Some(star);
            let entity = entity.clone();
            Button::new(SharedString::from(format!("star-{star}")))
                .when(selected, |b| b.primary())
                .when(!selected, |b| b.ghost())
                .label(if star == 0 { "0".into() } else { "★".repeat(star as usize) })
                .on_click(move |_, _, cx| {
                    entity.update(cx, |this, cx| this.set_rating(star, cx));
                })
        }))
}

fn tags_editor(
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    tags: &[library_core::Tag],
    selected_items: u32,
    entity: Entity<RootApp>,
    read_only: bool,
) -> impl IntoElement {
    v_flex()
        .id("tags-editor")
        .gap_1()
        .child(
            Label::new(t(locale, "details.tags"))
                .text_xs()
                .text_color(theme.muted_foreground),
        )
        .child(
            h_flex()
                .gap_1()
                .flex_wrap()
                .when(tags.is_empty(), |el| {
                    el.child(
                        Label::new(t(locale, "details.none"))
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                })
                .children(tags.iter().map(|tag| {
                    let name = tag.name.clone();
                    let entity = entity.clone();
                    let dimmed = tag
                        .image_count
                        .is_some_and(|n| n > 0 && n < selected_items);
                    Button::new(SharedString::from(format!("tag-{name}")))
                        .ghost()
                        .label(if dimmed {
                            format!("({name})")
                        } else {
                            name.clone()
                        })
                        .when(!read_only, |b| {
                            b.on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.remove_tag_from_selection(name.clone(), cx);
                                });
                            })
                        })
                })),
        )
        .when(!read_only, |el| {
            let entity = entity.clone();
            el.child(
                Button::new("add-tag")
                    .outline()
                    .label(t(locale, "details.addTag"))
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| {
                            this.open_add_tag_dialog(window, cx);
                        });
                    }),
            )
        })
}

fn action_buttons(
    locale: Locale,
    entity: Entity<RootApp>,
    several: bool,
    in_trash: bool,
    read_only: bool,
) -> impl IntoElement {
    h_flex()
        .id("details-actions")
        .gap_1()
        .flex_wrap()
        .when(!in_trash, |el| {
            let reveal = entity.clone();
            let open = entity.clone();
            let copy = entity.clone();
            let trash = entity.clone();
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
            .when(!read_only, |el| {
                el.child(
                    Button::new("details-trash")
                        .outline()
                        .label(t(locale, "trash.move"))
                        .on_click(move |_, window, cx| {
                            trash.update(cx, |this, cx| this.trash_selection(window, cx));
                        }),
                )
            })
        })
        .when(in_trash && !read_only, |el| {
            let restore = entity.clone();
            let delete = entity.clone();
            let empty = entity.clone();
            el.child(
                Button::new("details-restore")
                    .primary()
                    .label(t(locale, "trash.restore"))
                    .on_click(move |_, _, cx| {
                        restore.update(cx, |this, cx| this.restore_selection(cx));
                    }),
            )
            .child(
                Button::new("details-delete")
                    .outline()
                    .label(t(locale, "trash.delete"))
                    .on_click(move |_, window, cx| {
                        delete.update(cx, |this, cx| {
                            this.delete_permanently_selection(window, cx);
                        });
                    }),
            )
            .child(
                Button::new("details-empty")
                    .ghost()
                    .label(t(locale, "trash.empty"))
                    .on_click(move |_, window, cx| {
                        empty.update(cx, |this, cx| this.empty_trash(window, cx));
                    }),
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
) -> impl IntoElement {
    let id = folder.id.clone();
    let color = folder.icon_color.clone();
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
            h_flex().gap_1().children(
                std::iter::once(None)
                    .chain(library_core::FOLDER_COLORS.iter().copied().map(Some))
                    .map(|c| {
                        let entity = entity.clone();
                        let fid = id.clone();
                        let selected = color.as_deref() == c;
                        let label = c.unwrap_or("none");
                        Button::new(SharedString::from(format!("fp-color-{label}")))
                            .when(selected, |b| b.primary())
                            .when(!selected, |b| b.ghost())
                            .label(label)
                            .disabled(read_only)
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
            ),
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
            h_flex().gap_1().children(
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
                        .when(selected, |b| b.primary())
                        .when(!selected, |b| b.ghost())
                        .label(t(locale, label))
                        .disabled(read_only)
                        .on_click(move |_, _, cx| {
                            if read_only {
                                return;
                            }
                            entity.update(cx, |this, cx| {
                                this.set_folder_sort(
                                    fid.clone(),
                                    key,
                                    key.map(|_| true),
                                    cx,
                                );
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
