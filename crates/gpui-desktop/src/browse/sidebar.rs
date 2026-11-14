//! Left bar: places, then one scrolling column with folders, locked folders, smart folders
//! and tags (Phase 1 + 3, restyled in D-141).

use app_store::Locale;
use gpui_kit::assets::IconName;
use std::collections::{HashMap, HashSet};

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, Selectable as _, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, DragMoveEvent, Entity, Hsla, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};
use library_core::{Folder, TagGroup};

use crate::app::RootApp;
use crate::browse::style::{self, Kit};
use crate::dnd::{self, DragPayload, FolderZone};
use crate::i18n::t;
use crate::library::{self, OpenLibraryState, Place};
use crate::ops_dnd::DropIntent;
use crate::ops_mutations::NamePromptKind;
use crate::{ops_items, ops_organize};

/// The key of the ungrouped tags in [`RootApp::expanded_tag_groups`] (D-162). Not a ULID, so
/// it never meets a group id.
pub const UNGROUPED: &str = "ungrouped";

/// What the left bar shows besides the open library's own state.
pub struct SidebarExtras<'a> {
    pub tags: &'a [library_core::Tag],
    pub tag_groups: &'a [TagGroup],
    pub expanded_tag_groups: &'a HashSet<String>,
    pub name_input: Entity<InputState>,
    /// Which ⊕ has its name popover open.
    pub name_prompt: Option<NamePromptKind>,
    /// The Empty Trash confirmation open at the Trash row (D-165).
    pub trash_confirm: Option<library_core::TrashInfo>,
}

/// A left-bar row as the trigger of a popover the app opens itself (D-165): it is never drawn
/// as selected or open, and the popover listens to the middle button only, so a left click
/// still does what the row does.
struct RowTrigger(AnyElement);

impl gpui_kit::component::Selectable for RowTrigger {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

impl IntoElement for RowTrigger {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        self.0
    }
}

pub fn render_sidebar(
    state: &OpenLibraryState,
    locale: Locale,
    theme: &Kit,
    entity: Entity<RootApp>,
    extras: SidebarExtras<'_>,
) -> impl IntoElement {
    let tags = extras.tags;
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
        .child(trash_confirm_popover(
            scope_row(
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
            )
            // Size, Restore All and Empty Trash… (D-164); the confirmation opens here (D-165).
            .context_menu(ops_organize::trash_menu(entity.clone())),
            extras.trash_confirm,
            locale,
            theme,
            entity.clone(),
        ));

    let folders_header = {
        let mut header = style::section_header("folders-header", t(locale, "shell.folders"), theme);
        // Include Subfolders sits by the new-folder button (D-158). It only changes the
        // view, so a read-only library keeps it.
        let toggle = entity.clone();
        header = header.child(
            Button::new("sb-include-subfolders")
                .ghost()
                .xsmall()
                .icon(IconName::FolderTree)
                .tooltip(t(locale, "commands.view.includeSubfolders"))
                .selected(state.browse.include_subfolders)
                .on_click(move |_, _, cx| {
                    toggle.update(cx, |this, cx| this.toggle_include_subfolders(cx));
                }),
        );
        if !read_only {
            header = attach_drop(header, entity.clone(), DropIntent::Root);
            header = header.child(name_popover(
                NamePromptKind::NewFolder,
                locale,
                entity.clone(),
                &extras,
            ));
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
            header = header.child(name_popover(
                NamePromptKind::SmartFolder,
                locale,
                entity.clone(),
                &extras,
            ));
        }
        header
    };

    let tags_header = {
        let mut header = style::section_header("tags-header", t(locale, "shell.tags"), theme);
        if !read_only {
            // Opens under the button, its left edge on the button's (D-160).
            header = header.child(
                Button::new("sb-tag-groups")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Ellipsis)
                    .tooltip(t(locale, "tags.groups"))
                    .dropdown_menu_with_anchor(
                        gpui_kit::Anchor::TopLeft,
                        ops_organize::tags_menu(entity.clone()),
                    ),
            );
        }
        header
    };
    let tag_rows = render_tag_groups(state, locale, theme, entity.clone(), &extras);

    let show_smart = !state.smart_folders.is_empty() || !read_only;
    let show_tags = !tags.is_empty() || !extras.tag_groups.is_empty();

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
                // Room for the overlay scrollbar, so it never covers the counts (D-145).
                .pr(px(style::SCROLLBAR_GUTTER))
                .overflow_y_scrollbar()
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
                .when(show_tags, |el| el.child(tags_header).children(tag_rows)),
        )
}

