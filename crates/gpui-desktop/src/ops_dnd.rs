//! Drop intents for Phase 3.F + folder tree helpers.

use gpui_kit::{Context, SharedString};
use library_core::{Folder, FolderChange, ItemEdit, ItemId, SortKey};

use crate::app::RootApp;
use crate::dnd::{DragPayload, FolderZone};
use crate::i18n::{t, t_n};
use crate::library::Place;

impl RootApp {
    pub(crate) fn handle_drop(
        &mut self,
        payload: &DragPayload,
        target: DropIntent,
        cx: &mut Context<Self>,
    ) {
        if self.library.as_ref().is_some_and(|lib| lib.info.read_only) {
            self.status = SharedString::from(t(self.locale, "errors.readOnly"));
            cx.notify();
            return;
        }
        match (payload, target) {
            (DragPayload::Items { target, .. }, DropIntent::Folder { id }) => {
                let target = target.clone();
                match self
                    .session
                    .apply_batch_edit(target, ItemEdit::AddToFolders(vec![id]))
                {
                    Ok(n) => {
                        self.status = SharedString::from(t_n(self.locale, "edit.changed", n));
                        self.reload_after_write(cx);
                    }
                    Err(error) => {
                        self.status = SharedString::from(format!("{error}"));
                        cx.notify();
                    }
                }
            }
            (DragPayload::Items { target, .. }, DropIntent::Tag { name }) => {
                let target = target.clone();
                match self
                    .session
                    .apply_batch_edit(target, ItemEdit::AddTags(vec![name]))
                {
                    Ok(n) => {
                        self.status = SharedString::from(t_n(self.locale, "edit.changed", n));
                        self.reload_after_write(cx);
                    }
                    Err(error) => {
                        self.status = SharedString::from(format!("{error}"));
                        cx.notify();
                    }
                }
            }
            (DragPayload::Items { target, .. }, DropIntent::Trash) => {
                let target = target.clone();
                match self.session.apply_batch_edit(target, ItemEdit::Trash) {
                    Ok(n) => {
                        self.status = SharedString::from(t_n(self.locale, "edit.changed", n));
                        if let Some(lib) = self.library.as_mut() {
                            lib.selection.clear();
                        }
                        self.reload_after_write(cx);
                    }
                    Err(error) => {
                        self.status = SharedString::from(format!("{error}"));
                        cx.notify();
                    }
                }
            }
            (
                DragPayload::Items { target: items, .. },
                DropIntent::Cell {
                    folder_id,
                    before_id,
                },
            ) => match self
                .session
                .reorder_items(&folder_id, items.ids.clone(), before_id)
            {
                Ok(()) => {
                    self.status = SharedString::from(t(self.locale, "dnd.reordered"));
                    self.reload_after_write(cx);
                }
                Err(error) => {
                    self.status = SharedString::from(format!("{error}"));
                    cx.notify();
                }
            },
            (DragPayload::Folder { id, .. }, DropIntent::Root) => {
                self.move_folder(id, None, 0, cx);
            }
            (DragPayload::Folder { id, .. }, DropIntent::FolderNest { parent, zone }) => {
                self.drop_folder_on_folder(id, &parent, zone, cx);
            }
            _ => {}
        }
    }

