//! In-window DnD payloads (D-042 / D-057 / D-073) via GPUI `on_drag` / `on_drop`.

use desktop_session::ItemsTarget;
use gpui_kit::component::label::Label;
use gpui_kit::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement, Pixels, Point, Render,
    SharedString, Styled, Window, div, px,
};
use library_core::ItemId;

/// What is being dragged inside the GPUI window.
#[derive(Clone, Debug)]
pub enum DragPayload {
    Items {
        view_id: u64,
        ids: Vec<ItemId>,
        label: SharedString,
    },
    Folder {
        id: String,
        label: SharedString,
    },
}

impl DragPayload {
    pub fn items(view_id: u64, ids: Vec<ItemId>, label: impl Into<SharedString>) -> Self {
        Self::Items {
            view_id,
            ids,
            label: label.into(),
        }
    }

    pub fn folder(id: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self::Folder {
            id: id.into(),
            label: label.into(),
        }
    }

    pub fn label(&self) -> SharedString {
        match self {
            Self::Items { label, .. } | Self::Folder { label, .. } => label.clone(),
        }
    }

    pub fn as_items_target(&self) -> Option<ItemsTarget> {
        match self {
            Self::Items { view_id, ids, .. } => Some(ItemsTarget {
                view_id: Some(*view_id),
                all: false,
                ids: ids.clone(),
            }),
            Self::Folder { .. } => None,
        }
    }
}

/// Floating label shown while a drag is active.
pub struct DragGhost {
    label: SharedString,
}

impl DragGhost {
    pub fn new(label: SharedString) -> Self {
        Self { label }
    }
}

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(gpui_kit::rgb(0x2563_eb))
            .text_color(gpui_kit::rgb(0xffffff))
            .child(Label::new(self.label.clone()).text_xs())
    }
}

pub fn ghost(
    payload: &DragPayload,
    _: Point<Pixels>,
    _: &mut Window,
    cx: &mut App,
) -> Entity<DragGhost> {
    cx.new(|_| DragGhost::new(payload.label()))
}

/// Where a folder drop lands relative to a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FolderZone {
    Before,
    Inside,
    After,
}

/// Hit-test a folder row by local Y fraction (0..=1).
pub fn folder_zone(part: f32) -> FolderZone {
    if part < 0.25 {
        FolderZone::Before
    } else if part > 0.75 {
        FolderZone::After
    } else {
        FolderZone::Inside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_zone_thirds() {
        assert_eq!(folder_zone(0.0), FolderZone::Before);
        assert_eq!(folder_zone(0.24), FolderZone::Before);
        assert_eq!(folder_zone(0.5), FolderZone::Inside);
        assert_eq!(folder_zone(0.76), FolderZone::After);
        assert_eq!(folder_zone(1.0), FolderZone::After);
    }
}