/// The Trash row with the Empty Trash confirmation that its menu opens under it (D-165): what
/// goes, its size, that the files leave the disk for good; Cancel and Empty Trash.
fn trash_confirm_popover(
    row: impl IntoElement,
    confirm: Option<library_core::TrashInfo>,
    locale: Locale,
    theme: &Kit,
    entity: Entity<RootApp>,
) -> impl IntoElement {
    let close = entity.clone();
    let warning = theme.danger;
    Popover::new("trash-confirm-popover")
        .anchor(gpui_kit::Anchor::TopLeft)
        .mouse_button(gpui_kit::MouseButton::Middle)
        .trigger_style(gpui_kit::StyleRefinement::default().w_full())
        .open(confirm.is_some())
        .on_open_change(move |open, _, cx| {
            // Only the app opens it; a dismissal (Escape, a click outside) cancels.
            if !*open {
                close.update(cx, |this, cx| this.answer_empty_trash(false, cx));
            }
        })
        .trigger(RowTrigger(row.into_any_element()))
        .content(move |_, _, _| {
            let message = confirm
                .map(|info| {
                    t(locale, "trash.emptyMessage")
                        .replace("{what}", &ops_organize::trash_summary(locale, info))
                })
                .unwrap_or_default();
            let cancel = entity.clone();
            let empty = entity.clone();
            v_flex()
                .gap_2()
                .w(px(300.))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .small()
                                .flex_none()
                                .text_color(warning),
                        )
                        .child(Label::new(t(locale, "trash.emptyTitle")).font_semibold()),
                )
                .child(Label::new(message).text_sm())
                .child(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("trash-confirm-cancel")
                                .small()
                                .label(t(locale, "common.cancel"))
                                .on_click(move |_, _, cx| {
                                    cancel
                                        .update(cx, |this, cx| this.answer_empty_trash(false, cx));
                                }),
                        )
                        .child(
                            Button::new("trash-confirm-ok")
                                .small()
                                .danger()
                                .label(t(locale, "trash.empty"))
                                .on_click(move |_, _, cx| {
                                    empty.update(cx, |this, cx| this.answer_empty_trash(true, cx));
                                }),
                        ),
                )
        })
}

