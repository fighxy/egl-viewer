//! Tag and folder pickers of the right bar, in Eagle's manner (D-149): a popover under the
//! "+" with a search field, the values to check, and keyboard hints. Tags come in sections
//! (Recent, each tag group, Other) in two columns; folders as the tree, or a flat list of
//! matches while searching. A click or Enter checks or unchecks a value on the selection and
//! the popover stays open; a search with no exact match offers to create that tag.

use std::collections::HashSet;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, SharedString, StatefulInteractiveElement as _, Styled, Subscription, WeakEntity,
    Window, div, px,
};
use library_core::{Folder, ItemEdit, Tag, TagGroup};

use crate::app::RootApp;
use crate::i18n::t;
use crate::ops_items::flatten_pickable;

/// What a picker edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Tags,
    Folders,
    /// The tags of one tag group, from the group's menu in the left bar (D-187).
    GroupTags,
}

impl PickerKind {
    /// Cells a row of the list holds.
    pub fn columns(self) -> usize {
        match self {
            Self::Tags | Self::GroupTags => 2,
            Self::Folders => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionTitle {
    Recent,
    /// A tag group: its + adds a typed tag to it (D-187).
    Group {
        id: String,
        name: String,
    },
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerRow {
    /// The tag's name or the folder's id.
    pub key: String,
    pub label: String,
    pub count: Option<u32>,
    /// Folder depth; 0 while searching (the label is then the path).
    pub depth: u32,
    /// The selection has it.
    pub on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerSection {
    pub title: Option<SectionTitle>,
    pub rows: Vec<PickerRow>,
}

fn matches(text: &str, query: &str) -> bool {
    query.is_empty() || text.to_lowercase().contains(&query.to_lowercase())
}

fn tag_row(tag: &Tag, on: &HashSet<String>) -> PickerRow {
    PickerRow {
        key: tag.name.clone(),
        label: tag.name.clone(),
        count: tag.image_count.filter(|n| *n > 0),
        depth: 0,
        on: on.contains(&tag.name),
    }
}

/// Tag sections. Without a query: Recent, every non-empty group, then the tags of no group.
/// With one: the matches in a single section, the ones that start with it first.
pub fn tag_sections(
    vocabulary: &[Tag],
    groups: &[TagGroup],
    recent: &[String],
    on: &HashSet<String>,
    query: &str,
) -> Vec<PickerSection> {
    let query = query.trim();
    let find = |name: &str| vocabulary.iter().find(|tag| tag.name == name);
    if !query.is_empty() {
        let lower = query.to_lowercase();
        let mut rows: Vec<&Tag> = vocabulary
            .iter()
            .filter(|tag| matches(&tag.name, query))
            .collect();
        rows.sort_by_key(|tag| !tag.name.to_lowercase().starts_with(&lower));
        return vec![PickerSection {
            title: None,
            rows: rows.into_iter().map(|tag| tag_row(tag, on)).collect(),
        }];
    }
    let mut sections = Vec::new();
    let recent: Vec<PickerRow> = recent
        .iter()
        .filter_map(|name| find(name))
        .map(|tag| tag_row(tag, on))
        .collect();
    if !recent.is_empty() {
        sections.push(PickerSection {
            title: Some(SectionTitle::Recent),
            rows: recent,
        });
    }
    let mut grouped = HashSet::new();
    for group in groups {
        let rows: Vec<PickerRow> = group
            .tags
            .iter()
            .filter_map(|name| find(name))
            .map(|tag| tag_row(tag, on))
            .collect();
        grouped.extend(group.tags.iter().cloned());
        // An empty group keeps its header: its + is how it gets a first tag (D-187).
        sections.push(PickerSection {
            title: Some(SectionTitle::Group {
                id: group.id.clone(),
                name: group.name.clone(),
            }),
            rows,
        });
    }
    let other: Vec<PickerRow> = vocabulary
        .iter()
        .filter(|tag| !grouped.contains(&tag.name))
        .map(|tag| tag_row(tag, on))
        .collect();
    if !other.is_empty() {
        sections.push(PickerSection {
            title: Some(SectionTitle::Other),
            rows: other,
        });
    }
    sections
}

/// Folder sections: Recent, then the tree; while searching, the matches by path. Locked
/// folders are not offered (D-059).
pub fn folder_sections(
    folders: &[Folder],
    recent: &[String],
    on: &HashSet<String>,
    query: &str,
) -> Vec<PickerSection> {
    let query = query.trim();
    let entries = flatten_pickable(folders);
    let row = |entry: &crate::ops_items::FolderEntry, flat: bool| PickerRow {
        key: entry.id.clone(),
        label: if flat {
            entry.path.clone()
        } else {
            entry.name.clone()
        },
        count: None,
        depth: if flat { 0 } else { entry.depth },
        on: on.contains(&entry.id),
    };
    if !query.is_empty() {
        return vec![PickerSection {
            title: None,
            rows: entries
                .iter()
                .filter(|entry| matches(&entry.name, query))
                .map(|entry| row(entry, true))
                .collect(),
        }];
    }
    let mut sections = Vec::new();
    let recent: Vec<PickerRow> = recent
        .iter()
        .filter_map(|id| entries.iter().find(|entry| entry.id == *id))
        .map(|entry| row(entry, true))
        .collect();
    if !recent.is_empty() {
        sections.push(PickerSection {
            title: Some(SectionTitle::Recent),
            rows: recent,
        });
    }
    if !entries.is_empty() {
        sections.push(PickerSection {
            title: None,
            rows: entries.iter().map(|entry| row(entry, false)).collect(),
        });
    }
    sections
}

/// The picker of a group's tags (D-187): the vocabulary plus any name the group holds that no
/// item carries yet, with the group's own tags checked. Also the vocabulary it searches.
pub fn group_tag_vocabulary(vocabulary: &[Tag], group: &TagGroup) -> Vec<Tag> {
    let mut all = vocabulary.to_vec();
    for name in &group.tags {
        if !all.iter().any(|tag| tag.name == *name) {
            all.push(Tag {
                name: name.clone(),
                image_count: None,
            });
        }
    }
    all
}

/// A typed name that is no tag yet; Enter then creates it.
pub fn create_candidate(vocabulary: &[Tag], query: &str) -> Option<String> {
    let name = query.trim();
    (!name.is_empty() && !vocabulary.iter().any(|tag| tag.name == name)).then(|| name.to_owned())
}

/// The row behind a flat index.
pub fn row_at(sections: &[PickerSection], flat: usize) -> Option<&PickerRow> {
    sections.iter().flat_map(|s| s.rows.iter()).nth(flat)
}

fn locate(sections: &[PickerSection], flat: usize) -> Option<(usize, usize)> {
    let mut start = 0;
    for (s, section) in sections.iter().enumerate() {
        if flat < start + section.rows.len() {
            return Some((s, flat - start));
        }
        start += section.rows.len();
    }
    None
}

fn flat_of(sections: &[PickerSection], section: usize, index: usize) -> usize {
    sections[..section]
        .iter()
        .map(|s| s.rows.len())
        .sum::<usize>()
        + index
}

/// Arrow keys over the sections laid out in `columns`: left / right step through the cells,
/// up / down move a row, across sections, keeping the column where the row has one.
pub fn step(
    sections: &[PickerSection],
    columns: usize,
    current: Option<usize>,
    key: &str,
) -> Option<usize> {
    let total: usize = sections.iter().map(|s| s.rows.len()).sum();
    if total == 0 {
        return None;
    }
    let Some(current) = current.filter(|c| *c < total) else {
        return Some(if key == "up" || key == "left" {
            total - 1
        } else {
            0
        });
    };
    let columns = columns.max(1);
    let (s, i) = locate(sections, current)?;
    let len = sections[s].rows.len();
    let column = i % columns;
    Some(match key {
        "left" => current.saturating_sub(1),
        "right" => (current + 1).min(total - 1),
        "down" if i + columns < len => current + columns,
        "down" if (i / columns) < (len - 1) / columns => flat_of(sections, s, len - 1),
        "down" => match sections[s + 1..].iter().position(|n| !n.rows.is_empty()) {
            Some(offset) => {
                let next = s + 1 + offset;
                flat_of(sections, next, column.min(sections[next].rows.len() - 1))
            }
            None => current,
        },
        "up" if i >= columns => current - columns,
        "up" => match sections[..s].iter().rposition(|p| !p.rows.is_empty()) {
            Some(prev) => {
                let plen = sections[prev].rows.len();
                let last_row = (plen - 1) / columns * columns;
                flat_of(sections, prev, (last_row + column).min(plen - 1))
            }
            None => current,
        },
        _ => current,
    })
}

/// Puts `value` first in a short most-recent list.
pub fn remember(recent: &mut Vec<String>, value: &str) {
    recent.retain(|v| v != value);
    recent.insert(0, value.to_owned());
    recent.truncate(8);
}

/// The widgets the right bar draws: the pickers and the in-place fields.
#[derive(Clone)]
pub struct PickerHandles {
    pub tags: Entity<Picker>,
    pub folders: Entity<Picker>,
    pub tags_open: bool,
    pub folders_open: bool,
    /// The in-place name, note and link fields (D-151).
    pub editors: crate::ops_details::DetailEditors,
    /// Folded right-bar sections.
    pub info_collapsed: bool,
    pub exif_collapsed: bool,
    /// Any colour for the open folder (D-159).
    pub folder_color: Entity<gpui_kit::component::color_picker::ColorPickerState>,
}

impl RootApp {
    pub(crate) fn picker_handles(&self) -> PickerHandles {
        PickerHandles {
            tags: self.tag_pick.clone(),
            folders: self.folder_pick.clone(),
            tags_open: self.tag_pick_open,
            folders_open: self.folder_pick_open,
            editors: self.detail_editors.clone(),
            info_collapsed: self.info_collapsed,
            exif_collapsed: self.exif_collapsed,
            folder_color: self.folder_color_picker.clone(),
        }
    }

    /// Tags every selected item has: the picker shows them checked.
    pub(crate) fn selection_tag_names(&self) -> HashSet<String> {
        if let Some(item) = self.details.item.as_ref() {
            return item.tags.iter().cloned().collect();
        }
        let Some(selection) = self.details.selection.as_ref() else {
            return HashSet::new();
        };
        selection
            .tags
            .iter()
            .filter(|tag| tag.image_count == Some(selection.items))
            .map(|tag| tag.name.clone())
            .collect()
    }

    /// Opens or closes a picker; opening starts with an empty search, the caret in it.
    pub(crate) fn set_picker_open(
        &mut self,
        kind: PickerKind,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = match kind {
            PickerKind::Tags => {
                self.tag_pick_open = open;
                self.tag_pick.clone()
            }
            PickerKind::GroupTags => {
                if !open {
                    self.group_tag_pick_for = None;
                }
                self.group_tag_pick.clone()
            }
            PickerKind::Folders => {
                self.folder_pick_open = open;
                self.folder_pick.clone()
            }
        };
        if open {
            picker.update(cx, |picker, cx| picker.reset(window, cx));
        }
        cx.notify();
    }
}

/// The popover's content: one per kind, kept for the app's life.
pub struct Picker {
    kind: PickerKind,
    root: WeakEntity<RootApp>,
    input: Entity<InputState>,
    highlighted: Option<usize>,
    /// The group (id, name) whose + was pressed: Enter puts the typed name into it (D-187).
    adding_to: Option<(String, String)>,
    _sub: Subscription,
}

impl Picker {
    pub fn new(
        kind: PickerKind,
        root: WeakEntity<RootApp>,
        placeholder: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let _sub = cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                // A new search highlights its first match, unless Enter should create the
                // typed tag.
                let (sections, candidate) = this.sections(cx);
                this.highlighted =
                    (candidate.is_none() && row_at(&sections, 0).is_some()).then_some(0);
                cx.notify();
            }
        });
        Self {
            kind,
            root,
            input,
            highlighted: None,
            adding_to: None,
            _sub,
        }
    }

    /// Opening: an empty search with the caret in it.
    pub fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.adding_to = None;
        self.highlighted = None;
        self.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.highlighted = None;
        cx.notify();
    }

    fn query(&self, cx: &gpui_kit::App) -> String {
        self.input.read(cx).value().to_string()
    }

    /// The sections for the search now, and the tag Enter would create.
    fn sections(&self, cx: &gpui_kit::App) -> (Vec<PickerSection>, Option<String>) {
        let Some(root) = self.root.upgrade() else {
            return (Vec::new(), None);
        };
        let app = root.read(cx);
        let query = self.query(cx);
        match self.kind {
            PickerKind::Tags => {
                let on = app.selection_tag_names();
                (
                    tag_sections(
                        &app.tag_vocabulary,
                        &app.tag_groups,
                        &app.recent_tags,
                        &on,
                        &query,
                    ),
                    create_candidate(&app.tag_vocabulary, &query),
                )
            }
            PickerKind::GroupTags => {
                let Some(group) = app
                    .group_tag_pick_for
                    .as_ref()
                    .and_then(|id| app.tag_groups.iter().find(|g| g.id == *id))
                else {
                    return (Vec::new(), None);
                };
                let vocabulary = group_tag_vocabulary(&app.tag_vocabulary, group);
                let on: HashSet<String> = group.tags.iter().cloned().collect();
                (
                    tag_sections(&vocabulary, &app.tag_groups, &app.recent_tags, &on, &query),
                    create_candidate(&vocabulary, &query),
                )
            }
            PickerKind::Folders => {
                let on: HashSet<String> = app
                    .details
                    .item
                    .as_ref()
                    .map(|item| item.folders.iter().cloned().collect())
                    .unwrap_or_default();
                let folders = app
                    .library
                    .as_ref()
                    .map(|lib| lib.folders.clone())
                    .unwrap_or_default();
                (
                    folder_sections(&folders, &app.recent_folders, &on, &query),
                    None,
                )
            }
        }
    }

    /// Checks or unchecks a value on the selection; the popover stays open.
    fn toggle(&mut self, row: &PickerRow, cx: &mut Context<Self>) {
        let (kind, key, on) = (self.kind, row.key.clone(), row.on);
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |app, cx| match (kind, on) {
                (PickerKind::Tags, true) => app.remove_tag_from_selection(key, cx),
                (PickerKind::Tags, false) => {
                    remember(&mut app.recent_tags, &key);
                    app.add_tag_to_selection(key, cx);
                }
                (PickerKind::Folders, true) => {
                    app.apply_edit_to_selection(ItemEdit::RemoveFromFolders(vec![key]), cx)
                }
                (PickerKind::Folders, false) => {
                    remember(&mut app.recent_folders, &key);
                    app.apply_edit_to_selection(ItemEdit::AddToFolders(vec![key]), cx);
                }
                (PickerKind::GroupTags, on) => app.set_tag_in_picked_group(key, !on, cx),
            });
        }
        cx.notify();
    }

    fn create(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let kind = self.kind;
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |app, cx| {
                remember(&mut app.recent_tags, &name);
                if kind == PickerKind::GroupTags {
                    app.set_tag_in_picked_group(name, true, cx);
                    return;
                }
                app.add_tag_to_selection(name, cx);
                // A new tag joins the vocabulary the picker lists.
                app.tag_vocabulary = app.session.tag_vocabulary().unwrap_or_default();
            });
        }
        self.reset(window, cx);
    }

    /// The + of a group's header: the field now takes the name of a tag for that group.
    fn start_adding(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.highlighted = None;
        self.adding_to = Some((id, name));
        cx.notify();
    }

    /// Puts `name` into the group whose + was pressed. From the right bar the selection gets
    /// the tag as well, which also brings a new name into the vocabulary (D-187).
    fn add_to_group(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some((group, _)) = self.adding_to.clone() else {
            return;
        };
        let kind = self.kind;
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |app, cx| {
                remember(&mut app.recent_tags, &name);
                let tag = name.clone();
                app.update_tag_group(
                    &group,
                    move |g| {
                        if !g.tags.contains(&tag) {
                            g.tags.push(tag);
                        }
                    },
                    cx,
                );
                if kind == PickerKind::Tags {
                    app.add_tag_to_selection(name, cx);
                    app.tag_vocabulary = app.session.tag_vocabulary().unwrap_or_default();
                }
            });
        }
        self.reset(window, cx);
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        let kind = self.kind;
        if let Some(root) = self.root.upgrade() {
            root.update(cx, |app, cx| {
                match kind {
                    PickerKind::Tags => app.tag_pick_open = false,
                    PickerKind::Folders => app.folder_pick_open = false,
                    PickerKind::GroupTags => app.group_tag_pick_for = None,
                }
                cx.notify();
            });
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if self.adding_to.is_some() {
            match key {
                "enter" => {
                    let name = self.query(cx).trim().to_owned();
                    if name.is_empty() {
                        return;
                    }
                    self.add_to_group(name, window, cx);
                }
                // Escape leaves the group's +, not the picker.
                "escape" => self.reset(window, cx),
                _ => return,
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let (sections, candidate) = self.sections(cx);
        match key {
            "up" | "down" | "left" | "right" => {
                // Left / right stay with the caret while there is text to move in.
                if matches!(key, "left" | "right") && !self.query(cx).is_empty() {
                    return;
                }
                self.highlighted = step(&sections, self.kind.columns(), self.highlighted, key);
            }
            "enter" => match self.highlighted.and_then(|i| row_at(&sections, i)).cloned() {
                Some(row) => self.toggle(&row, cx),
                None => match candidate {
                    Some(name) => self.create(name, window, cx),
                    None => return,
                },
            },
            "escape" => self.close(cx),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for Picker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let locale = self
            .root
            .upgrade()
            .map(|root| root.read(cx).locale)
            .unwrap_or_default();
        let (sections, candidate) = self.sections(cx);
        let columns = self.kind.columns();
        let highlighted = self.highlighted;
        let kind = self.kind;
        let mut flat = 0usize;

        // After a group's +, the field names a tag for that group: one row says so (D-187).
        let adding = self.adding_to.clone().map(|(_, group)| {
            let typed = self.query(cx).trim().to_owned();
            let text = if typed.is_empty() {
                t(locale, "pick.addToGroupHint").replace("{group}", &group)
            } else {
                t(locale, "pick.addToGroup")
                    .replace("{name}", &typed)
                    .replace("{group}", &group)
            };
            h_flex()
                .id("picker-add-to-group")
                .mx_2()
                .px_2()
                .py_1()
                .gap_2()
                .rounded(theme.radius)
                .bg(theme.list_hover)
                .when(!typed.is_empty(), |el| el.cursor_pointer())
                .child(Icon::new(IconName::Plus).small())
                .child(Label::new(text).text_sm())
                .on_click(cx.listener(move |this, _, window, cx| {
                    if !typed.is_empty() {
                        this.add_to_group(typed.clone(), window, cx);
                    }
                }))
        });
        let adding_now = adding.is_some();
        let (sections, candidate) = if adding_now {
            (Vec::new(), None)
        } else {
            (sections, candidate)
        };
        let body = v_flex()
            .id("picker-body")
            .max_h(px(320.))
            .overflow_y_scrollbar()
            .py_1()
            .when_some(adding, |el, row| el.child(row))
            .when_some(
                candidate.clone().filter(|_| highlighted.is_none()),
                |el, name| {
                    el.child(
                        h_flex()
                            .id("picker-create")
                            .mx_2()
                            .px_2()
                            .py_1()
                            .gap_2()
                            .rounded(theme.radius)
                            .bg(theme.list_hover)
                            .cursor_pointer()
                            .child(Icon::new(IconName::Plus).small())
                            .child(
                                Label::new(t(locale, "pick.createTag").replace("{name}", &name))
                                    .text_sm(),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.create(name.clone(), window, cx)
                            })),
                    )
                },
            )
            .when(
                sections.is_empty() && candidate.is_none() && !adding_now,
                |el| {
                    el.child(
                        div().px_3().py_2().child(
                            Label::new(t(locale, "pick.nothing"))
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        ),
                    )
                },
            )
            .children(sections.iter().enumerate().map(|(s, section)| {
                let title = section.title.as_ref().map(|title| {
                    let name = match title {
                        SectionTitle::Recent => t(locale, "pick.recent"),
                        SectionTitle::Group { name, .. } => name.clone(),
                        SectionTitle::Other => t(locale, "pick.other"),
                    };
                    format!("{name} ({})", section.rows.len())
                });
                // A group's + takes the name of a new tag for it (D-187).
                let group_add = match &section.title {
                    Some(SectionTitle::Group { id, name })
                        if matches!(kind, PickerKind::Tags | PickerKind::GroupTags) =>
                    {
                        Some((id.clone(), name.clone()))
                    }
                    _ => None,
                };
                let cells: Vec<_> = section
                    .rows
                    .iter()
                    .map(|row| {
                        let index = flat;
                        flat += 1;
                        let row = row.clone();
                        let lit = highlighted == Some(index);
                        let mut cell = h_flex()
                            .id(SharedString::from(format!("picker-{s}-{index}")))
                            .gap_1p5()
                            .px_2()
                            .py(px(3.))
                            .rounded(theme.radius)
                            .border_1()
                            .border_color(if lit {
                                theme.border
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .when(lit, |el| el.bg(theme.list_hover))
                            .when(!lit, |el| {
                                let hover = theme.list_hover;
                                el.hover(move |el| el.bg(hover))
                            })
                            .cursor_pointer()
                            .overflow_hidden();
                        cell = match kind {
                            PickerKind::Tags | PickerKind::GroupTags => {
                                cell.flex_1().min_w_0().child(if row.on {
                                    Icon::new(IconName::Check)
                                        .xsmall()
                                        .text_color(theme.primary)
                                        .into_any_element()
                                } else {
                                    Label::new("•")
                                        .text_color(theme.muted_foreground)
                                        .into_any_element()
                                })
                            }
                            PickerKind::Folders => cell
                                .w_full()
                                .pl(px(8. + row.depth as f32 * 16.))
                                .child(
                                    Icon::new(if row.on {
                                        IconName::SquareCheck
                                    } else {
                                        IconName::Square
                                    })
                                    .small()
                                    .text_color(if row.on {
                                        theme.primary
                                    } else {
                                        theme.muted_foreground
                                    }),
                                )
                                .child(
                                    Icon::new(IconName::Folder)
                                        .small()
                                        .text_color(theme.muted_foreground),
                                ),
                        };
                        cell.child(
                            Label::new(row.label.clone())
                                .text_sm()
                                .whitespace_nowrap()
                                .text_ellipsis(),
                        )
                        .when_some(row.count, |el, n| {
                            el.child(
                                Label::new(format!("({n})"))
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            )
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.highlighted = Some(index);
                            this.toggle(&row, cx);
                        }))
                    })
                    .collect();
                let mut grid = v_flex().gap_0p5().px_2();
                let mut cells = cells.into_iter().peekable();
                while cells.peek().is_some() {
                    let mut line = h_flex().w_full().gap_1();
                    for _ in 0..columns {
                        match cells.next() {
                            Some(cell) => line = line.child(cell),
                            None if columns > 1 => line = line.child(div().flex_1()),
                            None => {}
                        }
                    }
                    grid = grid.child(line);
                }
                v_flex()
                    .gap_1()
                    .py_1()
                    .when(s > 0 && section.title.is_some(), |el| {
                        el.border_t_1().border_color(theme.border).pt_2()
                    })
                    .when_some(title, |el, title| {
                        el.child(
                            h_flex()
                                .px_3()
                                .gap_1()
                                .items_center()
                                .child(
                                    Label::new(title)
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                        .text_color(theme.muted_foreground),
                                )
                                .when_some(group_add.clone(), |el, (id, name)| {
                                    let tip =
                                        t(locale, "pick.newTagInGroup").replace("{group}", &name);
                                    el.child(
                                        Button::new(SharedString::from(format!(
                                            "picker-group-add-{id}"
                                        )))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Plus)
                                        .tooltip(tip)
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.start_adding(
                                                    id.clone(),
                                                    name.clone(),
                                                    window,
                                                    cx,
                                                )
                                            }),
                                        ),
                                    )
                                }),
                        )
                    })
                    .child(grid)
            }));

        let hint = |label: String, keys: &'static str| {
            h_flex()
                .gap_1()
                .child(
                    Label::new(label)
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .px_1()
                        .rounded(px(3.))
                        .bg(theme.muted)
                        .child(Label::new(keys).text_xs()),
                )
        };

        v_flex()
            .id(match kind {
                PickerKind::Tags => "tag-picker",
                PickerKind::GroupTags => "group-tag-picker",
                PickerKind::Folders => "folder-picker",
            })
            .w(px(match kind {
                PickerKind::Tags | PickerKind::GroupTags => 320.,
                PickerKind::Folders => 300.,
            }))
            .capture_key_down(cx.listener(Self::on_key))
            .child(
                h_flex()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        Icon::new(IconName::Search)
                            .small()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.input).appearance(false).small()),
                    ),
            )
            .child(body)
            .child(
                h_flex()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(hint(t(locale, "pick.move"), "↑ ↓ ← →"))
                    .child(hint(t(locale, "pick.select"), "⏎"))
                    .child(hint(t(locale, "pick.close"), "Esc")),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str, count: u32) -> Tag {
        Tag {
            name: name.into(),
            image_count: Some(count),
        }
    }

    /// D-187: a group's picker lists its own names even when no item carries them, checks
    /// them, and offers to create only a name that is neither a tag nor in the group.
    #[test]
    fn group_picker_lists_and_checks_the_group_tags() {
        let group = TagGroup {
            id: "G".into(),
            name: "People".into(),
            color: None,
            tags: vec!["Баба Женя".into(), "Unused".into()],
        };
        let all = group_tag_vocabulary(&vocabulary(), &group);
        assert!(
            all.iter()
                .any(|t| t.name == "Unused" && t.image_count.is_none())
        );
        assert_eq!(
            all.iter().filter(|t| t.name == "Баба Женя").count(),
            1,
            "a used tag is not listed twice"
        );
        let on: HashSet<String> = group.tags.iter().cloned().collect();
        let sections = tag_sections(&all, std::slice::from_ref(&group), &[], &on, "");
        let checked: Vec<&str> = sections
            .iter()
            .flat_map(|s| &s.rows)
            .filter(|r| r.on)
            .map(|r| r.key.as_str())
            .collect();
        assert_eq!(checked, ["Баба Женя", "Unused"]);
        assert_eq!(create_candidate(&all, "Unused"), None);
        assert_eq!(create_candidate(&all, " New "), Some("New".into()));
        // Tags are case-sensitive: another case is another tag.
        assert_eq!(create_candidate(&all, "unused"), Some("unused".into()));
    }

    fn vocabulary() -> Vec<Tag> {
        vec![
            tag("Баба Женя", 23),
            tag("Баба Нина", 0),
            tag("Общие", 1),
            tag("Влад", 2),
            tag("Данил", 3),
            tag("docs", 6),
        ]
    }

    fn groups() -> Vec<TagGroup> {
        vec![
            TagGroup {
                id: "g1".into(),
                name: "Семья".into(),
                color: None,
                tags: vec!["Баба Женя".into(), "Баба Нина".into(), "Общие".into()],
            },
            TagGroup {
                id: "g2".into(),
                name: "Друзья".into(),
                color: None,
                tags: vec!["Влад".into(), "Данил".into()],
            },
        ]
    }

    #[test]
    fn tags_come_as_recent_groups_and_other() {
        let on: HashSet<String> = ["Данил".to_owned()].into();
        let sections = tag_sections(&vocabulary(), &groups(), &["docs".into()], &on, "");
        let titles: Vec<_> = sections.iter().map(|s| s.title.clone()).collect();
        assert_eq!(
            titles,
            vec![
                Some(SectionTitle::Recent),
                Some(SectionTitle::Group {
                    id: "g1".into(),
                    name: "Семья".into()
                }),
                Some(SectionTitle::Group {
                    id: "g2".into(),
                    name: "Друзья".into()
                }),
                Some(SectionTitle::Other),
            ]
        );
        assert_eq!(sections[1].rows.len(), 3);
        assert_eq!(sections[1].rows[0].count, Some(23));
        assert_eq!(
            sections[1].rows[1].count, None,
            "an empty count is not shown"
        );
        assert!(sections[2].rows[1].on);
        assert_eq!(sections[3].rows[0].label, "docs");
    }

    #[test]
    fn a_search_lists_matches_prefix_first_and_offers_a_new_tag() {
        let sections = tag_sections(&vocabulary(), &groups(), &[], &HashSet::new(), "на");
        let labels: Vec<_> = sections[0].rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, vec!["Баба Нина"]);
        let sections = tag_sections(&vocabulary(), &groups(), &[], &HashSet::new(), "ба");
        assert_eq!(sections[0].rows.len(), 2);
        assert_eq!(create_candidate(&vocabulary(), "Влад"), None);
        assert_eq!(
            create_candidate(&vocabulary(), " влад "),
            Some("влад".into()),
            "tags are case-sensitive"
        );
        assert_eq!(create_candidate(&vocabulary(), "  "), None);
    }

    #[test]
    fn arrows_walk_two_columns_across_sections() {
        let sections = tag_sections(&vocabulary(), &groups(), &[], &HashSet::new(), "");
        // Семья: 0 1 / 2 ; Друзья: 3 4 ; Другие: 5
        assert_eq!(step(&sections, 2, None, "down"), Some(0));
        assert_eq!(step(&sections, 2, Some(0), "right"), Some(1));
        assert_eq!(
            step(&sections, 2, Some(1), "down"),
            Some(2),
            "to the short last row"
        );
        assert_eq!(
            step(&sections, 2, Some(2), "down"),
            Some(3),
            "into the next section"
        );
        assert_eq!(step(&sections, 2, Some(4), "down"), Some(5));
        assert_eq!(
            step(&sections, 2, Some(5), "down"),
            Some(5),
            "stays at the end"
        );
        assert_eq!(
            step(&sections, 2, Some(4), "up"),
            Some(2),
            "same column, clamped"
        );
        assert_eq!(step(&sections, 2, Some(3), "up"), Some(2));
        assert_eq!(step(&sections, 2, Some(0), "up"), Some(0));
        assert_eq!(step(&[], 2, None, "down"), None);
    }

    #[test]
    fn folders_list_the_tree_or_matching_paths() {
        let child = Folder {
            id: "c".into(),
            name: "screenshots".into(),
            ..Default::default()
        };
        let locked = Folder {
            id: "l".into(),
            name: "Secret".into(),
            is_locked: true,
            ..Default::default()
        };
        let parent = Folder {
            id: "p".into(),
            name: "Olympus".into(),
            children: vec![child],
            ..Default::default()
        };
        let folders = vec![parent, locked];
        let on: HashSet<String> = ["p".to_owned()].into();
        let tree = folder_sections(&folders, &[], &on, "");
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].rows.len(), 2, "locked folders are left out");
        assert!(tree[0].rows[0].on);
        assert_eq!(tree[0].rows[1].depth, 1);
        let found = folder_sections(&folders, &[], &on, "SCREEN");
        assert_eq!(found[0].rows[0].label, "Olympus / screenshots");
        let recent = folder_sections(&folders, &["c".into()], &on, "");
        assert_eq!(recent[0].title, Some(SectionTitle::Recent));
    }

    #[test]
    fn recent_values_move_to_the_front() {
        let mut recent = vec!["a".to_owned(), "b".to_owned()];
        remember(&mut recent, "b");
        assert_eq!(recent, vec!["b", "a"]);
        for n in 0..10 {
            remember(&mut recent, &n.to_string());
        }
        assert_eq!(recent.len(), 8);
    }
}