    fn drop_folder_on_folder(
        &mut self,
        id: &str,
        target_id: &str,
        zone: FolderZone,
        cx: &mut Context<Self>,
    ) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        if id == target_id {
            return;
        }
        match zone {
            FolderZone::Inside => {
                let index = child_count(&lib.folders, target_id);
                self.move_folder(id, Some(target_id.to_string()), index, cx);
            }
            FolderZone::Before | FolderZone::After => {
                let parent = parent_of(&lib.folders, target_id);
                let mut siblings = children_of(&lib.folders, parent.as_deref());
                siblings.retain(|s| s != id);
                let at = siblings.iter().position(|s| s == target_id).unwrap_or(0);
                let index = if zone == FolderZone::After {
                    at + 1
                } else {
                    at
                };
                self.move_folder(id, parent, index as u32, cx);
            }
        }
    }

    fn move_folder(
        &mut self,
        id: &str,
        parent: Option<String>,
        index: u32,
        cx: &mut Context<Self>,
    ) {
        match self.session.save_folder(FolderChange::Move {
            id: id.to_string(),
            parent,
            index,
        }) {
            Ok(tree) => {
                if let Some(lib) = self.library.as_mut() {
                    lib.folders = tree;
                }
                self.reload_after_write(cx);
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn can_accept_drop(&self, payload: &DragPayload, intent: &DropIntent) -> bool {
        if self.library.as_ref().is_some_and(|lib| lib.info.read_only) {
            return false;
        }
        match (payload, intent) {
            (DragPayload::Items { .. }, DropIntent::Folder { id }) => {
                folder_accepts_items(self.library.as_ref().map(|l| l.folders.as_slice()), id)
            }
            (DragPayload::Items { .. }, DropIntent::Tag { .. } | DropIntent::Trash) => true,
            (DragPayload::Folder { .. }, DropIntent::Tag { .. } | DropIntent::Trash) => false,
            (DragPayload::Items { target, .. }, DropIntent::Cell { before_id, .. }) => {
                self.library.as_ref().is_some_and(|lib| {
                    // A Select All is not reordered: its ids are not listed in the UI (D-167).
                    !target.all
                        && lib.sort == SortKey::Manual
                        && matches!(lib.place, Place::Folder { .. })
                        && before_id
                            .as_ref()
                            .is_none_or(|b| !target.ids.iter().any(|id| id == b))
                })
            }
            (DragPayload::Folder { .. }, DropIntent::Root) => true,
            (DragPayload::Folder { id, .. }, DropIntent::FolderNest { parent, zone }) => {
                if id == parent {
                    return false;
                }
                let Some(lib) = self.library.as_ref() else {
                    return false;
                };
                let owned_parent = match zone {
                    FolderZone::Inside => Some(parent.clone()),
                    FolderZone::Before | FolderZone::After => parent_of(&lib.folders, parent),
                };
                !is_descendant(&lib.folders, id, owned_parent.as_deref().unwrap_or(""))
                    && owned_parent.as_deref() != Some(id)
            }
            _ => false,
        }
    }

    pub(crate) fn drag_payload_for_selection(&self) -> Option<DragPayload> {
        let lib = self.library.as_ref()?;
        if lib.info.read_only || matches!(lib.place, Place::Trash) {
            return None;
        }
        let target = lib.selection.target(lib.view_id)?;
        let count = lib.selection.count();
        // The drag image shows the focused item if it is selected, else a loaded selected one
        // (D-152); a Select All does not list its items (D-167).
        let shown = lib
            .selection
            .focus
            .filter(|i| {
                lib.cells
                    .get(*i)
                    .is_some_and(|c| lib.selection.contains(&c.id))
            })
            .or_else(|| {
                lib.cells
                    .loaded()
                    .find(|(_, c)| lib.selection.contains(&c.id))
                    .map(|(i, _)| i)
            });
        let label = match (count, shown.and_then(|i| lib.cells.get(i))) {
            (1, Some(cell)) => format!("{}.{}", cell.name, cell.ext),
            (1, None) => "1 item".into(),
            _ => format!("{count} items"),
        };
        let thumb = shown
            .and_then(|i| self.items.get(i))
            .and_then(|item| item.thumb.clone());
        Some(DragPayload::selection(target, count, label).with_thumb(thumb))
    }

    pub(crate) fn drag_payload_for_index(&self, index: usize) -> Option<DragPayload> {
        let lib = self.library.as_ref()?;
        if lib.info.read_only || matches!(lib.place, Place::Trash) {
            return None;
        }
        let cell = lib.cells.get(index)?;
        if lib.selection.contains(&cell.id) && lib.selection.count() > 1 {
            return self.drag_payload_for_selection();
        }
        let thumb = self.items.get(index).and_then(|item| item.thumb.clone());
        Some(
            DragPayload::items(
                lib.view_id,
                vec![cell.id.clone()],
                format!("{}.{}", cell.name, cell.ext),
            )
            .with_thumb(thumb),
        )
    }
}

#[derive(Clone, Debug)]
pub enum DropIntent {
    Folder {
        id: String,
    },
    FolderNest {
        parent: String,
        zone: FolderZone,
    },
    Tag {
        name: String,
    },
    Trash,
    Root,
    Cell {
        folder_id: String,
        before_id: Option<ItemId>,
    },
}

fn folder_accepts_items(folders: Option<&[Folder]>, id: &str) -> bool {
    let Some(folders) = folders else {
        return false;
    };
    match find_folder(folders, id) {
        Some(folder) if folder.is_locked && !folder.is_unlocked => false,
        Some(_) => true,
        None => false,
    }
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

fn parent_of(folders: &[Folder], id: &str) -> Option<String> {
    parent_of_inner(folders, id, None)
}

fn parent_of_inner(folders: &[Folder], id: &str, parent: Option<&str>) -> Option<String> {
    for folder in folders {
        if folder.id == id {
            return parent.map(str::to_owned);
        }
        if let Some(found) = parent_of_inner(&folder.children, id, Some(&folder.id)) {
            return Some(found);
        }
    }
    None
}

fn children_of(folders: &[Folder], parent: Option<&str>) -> Vec<String> {
    match parent {
        None => folders.iter().map(|f| f.id.clone()).collect(),
        Some(pid) => find_folder(folders, pid)
            .map(|f| f.children.iter().map(|c| c.id.clone()).collect())
            .unwrap_or_default(),
    }
}

fn child_count(folders: &[Folder], id: &str) -> u32 {
    find_folder(folders, id)
        .map(|f| f.children.len() as u32)
        .unwrap_or(0)
}

fn is_descendant(folders: &[Folder], ancestor: &str, maybe_child: &str) -> bool {
    if maybe_child.is_empty() {
        return false;
    }
    if maybe_child == ancestor {
        return true;
    }
    find_folder(folders, ancestor)
        .map(|f| contains_id(&f.children, maybe_child))
        .unwrap_or(false)
}

fn contains_id(folders: &[Folder], id: &str) -> bool {
    folders
        .iter()
        .any(|f| f.id == id || contains_id(&f.children, id))
}