/// A ⊕ of the left bar with its name popover under it: the name, Cancel and Save (D-161,
/// D-163). Enter saves; Escape or a click outside closes it. One popover is open at a time.
fn name_popover(
    kind: NamePromptKind,
    locale: Locale,
    entity: Entity<RootApp>,
    extras: &SidebarExtras<'_>,
) -> impl IntoElement {
    let (id, title, tooltip) = match kind {
        NamePromptKind::NewFolder => ("sb-new-folder", "folder.newTitle", "commands.folder.new"),
        NamePromptKind::SmartFolder => ("sb-save-smart", "smart.saveTitle", "smart.save"),
    };
    let input = extras.name_input.clone();
    let open_change = entity.clone();
    Popover::new(SharedString::from(format!("{id}-popover")))
        .anchor(gpui_kit::Anchor::TopLeft)
        .open(extras.name_prompt == Some(kind))
        .on_open_change(move |open, window, cx| {
            let open = *open;
            open_change.update(cx, |this, cx| {
                this.set_name_prompt(open.then_some(kind), window, cx);
            });
        })
        .trigger(
            Button::new(id)
                .ghost()
                .xsmall()
                .icon(IconName::Plus)
                .tooltip(t(locale, tooltip)),
        )
        .content(move |_, _, _| {
            let cancel = entity.clone();
            let save = entity.clone();
            v_flex()
                .gap_2()
                .w(px(260.))
                .child(Label::new(t(locale, title)).text_sm())
                .child(Input::new(&input).small())
                .child(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new(SharedString::from(format!("{id}-cancel")))
                                .small()
                                .label(t(locale, "common.cancel"))
                                .on_click(move |_, window, cx| {
                                    cancel.update(cx, |this, cx| {
                                        this.set_name_prompt(None, window, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new(SharedString::from(format!("{id}-ok")))
                                .small()
                                .primary()
                                .label(t(locale, "common.save"))
                                .on_click(move |_, window, cx| {
                                    save.update(cx, |this, cx| {
                                        this.confirm_name_prompt(window, cx)
                                    });
                                }),
                        ),
                )
        })
}

/// The Tags section as Eagle draws it (D-162): each tag group as a row that unfolds into its
/// tags, tinted with the group's colour, then the tags in no group under "Ungrouped". Without
/// groups the tags are listed as before.
fn render_tag_groups(
    state: &OpenLibraryState,
    locale: Locale,
    theme: &Kit,
    entity: Entity<RootApp>,
    extras: &SidebarExtras<'_>,
) -> Vec<AnyElement> {
    let read_only = state.info.read_only;
    let counts: HashMap<&str, u32> = extras
        .tags
        .iter()
        .map(|tag| (tag.name.as_str(), tag.image_count.unwrap_or(0)))
        .collect();
    let tag = |name: &str, group: Option<(&TagGroup, Option<Hsla>)>| {
        tag_row(
            name.to_owned(),
            counts.get(name).copied().unwrap_or(0),
            state.filters.tags.iter().any(|t| t == name),
            read_only,
            theme,
            entity.clone(),
            group.map(|(group, tint)| (group.id.clone(), tint)),
        )
    };
    if extras.tag_groups.is_empty() {
        return extras
            .tags
            .iter()
            .take(200)
            .map(|t| tag(&t.name, None))
            .collect();
    }
    let mut rows = Vec::new();
    for group in extras.tag_groups {
        let expanded = extras.expanded_tag_groups.contains(&group.id);
        let tint = group
            .color
            .as_deref()
            .and_then(style::folder_color)
            .map(|hex| rgb(hex).into());
        rows.push(tag_group_row(
            &group.id,
            group.name.clone(),
            group.tags.len(),
            tint,
            expanded,
            read_only,
            theme,
            entity.clone(),
        ));
        if expanded {
            rows.extend(group.tags.iter().map(|name| tag(name, Some((group, tint)))));
        }
    }
    let ungrouped = ungrouped_tags(extras.tags, extras.tag_groups);
    if !ungrouped.is_empty() {
        let expanded = extras.expanded_tag_groups.contains(UNGROUPED);
        rows.push(tag_group_row(
            UNGROUPED,
            t(locale, "tags.ungrouped"),
            ungrouped.len(),
            None,
            expanded,
            true,
            theme,
            entity.clone(),
        ));
        if expanded {
            rows.extend(ungrouped.iter().take(200).map(|t| tag(&t.name, None)));
        }
    }
    rows
}

/// The tags no group holds, in vocabulary order. Names match exactly: `"Poster"` in a group
/// leaves `"poster"` ungrouped (D-057).
fn ungrouped_tags<'a>(
    tags: &'a [library_core::Tag],
    groups: &[TagGroup],
) -> Vec<&'a library_core::Tag> {
    let grouped: HashSet<&str> = groups
        .iter()
        .flat_map(|group| group.tags.iter().map(String::as_str))
        .collect();
    tags.iter()
        .filter(|tag| !grouped.contains(tag.name.as_str()))
        .collect()
}

/// A tag group's row: a click folds or unfolds it; its menu filters, renames, colours or
/// deletes the group. `fixed` rows (Ungrouped, a read-only library) have no menu.
#[allow(clippy::too_many_arguments)]
fn tag_group_row(
    id: &str,
    name: String,
    count: usize,
    tint: Option<Hsla>,
    expanded: bool,
    fixed: bool,
    theme: &Kit,
    entity: Entity<RootApp>,
) -> AnyElement {
    let toggle = entity.clone();
    let toggle_id = id.to_owned();
    let row = style::nav_row(SharedString::from(format!("tag-group-{id}")), false, theme)
        .on_click(move |_, _, cx| {
            toggle.update(cx, |this, cx| this.toggle_tag_group(&toggle_id, cx));
        })
        .child(
            div().w(px(14.)).flex_none().child(
                Icon::new(if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .xsmall()
                .text_color(theme.muted_foreground),
            ),
        )
        .child(
            Icon::new(IconName::Tags)
                .small()
                .flex_none()
                .text_color(tint.unwrap_or(theme.muted_foreground)),
        )
        .child(style::row_label(name))
        .child(style::row_count(count as u32, theme));
    if fixed {
        row.into_any_element()
    } else {
        row.context_menu(ops_organize::tag_group_menu(entity, id.to_owned()))
            .into_any_element()
    }
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
            // Opens under the button (D-160). The wrapper keeps the click from also opening or
            // unlocking the folder through the row.
            div()
                .id(SharedString::from(format!(
                    "locked-manage-wrap-{manage_id}"
                )))
                .on_click(|_, _, cx| cx.stop_propagation())
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
                        .dropdown_menu_with_anchor(
                            gpui_kit::Anchor::TopLeft,
                            ops_organize::lock_manage_menu(entity, manage_id),
                        ),
                ),
        )
        .into_any_element()
}

/// A tag: a click filters by it, items dropped on it get it.
/// Inside a group (`group`: its id and colour) the row is indented and tinted; its menu adds
/// the tag to a group or takes it out of this one (D-162).
fn tag_row(
    name: String,
    count: u32,
    active: bool,
    read_only: bool,
    theme: &Kit,
    entity: Entity<RootApp>,
    group: Option<(String, Option<Hsla>)>,
) -> AnyElement {
    let filter_tag = name.clone();
    let intent = DropIntent::Tag { name: name.clone() };
    let click = entity.clone();
    let tint = group
        .as_ref()
        .and_then(|(_, tint)| *tint)
        .unwrap_or(theme.muted_foreground);
    let key = match &group {
        Some((group_id, _)) => format!("tag-{group_id}-{name}"),
        None => format!("tag-{name}"),
    };
    let mut row = style::nav_row(SharedString::from(key), active, theme)
        .when(group.is_some(), |row| row.pl(px(26.)))
        .on_click(move |_, _, cx| {
            click.update(cx, |this, cx| {
                this.toggle_tag_filter(filter_tag.clone(), cx);
            });
        })
        .child(
            Icon::new(IconName::Tag)
                .small()
                .flex_none()
                .text_color(tint),
        )
        .child(style::row_label(name.clone()))
        .child(style::row_count(count, theme));
    if read_only {
        return row.into_any_element();
    }
    row = attach_drop(row, entity.clone(), intent);
    row.context_menu(ops_organize::tag_row_menu(
        entity,
        name,
        group.map(|(id, _)| id),
    ))
    .into_any_element()
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
) -> gpui_kit::Stateful<gpui_kit::Div> {
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
    // Items dropped here join the folder; a folder dragged here shows the zone lines instead.
    let el = drop_highlight(el, false);
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

/// The row under a drag of items is tinted, so it is clear where they will go (D-152).
fn drop_highlight<E: InteractiveElement>(el: E, folders_too: bool) -> E {
    el.drag_over::<DragPayload>(move |style, payload, _, cx| {
        let accepts = folders_too || matches!(payload, DragPayload::Items { .. });
        if !accepts {
            return style;
        }
        let tint =
            crate::browse::style::selection_color(gpui_kit::component::ActiveTheme::theme(cx));
        style.bg(tint.opacity(0.22))
    })
}

fn attach_drop<E>(el: E, entity: Entity<RootApp>, intent: DropIntent) -> E
where
    E: InteractiveElement,
{
    let intent_can = intent.clone();
    let entity_can = entity.clone();
    let folders_too = matches!(intent, DropIntent::Root);
    let el = drop_highlight(el, folders_too);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ungrouped_tags_are_the_ones_no_group_holds_exactly() {
        let tag = |name: &str| library_core::Tag {
            name: name.into(),
            image_count: Some(1),
        };
        let tags = [tag("Poster"), tag("poster"), tag("Logo"), tag("Кот")];
        let groups = [
            TagGroup {
                id: "G1".into(),
                name: "Design".into(),
                color: Some("red".into()),
                tags: vec!["Poster".into(), "Logo".into()],
            },
            TagGroup {
                id: "G2".into(),
                name: "Empty".into(),
                color: None,
                tags: vec![],
            },
        ];
        let names: Vec<&str> = ungrouped_tags(&tags, &groups)
            .into_iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(names, ["poster", "Кот"]);
        assert_eq!(ungrouped_tags(&tags, &[]).len(), 4);
        assert_ne!(UNGROUPED.len(), 26, "never the length of a ULID group id");
    }
}
