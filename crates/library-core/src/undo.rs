//! Undo of item edits (docs/architecture.md §7.1): the inverse of an edit, worked out from the
//! items as they were just before it, applied later as new edits.

use std::collections::HashMap;

use serde::Serialize;
use ts_rs::TS;

use crate::edit::ItemEdit;
use crate::model::ItemId;

/// Edits kept for `Ctrl+Z`; the oldest goes first.
pub const UNDO_KEPT: usize = 50;

/// What an item edit can change, read just before the edit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemState {
    pub id: ItemId,
    pub name: String,
    pub star: u8,
    pub annotation: String,
    pub url: String,
    pub tags: Vec<String>,
    pub folders: Vec<String>,
    pub in_trash: bool,
}

/// One edit for a group of items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoStep {
    pub ids: Vec<ItemId>,
    pub edit: ItemEdit,
}

/// Which kind of edit an undo takes back; the menu names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EditKind {
    AddTags,
    RemoveTags,
    AddToFolders,
    RemoveFromFolders,
    SetFolders,
    SetStar,
    SetAnnotation,
    SetUrl,
    Rename,
    Trash,
    Restore,
}

impl ItemEdit {
    pub fn kind(&self) -> EditKind {
        match self {
            Self::AddTags(_) => EditKind::AddTags,
            Self::RemoveTags(_) => EditKind::RemoveTags,
            Self::AddToFolders(_) => EditKind::AddToFolders,
            Self::RemoveFromFolders(_) => EditKind::RemoveFromFolders,
            Self::SetFolders(_) => EditKind::SetFolders,
            Self::SetStar(_) => EditKind::SetStar,
            Self::SetAnnotation(_) => EditKind::SetAnnotation,
            Self::SetUrl(_) => EditKind::SetUrl,
            Self::Rename(_) => EditKind::Rename,
            Self::Trash => EditKind::Trash,
            Self::Restore => EditKind::Restore,
        }
    }
}

/// The edit that `Ctrl+Z` would take back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UndoStatus {
    pub kind: EditKind,
    /// Items the edit changed.
    pub items: u32,
}

/// The edits that bring the items of `before` back after `edit`. Items that need the same edit
/// share one step; items the edit did not change need none.
pub fn inverse(edit: &ItemEdit, before: &[ItemState]) -> Vec<UndoStep> {
    let mut steps: Vec<UndoStep> = Vec::new();
    let mut index: HashMap<ItemEdit, usize> = HashMap::new();
    for item in before {
        let Some(back) = undo_one(edit, item) else {
            continue;
        };
        match index.get(&back) {
            Some(&at) => steps[at].ids.push(item.id.clone()),
            None => {
                index.insert(back.clone(), steps.len());
                steps.push(UndoStep {
                    ids: vec![item.id.clone()],
                    edit: back,
                });
            }
        }
    }
    steps
}

fn undo_one(edit: &ItemEdit, item: &ItemState) -> Option<ItemEdit> {
    match edit {
        ItemEdit::AddTags(tags) => absent(tags, &item.tags).map(ItemEdit::RemoveTags),
        ItemEdit::RemoveTags(tags) => present(tags, &item.tags).map(ItemEdit::AddTags),
        ItemEdit::AddToFolders(folders) => {
            absent(folders, &item.folders).map(ItemEdit::RemoveFromFolders)
        }
        ItemEdit::RemoveFromFolders(folders) => {
            present(folders, &item.folders).map(ItemEdit::AddToFolders)
        }
        ItemEdit::SetFolders(folders) => {
            let before = sorted(&item.folders);
            (sorted(folders) != before).then_some(ItemEdit::SetFolders(before))
        }
        ItemEdit::SetStar(star) => (*star != item.star).then_some(ItemEdit::SetStar(item.star)),
        ItemEdit::SetAnnotation(text) => {
            (*text != item.annotation).then(|| ItemEdit::SetAnnotation(item.annotation.clone()))
        }
        ItemEdit::SetUrl(url) => (*url != item.url).then(|| ItemEdit::SetUrl(item.url.clone())),
        ItemEdit::Rename(name) => (*name != item.name).then(|| ItemEdit::Rename(item.name.clone())),
        ItemEdit::Trash => (!item.in_trash).then_some(ItemEdit::Restore),
        ItemEdit::Restore => item.in_trash.then_some(ItemEdit::Trash),
    }
}

/// The names of `wanted` that `have` lacks, sorted; `None` when there are none.
fn absent(wanted: &[String], have: &[String]) -> Option<Vec<String>> {
    let names: Vec<String> = sorted(wanted)
        .into_iter()
        .filter(|name| !have.contains(name))
        .collect();
    (!names.is_empty()).then_some(names)
}

/// The names of `wanted` that `have` contains, sorted; `None` when there are none.
fn present(wanted: &[String], have: &[String]) -> Option<Vec<String>> {
    let names: Vec<String> = sorted(wanted)
        .into_iter()
        .filter(|name| have.contains(name))
        .collect();
    (!names.is_empty()).then_some(names)
}

fn sorted(names: &[String]) -> Vec<String> {
    let mut names = names.to_vec();
    names.sort();
    names.dedup();
    names
}

/// One edit that can be taken back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoEntry {
    pub kind: EditKind,
    pub steps: Vec<UndoStep>,
}

