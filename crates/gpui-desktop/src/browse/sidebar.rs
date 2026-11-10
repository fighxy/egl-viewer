//! Left bar: places, then one scrolling column with folders, locked folders, smart folders
//! and tags (D-129 + D-131, restyled in D-141).

use app_store::Locale;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::{Icon, Sizable as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, DragMoveEvent, Entity, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use library_core::Folder;

use crate::app::RootApp;
use crate::browse::style::{self, Kit};
use crate::dnd::{self, DragPayload, FolderZone};
use crate::i18n::t;
use crate::library::{self, OpenLibraryState, Place};
use crate::ops_dnd::DropIntent;
use crate::ops_items;

pub fn render_sidebar(
    state: &OpenLibraryState,
    locale: Locale,
    theme: &Kit,
    entity: Entity<RootApp>,
    tags: &[library_core::Tag],
) -> impl IntoElement {
    let unlocked = library::unlocked_tree(&state.folders);
    let locked = library::locked_folders(&state.folders);
    let read_only = state.info.read_only;

    let places = v_flex()
        .id("sidebar-places")
        .w_full()
        .gap(px(1.))
        .child(scope_row(
            "all",
            IconName::Images,
            t(locale, "shell.all"),
            state.counts.all,
            matches!(state.place, Place::All),
            theme,
            entity.clone(),
            Place::All,
            None,
        ))
        .child(scope_row(
            "uncategorized",
            IconName::FolderX,
            t(locale, "shell.uncategorized"),
            state.counts.uncategorized,
            matches!(state.place, Place::Uncategorized),
            theme,
            entity.clone(),
            Place::Uncategorized,
            None,
        ))
        .child(scope_row(
            "untagged",
            IconName::Bookmark,
            t(locale, "shell.untagged"),
            state.counts.untagged,
            matches!(state.place, Place::Untagged),
            theme,
            entity.clone(),
            Place::Untagged,
            None,
        ))
        .child(scope_row(
            "trash",
            IconName::Trash,
            t(locale, "shell.trash"),
            state.counts.trash,
            matches!(state.place, Place::Trash),
            theme,
            entity.clone(),
            Place::Trash,
            if read_only {
                None
            } else {
                Some(DropIntent::Trash)
            },
        ));

    let folders_header = {
        let mut header = style::section_header("folders-header", t(locale, "shell.folders"), theme);
        if !read_only {
            header = attach_drop(header, entity.clone(), DropIntent::Root);
            let new_folder = entity.clone();
            header = header.child(
                Button::new("sb-new-folder")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .tooltip(t(locale, "commands.folder.new"))
                    .on_click(move |_, window, cx| {
                        new_folder.update(cx, |this, cx| this.create_folder(window, cx));
                    }),
            );
        }
        header
    };

    let folder_tree = v_flex()
        .id("folder-tree")
        .w_full()
        // A scrolling column shrinks its children to fit unless they refuse: without this
        // the tree collapsed and the sections below were drawn over its rows.
        .flex_none()
        .gap(px(1.))
        .min_h(px(24.))
        .children(render_folders(
            &unlocked,
            &state.place,
            &state.expanded,
            state.browse.include_subfolders,
            0,
            theme,
            entity.clone(),
            read_only,
        ))
        .context_menu(ops_items::context_menu_builder(
            entity.clone(),
            ops_items::folder_menu_model,
        ));

    let smart_header = {
        let mut header =
            style::section_header("smart-header", t(locale, "shell.smartFolders"), theme);
        if !read_only {
            let save = entity.clone();
            header = header.child(
                Button::new("sb-save-smart")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .tooltip(t(locale, "smart.save"))
                    .on_click(move |_, window, cx| {
                        save.update(cx, |this, cx| this.save_as_smart_folder(window, cx));
                    }),
            );
        }
        header
    };

    let tags_header = {
        let mut header = style::section_header("tags-header", t(locale, "shell.tags"), theme);
        if !read_only {
            let groups = entity.clone();
            header = header.child(
                Button::new("sb-tag-groups")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Ellipsis)
                    .tooltip(t(locale, "tags.groups"))
                    .on_click(move |_, window, cx| {
                        groups.update(cx, |this, cx| this.open_tag_groups_dialog(window, cx));
                    }),
            );
        }
        header
    };

    let show_smart = !state.smart_folders.is_empty() || !read_only;
    let show_tags = !tags.is_empty();

    v_flex()
        .id("sidebar")
        .size_full()
        .min_h(px(0.))
        .p_2()
        .bg(theme.sidebar)
        .child(places)
        .child(
            v_flex()
                .id("sidebar-scroll")
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .overflow_y_scroll()
                .child(folders_header)
                .child(folder_tree)
                .when(!locked.is_empty(), |el| {
                    el.child(style::section_header(
                        "locked-header",
                        t(locale, "shell.lockedFolders"),
                        theme,
                    ))
                    .children(
                        locked
                            .into_iter()
                            .map(|folder| locked_row(folder, locale, theme, entity.clone())),
                    )
                })
                .when(show_smart, |el| {
                    el.child(smart_header)
                        .children(state.smart_folders.iter().map(|smart| {
                            let id = smart.id.clone();
                            let selected = matches!(
                                state.place,
                                Place::Smart { id: ref open } if open == &smart.id
                            );
                            let entity = entity.clone();
                            style::nav_row(
                                SharedString::from(format!("smart-{id}")),
                                selected,
                                theme,
                            )
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.set_place(Place::Smart { id: id.clone() }, cx);
                                });
                            })
                            .child(style::row_icon(IconName::FolderSearch, theme))
                            .child(style::row_label(smart.name.clone()))
                            .into_any_element()
                        }))
                })
                .when(show_tags, |el| {
                    el.child(tags_header)
                        .children(tags.iter().take(200).map(|tag| {
                            tag_row(
                                tag.name.clone(),
                                tag.image_count.unwrap_or(0),
                                state.filters.tags.contains(&tag.name),
                                read_only,
                                theme,
                                entity.clone(),
                            )
                        }))
                }),
        )
}

