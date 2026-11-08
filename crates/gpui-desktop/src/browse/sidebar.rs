//! Left bar: scopes, folders, locked folders, smart folders, tags (Phase 1 + 3).

use app_store::Locale;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{h_flex, label::Label, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, DragMoveEvent, Entity, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use library_core::Folder;

use crate::app::RootApp;
use crate::dnd::{self, DragPayload, FolderZone};
use crate::i18n::t;
use crate::library::{self, OpenLibraryState, Place};
use crate::ops_dnd::DropIntent;

pub fn render_sidebar(
    state: &OpenLibraryState,
    locale: Locale,
    entity: Entity<RootApp>,
    tags: &[library_core::Tag],
) -> impl IntoElement {
    let unlocked = library::unlocked_tree(&state.folders);
    let locked = library::locked_folders(&state.folders);
    let read_only = state.info.read_only;

    v_flex()
        .id("sidebar")
        .size_full()
        .gap_1()
        .p_2()
        .child(scope_row(
            "all",
            t(locale, "shell.all"),
            state.counts.all,
            matches!(state.place, Place::All),
            entity.clone(),
            Place::All,
            None,
        ))
        .child(scope_row(
            "uncategorized",
            t(locale, "shell.uncategorized"),
            state.counts.uncategorized,
            matches!(state.place, Place::Uncategorized),
            entity.clone(),
            Place::Uncategorized,
            None,
        ))
        .child(scope_row(
            "untagged",
            t(locale, "shell.untagged"),
            state.counts.untagged,
            matches!(state.place, Place::Untagged),
            entity.clone(),
            Place::Untagged,
            None,
        ))
        .child(scope_row(
            "trash",
            t(locale, "shell.trash"),
            state.counts.trash,
            matches!(state.place, Place::Trash),
            entity.clone(),
            Place::Trash,
            if read_only {
                None
            } else {
                Some(DropIntent::Trash)
            },
        ))
        .child({
            let root_entity = entity.clone();
            let mut header = h_flex()
                .id("folders-header")
                .w_full()
                .items_center()
                .mt_3()
                .mb_1()
                .child(Label::new(t(locale, "shell.folders")).text_xs().flex_1());
            if !read_only {
                header = attach_drop(
                    header,
                    root_entity.clone(),
                    DropIntent::Root,
                );
                let new_folder = entity.clone();
                let rename = entity.clone();
                let delete = entity.clone();
                let lock = entity.clone();
                header = header
                    .child(
                        Button::new("sb-new-folder")
                            .ghost()
                            .label("+")
                            .on_click(move |_, window, cx| {
                                new_folder.update(cx, |this, cx| this.create_folder(window, cx));
                            }),
                    )
                    .child(
                        Button::new("sb-rename-folder")
                            .ghost()
                            .label("Aa")
                            .on_click(move |_, window, cx| {
                                rename.update(cx, |this, cx| this.rename_open_folder(window, cx));
                            }),
                    )
                    .child(
                        Button::new("sb-del-folder")
                            .ghost()
                            .label("Del")
                            .on_click(move |_, window, cx| {
                                delete.update(cx, |this, cx| this.delete_open_folder(window, cx));
                            }),
                    )
                    .child(
                        Button::new("sb-lock-folder")
                            .ghost()
                            .label("Lock")
                            .on_click(move |_, window, cx| {
                                lock.update(cx, |this, cx| this.lock_open_folder(window, cx));
                            }),
                    );
            }
            header
        })
        .child(
            div()
                .id("folder-tree")
                .flex_1()
                .w_full()
                .overflow_y_scroll()
                .children(render_folders(
                    &unlocked,
                    &state.place,
                    &state.expanded,
                    state.browse.include_subfolders,
                    0,
                    entity.clone(),
                    read_only,
                )),
        )
        .when(!locked.is_empty(), |el| {
            el.child(
                Label::new(t(locale, "shell.lockedFolders"))
                    .text_xs()
                    .mt_3()
                    .mb_1(),
            )
            .children(locked.into_iter().map(|folder| {
                locked_row(folder, locale, entity.clone())
            }))
        })
        .when(!tags.is_empty() && !read_only, |el| {
            el.child(
                Label::new(t(locale, "shell.tags"))
                    .text_xs()
                    .mt_3()
                    .mb_1(),
            )
            .children(tags.iter().take(40).map(|tag| {
                tag_drop_row(
                    tag.name.clone(),
                    tag.image_count.unwrap_or(0),
                    entity.clone(),
                )
            }))
        })
        .when(!state.smart_folders.is_empty(), |el| {
            el.child(
                Label::new(t(locale, "shell.smartFolders"))
                    .text_xs()
                    .mt_3()
                    .mb_1(),
            )
            .children(state.smart_folders.iter().map(|smart| {
                let id = smart.id.clone();
                let selected =
                    matches!(state.place, Place::Smart { id: ref open } if open == &smart.id);
                let entity = entity.clone();
                h_flex()
                    .id(SharedString::from(format!("smart-{id}")))
                    .w_full()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .when(selected, |el| el.border_1())
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.set_place(Place::Smart { id: id.clone() }, cx);
                        });
                    })
                    .child(Label::new(smart.name.clone()).text_sm())
                    .into_any_element()
            }))
        })
        .when(!state.info.read_only, |el| {
            let save = entity.clone();
            let groups = entity.clone();
            el.child(
                Button::new("sb-save-smart")
                    .ghost()
                    .label(t(locale, "smart.save"))
                    .on_click(move |_, window, cx| {
                        save.update(cx, |this, cx| this.save_as_smart_folder(window, cx));
                    }),
            )
            .child(
                Button::new("sb-tag-groups")
                    .ghost()
                    .label(t(locale, "tags.groups"))
                    .on_click(move |_, window, cx| {
                        groups.update(cx, |this, cx| this.open_tag_groups_dialog(window, cx));
                    }),
            )
        })
}

