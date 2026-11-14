//! The positions of an open view, held a page at a time (D-167).
//!
//! A view can hold tens of thousands of items; the UI never loads them all (CLAUDE.md). The
//! grid is laid out for the whole `total` from the view's dimensions, and the cells (and their
//! thumbnails) of the pages on screen are fetched as they scroll into view. Pages far from the
//! screen are dropped again, so memory follows what is shown, not the size of the library.

use std::collections::BTreeMap;
use std::ops::Range;

use library_core::{ItemCell, ItemId};

/// Positions per page: a few screens of thumbnails, one `view_range` call.
pub const PAGE: usize = 200;

/// Pages kept around the screen before the farthest are dropped.
pub const KEPT_PAGES: usize = 12;

/// The loaded pages of a view of `total` positions.
#[derive(Debug, Clone)]
pub struct Paged<T> {
    total: usize,
    pages: BTreeMap<usize, Vec<T>>,
}

/// The cells of the open view.
pub type ViewCells = Paged<ItemCell>;

impl<T> Default for Paged<T> {
    fn default() -> Self {
        Self::new(0)
    }
}

impl<T> Paged<T> {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            pages: BTreeMap::new(),
        }
    }

    /// Every position of the view, loaded or not.
    pub fn len(&self) -> usize {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.total {
            return None;
        }
        self.pages.get(&(index / PAGE))?.get(index % PAGE)
    }

    pub fn has_page(&self, page: usize) -> bool {
        self.pages.contains_key(&page)
    }

    /// The positions of `page`, clipped to the view.
    pub fn page_span(&self, page: usize) -> Range<usize> {
        let start = (page * PAGE).min(self.total);
        start..(start + PAGE).min(self.total)
    }

    /// The pages that hold any of `range`, clipped to the view.
    pub fn pages_of(&self, range: Range<usize>) -> Range<usize> {
        let end = range.end.min(self.total);
        if range.start >= end {
            return 0..0;
        }
        range.start / PAGE..(end - 1) / PAGE + 1
    }

    /// The pages of `range` not loaded yet.
    pub fn missing_pages(&self, range: Range<usize>) -> Vec<usize> {
        self.pages_of(range)
            .filter(|page| !self.has_page(*page))
            .collect()
    }

    /// Stores a page; positions past the end of the view are dropped.
    pub fn insert_page(&mut self, page: usize, mut items: Vec<T>) {
        let span = self.page_span(page);
        if span.is_empty() {
            return;
        }
        items.truncate(span.len());
        self.pages.insert(page, items);
    }

    /// Takes a page out, for building another store from it.
    pub fn page(&self, page: usize) -> Option<&[T]> {
        self.pages.get(&page).map(Vec::as_slice)
    }

    /// The loaded positions in view order.
    pub fn loaded(&self) -> impl Iterator<Item = (usize, &T)> {
        self.pages.iter().flat_map(|(page, items)| {
            items
                .iter()
                .enumerate()
                .map(move |(offset, item)| (page * PAGE + offset, item))
        })
    }

    /// Loads the missing pages of `range` through `fetch(start, len)`; the first error stops.
    pub fn fill<E>(
        &mut self,
        range: Range<usize>,
        mut fetch: impl FnMut(usize, usize) -> Result<Vec<T>, E>,
    ) -> Result<(), E> {
        for page in self.missing_pages(range) {
            let span = self.page_span(page);
            let items = fetch(span.start, span.len())?;
            self.insert_page(page, items);
        }
        Ok(())
    }

    /// Drops the pages farthest from `center` until at most `keep` are left.
    pub fn evict_far_from(&mut self, center: usize, keep: usize) {
        while self.pages.len() > keep {
            let farthest = self
                .pages
                .keys()
                .copied()
                .max_by_key(|page| page.abs_diff(center));
            match farthest {
                Some(page) => {
                    self.pages.remove(&page);
                }
                None => break,
            }
        }
    }
}

impl Paged<ItemCell> {
    /// The position of a loaded cell.
    pub fn position(&self, id: &ItemId) -> Option<usize> {
        self.loaded()
            .find(|(_, cell)| cell.id == *id)
            .map(|(index, _)| index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(start: usize, len: usize) -> Vec<ItemCell> {
        (start..start + len)
            .map(|i| ItemCell {
                id: ItemId(format!("id{i}")),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn a_view_is_as_long_as_its_total_before_anything_loads() {
        let view = ViewCells::new(450);
        assert_eq!(view.len(), 450);
        assert!(view.get(0).is_none());
        assert_eq!(view.pages_of(0..450), 0..3);
        assert_eq!(view.pages_of(199..201), 0..2);
        assert_eq!(view.pages_of(440..900), 2..3);
        assert_eq!(view.pages_of(500..600), 0..0);
        assert_eq!(view.page_span(2), 400..450);
        assert!(ViewCells::new(0).is_empty());
    }

    #[test]
    fn fill_fetches_only_missing_pages_and_positions_follow_pages() {
        let mut view = ViewCells::new(450);
        let mut calls = Vec::new();
        view.fill(250..260, |start, len| {
            calls.push((start, len));
            Ok::<_, ()>(cells(start, len))
        })
        .unwrap();
        view.fill(0..450, |start, len| {
            calls.push((start, len));
            Ok::<_, ()>(cells(start, len))
        })
        .unwrap();
        assert_eq!(calls, [(200, 200), (0, 200), (400, 50)]);
        assert_eq!(view.get(449).unwrap().id.0, "id449");
        assert_eq!(view.get(201).unwrap().id.0, "id201");
        assert!(view.get(450).is_none());
        assert_eq!(view.position(&ItemId("id420".into())), Some(420));
        let order: Vec<usize> = view.loaded().map(|(i, _)| i).take(3).collect();
        assert_eq!(order, [0, 1, 2]);
    }

    #[test]
    fn far_pages_are_dropped_and_a_short_page_is_clipped() {
        let mut view = ViewCells::new(2_000);
        for page in 0..10 {
            view.insert_page(page, cells(page * PAGE, PAGE));
        }
        view.evict_far_from(8, 3);
        assert!(view.has_page(7) && view.has_page(8) && view.has_page(9));
        assert!(!view.has_page(0));
        let mut short = ViewCells::new(210);
        short.insert_page(1, cells(200, 200));
        assert_eq!(short.page(1).unwrap().len(), 10);
        short.insert_page(5, cells(1_000, 5));
        assert!(!short.has_page(5));
    }

    #[test]
    fn a_failed_fetch_stops_and_keeps_what_loaded() {
        let mut view = ViewCells::new(600);
        let result = view.fill(0..600, |start, len| {
            if start == 200 {
                Err("gone")
            } else {
                Ok(cells(start, len))
            }
        });
        assert_eq!(result, Err("gone"));
        assert!(view.has_page(0));
        assert!(!view.has_page(1) && !view.has_page(2));
    }
}