fn locked_row(folder: Folder, locale: Locale, theme: &Kit, entity: Entity<RootApp>) -> AnyElement {
    let id = folder.id.clone();
    let manage_id = folder.id.clone();
    let name = folder
        .lock_label
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| folder.name.clone());
    let unlocked = folder.is_unlocked;
    let click = entity.clone();
    style::nav_row(SharedString::from(format!("locked-{id}")), false, theme)
        .on_click(move |_, window, cx| {
            if unlocked {
                click.update(cx, |this, cx| {
                    this.set_place(Place::Folder { id: id.clone() }, cx);
                });
            } else {
                click.update(cx, |this, cx| {
                    this.unlock_folder_dialog(window, cx, id.clone());
                });
            }
        })
        .child(style::row_icon(
            if unlocked {
                IconName::LockOpen
            } else {
                IconName::Lock
            },
            theme,
        ))
        .child(style::row_label(name))
        .child(
            Button::new(SharedString::from(format!("locked-manage-{manage_id}")))
                .ghost()
                .xsmall()
                .icon(IconName::Ellipsis)
                .tooltip(t(
                    locale,
                    if unlocked {
                        "shell.unlocked"
                    } else {
                        "shell.locked"
                    },
                ))
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    entity.update(cx, |this, cx| {
                        this.open_lock_manage_dialog(window, cx, manage_id.clone());
                    });
                }),
        )
        .into_any_element()
}