impl UndoEntry {
    pub fn status(&self) -> UndoStatus {
        let items = self.steps.iter().map(|step| step.ids.len()).sum::<usize>();
        UndoStatus {
            kind: self.kind,
            items: u32::try_from(items).unwrap_or(u32::MAX),
        }
    }
}

/// The last item edits of a session, newest last (docs/spec.md §6).
#[derive(Debug, Clone, Default)]
pub struct UndoStack {
    entries: Vec<UndoEntry>,
}

impl UndoStack {
    /// Remembers how to take `edit` back; an edit that changed nothing is not remembered.
    pub fn push(&mut self, edit: &ItemEdit, before: &[ItemState]) {
        let steps = inverse(edit, before);
        if steps.is_empty() {
            return;
        }
        self.entries.push(UndoEntry {
            kind: edit.kind(),
            steps,
        });
        if self.entries.len() > UNDO_KEPT {
            self.entries.remove(0);
        }
    }

    pub fn pop(&mut self) -> Option<UndoEntry> {
        self.entries.pop()
    }

    pub fn status(&self) -> Option<UndoStatus> {
        self.entries.last().map(UndoEntry::status)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(id: &str) -> ItemState {
        ItemState {
            id: ItemId::from(id),
            name: format!("name {id}"),
            ..ItemState::default()
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    fn ids(list: &[&str]) -> Vec<ItemId> {
        list.iter().map(|id| ItemId::from(*id)).collect()
    }

    #[test]
    fn adding_tags_is_undone_by_removing_only_the_new_ones() {
        let mut a = state("a");
        a.tags = names(&["Poster"]);
        let b = state("b");
        let steps = inverse(&ItemEdit::AddTags(names(&["Poster", "Logo"])), &[a, b]);
        assert_eq!(
            steps,
            vec![
                UndoStep {
                    ids: ids(&["a"]),
                    edit: ItemEdit::RemoveTags(names(&["Logo"])),
                },
                UndoStep {
                    ids: ids(&["b"]),
                    edit: ItemEdit::RemoveTags(names(&["Logo", "Poster"])),
                },
            ]
        );
    }

    #[test]
    fn removing_tags_and_folders_is_undone_by_adding_back_what_was_there() {
        let mut a = state("a");
        a.tags = names(&["poster", "Poster"]);
        a.folders = names(&["F1"]);
        let b = state("b");
        assert_eq!(
            inverse(
                &ItemEdit::RemoveTags(names(&["Poster"])),
                &[a.clone(), b.clone()]
            ),
            vec![UndoStep {
                ids: ids(&["a"]),
                edit: ItemEdit::AddTags(names(&["Poster"])),
            }]
        );
        assert_eq!(
            inverse(&ItemEdit::RemoveFromFolders(names(&["F1", "F2"])), &[a, b]),
            vec![UndoStep {
                ids: ids(&["a"]),
                edit: ItemEdit::AddToFolders(names(&["F1"])),
            }]
        );
    }

    #[test]
    fn set_edits_go_back_to_each_items_own_value() {
        let mut a = state("a");
        a.star = 3;
        a.folders = names(&["F2", "F1"]);
        let mut b = state("b");
        b.star = 3;
        let mut c = state("c");
        c.star = 5;
        let all = [a.clone(), b.clone(), c.clone()];
        assert_eq!(
            inverse(&ItemEdit::SetStar(5), &all),
            vec![UndoStep {
                ids: ids(&["a", "b"]),
                edit: ItemEdit::SetStar(3),
            }]
        );
        assert_eq!(
            inverse(&ItemEdit::SetFolders(names(&["F1", "F2"])), &all),
            vec![UndoStep {
                ids: ids(&["b", "c"]),
                edit: ItemEdit::SetFolders(Vec::new()),
            }]
        );
        assert_eq!(
            inverse(&ItemEdit::Rename("x".into()), &[a]),
            vec![UndoStep {
                ids: ids(&["a"]),
                edit: ItemEdit::Rename("name a".into()),
            }]
        );
        assert!(inverse(&ItemEdit::SetAnnotation(String::new()), &[b]).is_empty());
    }

    #[test]
    fn trash_and_restore_undo_each_other() {
        let mut a = state("a");
        a.in_trash = true;
        let b = state("b");
        assert_eq!(
            inverse(&ItemEdit::Trash, &[a.clone(), b.clone()]),
            vec![UndoStep {
                ids: ids(&["b"]),
                edit: ItemEdit::Restore,
            }]
        );
        assert_eq!(
            inverse(&ItemEdit::Restore, &[a, b]),
            vec![UndoStep {
                ids: ids(&["a"]),
                edit: ItemEdit::Trash,
            }]
        );
    }

    #[test]
    fn the_stack_keeps_the_newest_edits_that_changed_something() {
        let mut stack = UndoStack::default();
        stack.push(&ItemEdit::SetStar(0), &[state("a")]);
        assert_eq!(stack.status(), None);
        for _ in 0..UNDO_KEPT + 5 {
            stack.push(&ItemEdit::SetStar(4), &[state("a"), state("b")]);
        }
        stack.push(&ItemEdit::Trash, &[state("a")]);
        assert_eq!(
            stack.status(),
            Some(UndoStatus {
                kind: EditKind::Trash,
                items: 1
            })
        );
        let mut popped = 0;
        while stack.pop().is_some() {
            popped += 1;
        }
        assert_eq!(popped, UNDO_KEPT);
    }
}
