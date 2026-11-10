//! In-window DnD payloads (D-042 / D-057 / D-073) via GPUI `on_drag` / `on_drop`.

use std::sync::Arc;

use desktop_session::ItemsTarget;
use gpui_kit::component::label::Label;
use gpui_kit::component::{ActiveTheme as _, v_flex};

use gpui_kit::{
    App, AppContext as _, Context, Entity, Image, IntoElement, ParentElement, Pixels, Point,
    Render, SharedString, Styled, StyledImage as _, Window, div, img, px,
};
use library_core::ItemId;

/// What is being dragged inside the GPUI window.
#[derive(Clone, Debug)]
pub enum DragPayload {
    Items {
        view_id: u64,
        ids: Vec<ItemId>,
        label: SharedString,
        /// The thumbnail of the item under the pointer, for the drag image (D-152).
        thumb: Option<Arc<Image>>,
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
            thumb: None,
        }
    }

    /// The drag image shows this thumbnail.
    pub fn with_thumb(mut self, image: Option<Arc<Image>>) -> Self {
        if let Self::Items { thumb, .. } = &mut self {
            *thumb = image;
        }
        self
    }

    /// How many items are dragged; 0 for a folder.
    pub fn count(&self) -> usize {
        match self {
            Self::Items { ids, .. } => ids.len(),
            Self::Folder { .. } => 0,
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

/// The drag image (D-152): the thumbnail of the item under the pointer, faded, with the number
/// of items when there are several; a folder, or an item without a thumbnail, shows its name.
pub struct DragGhost {
    label: SharedString,
    thumb: Option<Arc<Image>>,
    count: usize,
}

/// Longest side of the drag image's thumbnail.
const GHOST_SIZE: f32 = 96.0;

impl DragGhost {
    pub fn new(payload: &DragPayload) -> Self {
        let thumb = match payload {
            DragPayload::Items { thumb, .. } => thumb.clone(),
            DragPayload::Folder { .. } => None,
        };
        Self {
            label: payload.label(),
            thumb,
            count: payload.count(),
        }
    }
}

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let badge = (self.count > 1).then(|| {
            div()
                .absolute()
                .top(px(-6.))
                .right(px(-6.))
                .min_w(px(20.))
                .h(px(20.))
                .px_1()
                .rounded(px(10.))
                .bg(theme.blue)
                .text_color(gpui_kit::white())
                .flex()
                .items_center()
                .justify_center()
                .child(Label::new(self.count.to_string()).text_xs())
        });
        match self.thumb.clone() {
            Some(image) => v_flex()
                .relative()
                .opacity(0.85)
                .child(
                    img(image)
                        .max_w(px(GHOST_SIZE))
                        .max_h(px(GHOST_SIZE))
                        .object_fit(gpui_kit::ObjectFit::Contain)
                        .rounded(px(4.))
                        .shadow_lg(),
                )
                .children(badge)
                .into_any_element(),
            None => div()
                .relative()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(theme.blue)
                .text_color(gpui_kit::white())
                .child(Label::new(self.label.clone()).text_xs())
                .children(badge)
                .into_any_element(),
        }
    }
}

pub fn ghost(
    payload: &DragPayload,
    _: Point<Pixels>,
    _: &mut Window,
    cx: &mut App,
) -> Entity<DragGhost> {
    cx.new(|_| DragGhost::new(payload))
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