/// A tag: a click filters by it, items dropped on it get it.
fn tag_row(
    name: String,
    count: u32,
    active: bool,
    read_only: bool,
    theme: &Kit,
    entity: Entity<RootApp>,
) -> AnyElement {
    let filter_tag = name.clone();
    let intent = DropIntent::Tag { name: name.clone() };
    let click = entity.clone();
    let mut row = style::nav_row(SharedString::from(format!("tag-{name}")), active, theme)
        .on_click(move |_, _, cx| {
            click.update(cx, |this, cx| {
                this.toggle_tag_filter(filter_tag.clone(), cx);
            });
        })
        .child(style::row_icon(IconName::Tag, theme))
        .child(style::row_label(name))
        .child(style::row_count(count, theme));
    if !read_only {
        row = attach_drop(row, entity, intent);
    }
    row.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn scope_row(
    id: &'static str,
    icon: IconName,
    label: String,
    count: u32,
    selected: bool,
    theme: &Kit,
    entity: Entity<RootApp>,
    place: Place,
    drop: Option<DropIntent>,
) -> impl IntoElement {
    let click = entity.clone();
    let mut row = style::nav_row(SharedString::from(id), selected, theme)
        .on_click(
            move |_event: &gpui_kit::ClickEvent, _window: &mut Window, cx: &mut App| {
                let place = place.clone();
                click.update(cx, |this, cx| this.set_place(place, cx));
            },
        )
        .child(style::row_icon(icon, theme))
        .child(style::row_label(label))
        .child(style::row_count(count, theme));
    if let Some(intent) = drop {
        row = attach_drop(row, entity, intent);
    }
    row
}

#[allow(clippy::too_many_arguments)]
fn render_folders(
    folders: &[Folder],
    place: &Place,
    expanded: &std::collections::HashSet<String>,
    include_subfolders: bool,
    depth: u32,
    theme: &Kit,
    entity: Entity<RootApp>,
    read_only: bool,
) -> Vec<AnyElement> {
    let mut out = Vec::new();
    for folder in folders {
        let id = folder.id.clone();
        let selected = matches!(place, Place::Folder { id: open } if open == &folder.id);
        let is_expanded = expanded.contains(&folder.id);
        let has_children = !folder.children.is_empty();
        let count = if include_subfolders {
            folder.descendant_image_count.unwrap_or(0)
        } else {
            folder.image_count.unwrap_or(0)
        };
        let name = folder.name.clone();
        let place_id = id.clone();
        let toggle_id = id.clone();
        let entity_place = entity.clone();
        let entity_toggle = entity.clone();
        let drag_payload = if read_only {
            None
        } else {
            Some(DragPayload::folder(id.clone(), name.clone()))
        };

        let mut row = style::nav_row(SharedString::from(format!("folder-{id}")), selected, theme)
            .pl(px(4.0 + depth as f32 * 14.0))
            .on_click(move |_, _, cx| {
                entity_place.update(cx, |this, cx| {
                    this.set_place(
                        Place::Folder {
                            id: place_id.clone(),
                        },
                        cx,
                    );
                });
            })
            .child(if has_children {
                div()
                    .id(SharedString::from(format!("expand-{id}")))
                    .w(px(14.))
                    .flex_none()
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        entity_toggle.update(cx, |this, cx| {
                            if let Some(lib) = this.library.as_mut() {
                                if !lib.expanded.remove(&toggle_id) {
                                    lib.expanded.insert(toggle_id.clone());
                                }
                                cx.notify();
                            }
                        });
                    })
                    .child(
                        Icon::new(if is_expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .xsmall()
                        .text_color(theme.muted_foreground),
                    )
                    .into_any_element()
            } else {
                div().w(px(14.)).flex_none().into_any_element()
            })
            .child(style::folder_icon(
                folder.icon_color.as_deref(),
                selected || is_expanded,
                theme,
            ))
            .child(style::row_label(name))
            .child(style::row_count(count, theme));

        row = row.on_mouse_down(gpui_kit::MouseButton::Right, {
            let entity = entity.clone();
            let id = id.clone();
            move |_, _, cx| {
                entity.update(cx, |this, cx| this.open_folder_for_context(id.clone(), cx));
            }
        });
        if let Some(payload) = drag_payload {
            row = attach_folder_drag(row, payload, entity.clone());
        }
        if !read_only {
            row = attach_folder_drop(row, id.clone(), entity.clone());
        }

        out.push(row.into_any_element());

        if has_children && is_expanded {
            out.extend(render_folders(
                &folder.children,
                place,
                expanded,
                include_subfolders,
                depth + 1,
                theme,
                entity.clone(),
                read_only,
            ));
        }
    }
    out
}

