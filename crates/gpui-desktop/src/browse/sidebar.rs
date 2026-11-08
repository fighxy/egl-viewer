//! Left bar: scopes + read-only folder tree.

use app_store::Locale;
use gpui_kit::component::{h_flex, label::Label, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use library_core::Folder;

use crate::app::RootApp;
use crate::i18n::t;
use crate::library::{OpenLibraryState, Place};

pub fn render_sidebar(
    state: &OpenLibraryState,
    locale: Locale,
    entity: Entity<RootApp>,
) -> impl IntoElement {
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
            Label::new(t(locale, "shell.folders"))
                .text_xs()
                .mt_3()
                .mb_1(),
        )
        .child(
            div()
                .id("folder-tree")
                .flex_1()
                .w_full()
                .overflow_y_scroll()
                .children(render_folders(
                    &state.folders,
                    &state.place,
                    &state.expanded,
                    state.browse.include_subfolders,
                    0,
                    entity,
                )),
        )
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
            entity
                .update(cx, |this, cx| this.set_place(place, cx))
                ;
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
                    entity_place
                        .update(cx, |this, cx| {
                            this.set_place(
                                Place::Folder {
                                    id: place_id.clone(),
                                },
                                cx,
                            );
                        })
                        ;
                })
                .child(if has_children {
                    let label = if is_expanded { "▾" } else { "▸" };
                    div()
                        .id(SharedString::from(format!("expand-{id}")))
                        .w(px(16.))
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            entity_toggle
                                .update(cx, |this, cx| {
                                    if let Some(lib) = this.library.as_mut() {
                                        if !lib.expanded.remove(&toggle_id) {
                                            lib.expanded.insert(toggle_id.clone());
                                        }
                                        cx.notify();
                                    }
                                })
                                ;
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
