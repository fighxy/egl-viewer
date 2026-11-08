//! Left bar: scopes, folders, locked folders, smart folders (Phase 1 + 3).

use app_store::Locale;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{h_flex, label::Label, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use library_core::Folder;

use crate::app::RootApp;
use crate::i18n::t;
use crate::library::{self, OpenLibraryState, Place};

pub fn render_sidebar(
    state: &OpenLibraryState,
    locale: Locale,
    entity: Entity<RootApp>,
) -> impl IntoElement {
    let unlocked = library::unlocked_tree(&state.folders);
    let locked = library::locked_folders(&state.folders);

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
        ))
        .child(scope_row(
            "uncategorized",
            t(locale, "shell.uncategorized"),
            state.counts.uncategorized,
            matches!(state.place, Place::Uncategorized),
            entity.clone(),
            Place::Uncategorized,
        ))
        .child(scope_row(
            "untagged",
            t(locale, "shell.untagged"),
            state.counts.untagged,
            matches!(state.place, Place::Untagged),
            entity.clone(),
            Place::Untagged,
        ))
        .child(scope_row(
            "trash",
            t(locale, "shell.trash"),
            state.counts.trash,
            matches!(state.place, Place::Trash),
            entity.clone(),
            Place::Trash,
        ))
        .child(
            h_flex()
                .id("folders-header")
                .w_full()
                .items_center()
                .mt_3()
                .mb_1()
                .child(Label::new(t(locale, "shell.folders")).text_xs().flex_1())
                .when(!state.info.read_only, |el| {
                    let new_folder = entity.clone();
                    let rename = entity.clone();
                    let delete = entity.clone();
                    let lock = entity.clone();
                    el.child(
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
                    )
                }),
        )
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
            let entity = entity.clone();
            el.child(
                Button::new("sb-save-smart")
                    .ghost()
                    .label(t(locale, "smart.save"))
                    .on_click(move |_, window, cx| {
                        entity.update(cx, |this, cx| this.save_as_smart_folder(window, cx));
                    }),
            )
        })
}

fn locked_row(folder: Folder, locale: Locale, entity: Entity<RootApp>) -> AnyElement {
    let id = folder.id.clone();
    let name = folder
        .lock_label
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| folder.name.clone());
    let unlocked = folder.is_unlocked;
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
                entity.update(cx, |this, cx| {
                    this.set_place(
                        Place::Folder {
                            id: id.clone(),
                        },
                        cx,
                    );
                });
            } else {
                entity.update(cx, |this, cx| {
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
        .into_any_element()
}

fn scope_row(
    id: &'static str,
    label: String,
    count: u32,
    selected: bool,
    entity: Entity<RootApp>,
    place: Place,
) -> impl IntoElement {
    h_flex()
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
            entity.update(cx, |this, cx| this.set_place(place, cx));
        })
        .child(Label::new(label).text_sm())
        .child(Label::new(count.to_string()).text_xs())
}

fn render_folders(
    folders: &[Folder],
    place: &Place,
    expanded: &std::collections::HashSet<String>,
    include_subfolders: bool,
    depth: u32,
    entity: Entity<RootApp>,
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

        out.push(
            h_flex()
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
                .child(Label::new(count.to_string()).text_xs())
                .into_any_element(),
        );

        if has_children && is_expanded {
            out.extend(render_folders(
                &folder.children,
                place,
                expanded,
                include_subfolders,
                depth + 1,
                entity.clone(),
            ));
        }
    }
    out
}