fn attach_folder_drag<E>(el: E, payload: DragPayload, entity: Entity<RootApp>) -> E
where
    E: InteractiveElement + StatefulInteractiveElement,
{
    el.on_drag(payload, {
        let entity = entity.clone();
        move |payload, point, window, cx| {
            entity.update(cx, |this, cx| {
                this.drag_active = true;
                this.rubber = None;
                this.folder_drop_zone = None;
                cx.notify();
            });
            dnd::ghost(payload, point, window, cx)
        }
    })
}

fn attach_folder_drop<E>(el: E, folder_id: String, entity: Entity<RootApp>) -> E
where
    E: InteractiveElement + StatefulInteractiveElement,
{
    let id_move = folder_id.clone();
    let entity_move = entity.clone();
    let id_can = folder_id.clone();
    let entity_can = entity.clone();
    el.on_drag_move(move |event: &DragMoveEvent<DragPayload>, _, cx| {
        let height = event.bounds.size.height.as_f32().max(1.0);
        let y = event.event.position.y.as_f32() - event.bounds.origin.y.as_f32();
        let zone = dnd::folder_zone((y / height).clamp(0.0, 1.0));
        entity_move.update(cx, |this, cx| {
            this.folder_drop_zone = Some((id_move.clone(), zone));
            cx.notify();
        });
    })
    .can_drop(move |dragged, _, cx| {
        let Some(payload) = dragged.downcast_ref::<DragPayload>() else {
            return false;
        };
        match payload {
            DragPayload::Folder { .. } => {
                let zone = entity_can
                    .read(cx)
                    .folder_drop_zone
                    .as_ref()
                    .filter(|(id, _)| id == &id_can)
                    .map(|(_, z)| *z)
                    .unwrap_or(FolderZone::Inside);
                entity_can.read(cx).can_accept_drop(
                    payload,
                    &DropIntent::FolderNest {
                        parent: id_can.clone(),
                        zone,
                    },
                )
            }
            DragPayload::Items { .. } => entity_can
                .read(cx)
                .can_accept_drop(payload, &DropIntent::Folder { id: id_can.clone() }),
        }
    })
    .on_drop(move |payload: &DragPayload, _, cx| {
        entity.update(cx, |this, cx| {
            let intent = match payload {
                DragPayload::Folder { .. } => {
                    let zone = this
                        .folder_drop_zone
                        .as_ref()
                        .filter(|(id, _)| id == &folder_id)
                        .map(|(_, z)| *z)
                        .unwrap_or(FolderZone::Inside);
                    DropIntent::FolderNest {
                        parent: folder_id.clone(),
                        zone,
                    }
                }
                DragPayload::Items { .. } => DropIntent::Folder {
                    id: folder_id.clone(),
                },
            };
            this.drag_active = false;
            this.folder_drop_zone = None;
            this.handle_drop(payload, intent, cx);
        });
    })
}

fn attach_drop<E>(el: E, entity: Entity<RootApp>, intent: DropIntent) -> E
where
    E: InteractiveElement,
{
    let intent_can = intent.clone();
    let entity_can = entity.clone();
    el.can_drop(move |dragged, _, cx| {
        let Some(payload) = dragged.downcast_ref::<DragPayload>() else {
            return false;
        };
        entity_can.read(cx).can_accept_drop(payload, &intent_can)
    })
    .on_drop(move |payload: &DragPayload, _, cx| {
        let intent = intent.clone();
        entity.update(cx, |this, cx| {
            this.drag_active = false;
            this.folder_drop_zone = None;
            this.handle_drop(payload, intent, cx);
        });
    })
}