fn locked_row(folder: Folder, locale: Locale, entity: Entity<RootApp>) -> AnyElement {
    let id = folder.id.clone();
    let manage_id = folder.id.clone();
    let name = folder
        .lock_label
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| folder.name.clone());
    let unlocked = folder.is_unlocked;
    let click = entity.clone();
    h_flex()
        .id(SharedString::from(format!("locked-{id}")))
        .w_full()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            if unlocked {
                click.update(cx, |this, cx| {
                    this.set_place(
                        Place::Folder {
                            id: id.clone(),
                        },
                        cx,
                    );
                });
            } else {
                click.update(cx, |this, cx| {
                    this.unlock_folder_dialog(window, cx, id.clone());
                });
            }
        })
        .child(Label::new(name).text_sm().flex_1())
        .child(
            Label::new(if unlocked {
                t(locale, "shell.unlocked")
            } else {
                t(locale, "shell.locked")
            })
            .text_xs(),
        )
        .child(
            Button::new(SharedString::from(format!("locked-manage-{manage_id}")))
                .ghost()
                .label("…")
                .on_click(move |_, window, cx| {
                    entity.update(cx, |this, cx| {
                        this.open_lock_manage_dialog(window, cx, manage_id.clone());
                    });
                }),
        )
        .into_any_element()
}

fn tag_drop_row(name: String, count: u32, entity: Entity<RootApp>) -> AnyElement {
    let filter_tag = name.clone();
    let intent = DropIntent::Tag { name: name.clone() };
    let click = entity.clone();
    let mut row = h_flex()
        .id(SharedString::from(format!("tag-{name}")))
        .w_full()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .on_click(move |_, _, cx| {
            click.update(cx, |this, cx| {
                this.toggle_tag_filter(filter_tag.clone(), cx);
            });
        })
        .child(Label::new(name).text_sm())
        .child(Label::new(count.to_string()).text_xs());
    row = attach_drop(row, entity, intent);
    row.into_any_element()
}

fn scope_row(
    id: &'static str,
    label: String,
    count: u32,
    selected: bool,
    entity: Entity<RootApp>,
    place: Place,
    drop: Option<DropIntent>,
) -> impl IntoElement {
    let click = entity.clone();
    let mut row = h_flex()
        .id(SharedString::from(id))
        .w_full()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .rounded_md()
        .when(selected, |el| el.border_1())
        .cursor_pointer()
        .on_click(move |_event: &gpui_kit::ClickEvent, _window: &mut Window, cx: &mut App| {
            let place = place.clone();
            click.update(cx, |this, cx| this.set_place(place, cx));
        })
        .child(Label::new(label).text_sm())
        .child(Label::new(count.to_string()).text_xs());
    if let Some(intent) = drop {
        row = attach_drop(row, entity, intent);
    }
    row
}

fn render_folders(
    folders: &[Folder],
    place: &Place,
    expanded: &std::collections::HashSet<String>,
    include_subfolders: bool,
    depth: u32,
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

        let mut row = h_flex()
            .id(SharedString::from(format!("folder-{id}")))
            .w_full()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .pl(px(8.0 + depth as f32 * 12.0))
            .rounded_md()
            .when(selected, |el| el.border_1())
            .cursor_pointer()
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
                let label = if is_expanded { "▾" } else { "▸" };
                div()
                    .id(SharedString::from(format!("expand-{id}")))
                    .w(px(16.))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        entity_toggle.update(cx, |this, cx| {
                            if let Some(lib) = this.library.as_mut() {
                                if !lib.expanded.remove(&toggle_id) {
                                    lib.expanded.insert(toggle_id.clone());
                                }
                                cx.notify();
                            }
                        });
                    })
                    .child(Label::new(label).text_xs())
                    .into_any_element()
            } else {
                div().w(px(16.)).into_any_element()
            })
            .child(Label::new(name).text_sm().flex_1())
            .child(Label::new(count.to_string()).text_xs());

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
            DragPayload::Items { .. } => entity_can.read(cx).can_accept_drop(
                payload,
                &DropIntent::Folder {
                    id: id_can.clone(),
                },
            ),
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
