//! Scrolling of the items view (D-145): one handle for the grid and the list, kept in step
//! with the keyboard focus, the preview and changes of view.

use gpui_kit::{ScrollStrategy, point, px};

use crate::app::RootApp;
use crate::justified;
use crate::library::{ActiveFilters, Place};
use library_core::SortKey;

/// Height of one row of the list view (`browse/grid.rs`).
pub const LIST_ROW_HEIGHT: f32 = 48.0;

/// What a scroll position belongs to: a new place, filter, sort or search starts at the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewKey {
    pub place: Place,
    pub filters: ActiveFilters,
    pub sort: SortKey,
    pub ascending: bool,
}

impl RootApp {
    /// How far the items view is scrolled down, in pixels (≥ 0).
    pub(crate) fn grid_scroll_y(&self) -> f32 {
        (-self.grid_scroll.offset().y.as_f32()).max(0.0)
    }

    /// The row of the virtual list that holds `index`: a grid row, or the item in the list.
    fn list_row_of(&self, index: usize) -> Option<usize> {
        match self.justified.as_ref() {
            Some(layout) => layout.row_of.get(index).copied(),
            None => Some(index),
        }
    }

    /// Scrolls so that item `index` is visible.
    pub(crate) fn reveal_index(&self, index: usize, strategy: ScrollStrategy) {
        if let Some(row) = self.list_row_of(index) {
            self.grid_scroll.scroll_to_item(row, strategy);
        }
    }

    /// Scrolls just enough to show the focused item (arrow keys, closing the preview).
    pub(crate) fn reveal_focus(&self) {
        if let Some(focus) = self.library.as_ref().and_then(|lib| lib.selection.focus) {
            self.reveal_index(focus, ScrollStrategy::Nearest);
        }
    }

    pub(crate) fn scroll_items_to_top(&self) {
        self.grid_scroll.set_offset(point(px(0.), px(0.)));
    }

    /// The first item at the top of the view, kept there when the grid repacks.
    pub(crate) fn top_visible_index(&self) -> Option<usize> {
        let scroll_y = self.grid_scroll_y();
        match self.justified.as_ref() {
            Some(layout) => justified::first_index_at_offset(layout, scroll_y),
            None => Some((scroll_y / LIST_ROW_HEIGHT).floor() as usize),
        }
    }

    /// Repacks the grid (thumbnail size, names, grid / list, window width) with the item at
    /// the top of the view still at the top, instead of a pixel offset into other rows.
    pub(crate) fn relayout_keeping_anchor(&mut self) {
        let anchor = self.top_visible_index();
        self.recompute_justified();
        match anchor {
            Some(index) if self.grid_scroll_y() > 0.0 => {
                self.reveal_index(index, ScrollStrategy::Top);
            }
            _ => {}
        }
    }

    /// The view's identity now; `None` without a library.
    pub(crate) fn view_key(&self) -> Option<ViewKey> {
        self.library.as_ref().map(|lib| ViewKey {
            place: lib.place.clone(),
            filters: lib.filters.clone(),
            sort: lib.sort,
            ascending: lib.ascending,
        })
    }

    /// After the view reloads: back to the top when it is a different view, otherwise the
    /// scroll position stays (live updates, edits).
    pub(crate) fn sync_scroll_to_view(&mut self) {
        let key = self.view_key();
        if key != self.scrolled_view {
            self.scroll_items_to_top();
            self.scrolled_view = key;
        }
    }
}
