//! Fetching the pages of the open view as the grid scrolls (D-167).
//!
//! The grid is laid out for the view's whole total. Each frame it reports the positions it
//! draws; the pages of those positions (and half a page around) that are not here yet are
//! fetched in the background: their cells from the view, their thumbnails from disk. A page
//! that lands after the view changed is dropped. Far pages are let go again.

use std::ops::Range;

use gpui_kit::Context;
use library_core::ItemCell;

use crate::app::RootApp;
use crate::browse::grid::{self, ListedItem, ListedPages};
use crate::view_window::{KEPT_PAGES, PAGE};

impl RootApp {
    /// The grid drew `range`: remember it, and fetch the pages around it that are missing.
    pub(crate) fn show_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let generation = self.view_generation;
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        lib.visible = range.clone();
        let around = range.start.saturating_sub(PAGE / 2)..range.end + PAGE / 2;
        let view_id = lib.view_id;
        for page in lib.cells.pages_of(around) {
            if self.items.has_page(page) || !self.loading_pages.insert((generation, page)) {
                continue;
            }
            let span = lib.cells.page_span(page);
            let known = lib.cells.page(page).map(<[ItemCell]>::to_vec);
            let session = self.session.clone();
            cx.spawn(async move |this, cx| {
                let loaded = cx
                    .background_executor()
                    .spawn(async move {
                        let cells = match known {
                            Some(cells) => cells,
                            None => session
                                .view_range(view_id, span.start as u32, span.len() as u32)
                                .ok()?,
                        };
                        let listed = grid::load_listed(&session, &cells);
                        Some((cells, listed))
                    })
                    .await;
                this.update(cx, |this, cx| {
                    this.page_loaded(generation, page, loaded, cx);
                })
                .ok();
            })
            .detach();
        }
    }

    fn page_loaded(
        &mut self,
        generation: u64,
        page: usize,
        loaded: Option<(Vec<ItemCell>, Vec<ListedItem>)>,
        cx: &mut Context<Self>,
    ) {
        self.loading_pages.remove(&(generation, page));
        if generation != self.view_generation {
            return;
        }
        let Some((cells, listed)) = loaded else {
            return;
        };
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        if !lib.cells.has_page(page) {
            lib.cells.insert_page(page, cells);
        }
        self.items.insert_page(page, listed);
        let center = lib.visible.start / PAGE;
        self.items.evict_far_from(center, KEPT_PAGES);
        // Cells are small; keep more of them, for runs selected across pages.
        lib.cells.evict_far_from(center, KEPT_PAGES * 4);
        cx.notify();
    }

    /// The view was opened, refreshed or changed under the same id: forget every page,
    /// and build the thumbnails of the pages already fetched now (the ones on screen), so
    /// the grid does not flash empty tiles.
    pub(crate) fn reset_pages(&mut self) {
        self.view_generation += 1;
        let Some(lib) = self.library.as_ref() else {
            self.items = ListedPages::default();
            return;
        };
        let mut items = ListedPages::new(lib.cells.len());
        let on_screen = lib
            .cells
            .pages_of(0..PAGE)
            .chain(lib.cells.pages_of(lib.visible.clone()));
        for page in on_screen {
            if items.has_page(page) {
                continue;
            }
            if let Some(cells) = lib.cells.page(page) {
                items.insert_page(page, grid::load_listed(&self.session, cells));
            }
        }
        self.items = items;
    }

    /// Makes sure the cells of `range` are here, for what needs their ids now (a shift-click
    /// run, a rubber band, the keyboard, the preview). Cells only: thumbnails follow the grid.
    pub(crate) fn fill_cells(&mut self, range: Range<usize>) {
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        if let Err(error) = lib.fill_cells(&self.session, range) {
            self.status = format!("{error}").into();
        }
    }
}
