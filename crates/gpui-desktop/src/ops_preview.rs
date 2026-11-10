//! The full-size preview: open, step, zoom, pan, rotate (spec §7.3, D-143).
//!
//! Opening never reads or decodes the original on the UI thread. The preview shows the grid
//! thumbnail at once and swaps in the full image when the background decode lands; the
//! neighbours decode at the same time, so the next step is usually instant.

use gpui_kit::{App, Context};
use library_core::ItemCell;

use crate::app::RootApp;
use crate::browse::preview::PreviewState;
use crate::browse::preview_cache::{PreviewKey, Slot};
use crate::browse::preview_image::{self, Shown};

fn key_of(cell: &ItemCell) -> PreviewKey {
    PreviewKey {
        id: cell.id.0.clone(),
        version: cell.thumb_version,
    }
}

impl RootApp {
    pub(crate) fn open_preview(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let index = lib.selection.focus.or_else(|| {
            lib.selection
                .ids
                .iter()
                .next()
                .and_then(|id| lib.cells.iter().position(|c| c.id.0 == *id))
        });
        let Some(index) = index else {
            return;
        };
        self.open_preview_at(index, cx);
    }

    pub(crate) fn open_preview_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        let Some(cell) = lib.cells.get(index).cloned() else {
            return;
        };
        let total = lib.total.max(lib.cells.len() as u32);
        lib.selection.select_only(&cell.id.0, index);
        lib.persist_session(&self.session);
        // The right bar loads the thumbnail; the preview borrows it as its first frame.
        self.refresh_details(cx);

        let key = key_of(&cell);
        let (ready, loading) = match self.preview_cache.get(&key) {
            Some(Slot::Ready { shown, .. }) => (shown.clone(), false),
            Some(Slot::Loading) => (None, true),
            None => (None, true),
        };
        let placeholder = self.details.thumb.clone().map(Shown::Encoded);
        let was_open = self.preview.as_ref().map(|p| p.appear);
        let mut state = PreviewState::new(
            index,
            total,
            format!("{}.{}", cell.name, cell.ext),
            ready.or(placeholder),
        );
        state.key = Some(key);
        state.loading = loading;
        state.natural = cell
            .width
            .zip(cell.height)
            .map(|(w, h)| (w as f32, h as f32));
        // Stepping inside an open preview does not fade in again.
        if let Some(appear) = was_open {
            state.appear = appear;
        }
        self.drop_rotated(cx);
        self.preview = Some(state);
        self.load_preview_window(index, cx);
        cx.notify();
    }

    /// Decodes the open item and its neighbours; forgets everything else.
    fn load_preview_window(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        // The open item first: it is what the user waits for.
        let cells: Vec<ItemCell> = [Some(index), index.checked_add(1), index.checked_sub(1)]
            .into_iter()
            .flatten()
            .filter_map(|i| lib.cells.get(i).cloned())
            .collect();
        let keep: Vec<PreviewKey> = cells.iter().map(key_of).collect();
        for image in self.preview_cache.retain(&keep) {
            cx.drop_image(image, None);
        }
        for cell in cells {
            let key = key_of(&cell);
            if !self.preview_cache.begin(&key) {
                continue;
            }
            let session = self.session.clone();
            cx.spawn(async move |this, cx| {
                let (payload, shown) = cx
                    .background_executor()
                    .spawn(async move {
                        let payload = preview_image::preview_payload(&session, &cell);
                        let shown = payload
                            .as_ref()
                            .and_then(|p| preview_image::decode_payload(p, 0));
                        (payload, shown)
                    })
                    .await;
                this.update(cx, |this, cx| this.preview_loaded(key, payload, shown, cx))
                    .ok();
            })
            .detach();
        }
    }

    fn preview_loaded(
        &mut self,
        key: PreviewKey,
        payload: Option<preview_image::PreviewPayload>,
        shown: Option<Shown>,
        cx: &mut Context<Self>,
    ) {
        let is_open = self
            .preview
            .as_ref()
            .is_some_and(|p| p.key.as_ref() == Some(&key));
        if let Some(evicted) = self.preview_cache.finish(&key, payload, shown.clone()) {
            // The user moved on while this decoded.
            if let Some(image) = evicted.pixels().cloned() {
                cx.drop_image(image, None);
            }
            return;
        }
        if !is_open {
            return;
        }
        let rotation = self.preview.as_ref().map_or(0, |p| p.rotation);
        if let Some(preview) = self.preview.as_mut() {
            preview.loading = false;
            // A failed decode keeps the thumbnail rather than going blank.
            if rotation == 0 && shown.is_some() {
                preview.image = shown;
            }
        }
        if rotation != 0 {
            self.load_rotated(cx);
        }
        cx.notify();
    }

    /// Frees the rotated copy of the open item, if any.
    fn drop_rotated(&mut self, cx: &mut App) {
        if let Some(image) = self
            .preview_rotated
            .take()
            .and_then(|s| s.pixels().cloned())
        {
            cx.drop_image(image, None);
        }
    }

    /// Closes the preview and frees every decoded image it holds.
    pub(crate) fn release_preview(&mut self, cx: &mut App) {
        self.preview = None;
        self.drop_rotated(cx);
        for image in self.preview_cache.clear() {
            cx.drop_image(image, None);
        }
    }

    pub(crate) fn preview_is_open(&self) -> bool {
        self.preview.is_some()
    }

    pub(crate) fn close_preview(&mut self, cx: &mut Context<Self>) {
        self.release_preview(cx);
        // The grid shows the item the preview ended on; stepping selected it (D-145).
        self.reveal_focus();
        cx.notify();
    }

    pub(crate) fn preview_step(&mut self, step: i32, cx: &mut Context<Self>) {
        let Some(preview) = self.preview.as_ref() else {
            return;
        };
        let total = self.library.as_ref().map_or(0, |lib| lib.cells.len());
        let Some(next) = preview.index.checked_add_signed(step as isize) else {
            return;
        };
        if next >= total || next == preview.index {
            return;
        }
        self.open_preview_at(next, cx);
    }

    /// First (`Home`) or last (`End`) item of the view.
    pub(crate) fn preview_jump(&mut self, last: bool, cx: &mut Context<Self>) {
        let total = self.library.as_ref().map_or(0, |lib| lib.cells.len());
        if total == 0 {
            return;
        }
        let target = if last { total - 1 } else { 0 };
        if self.preview.as_ref().is_some_and(|p| p.index != target) {
            self.open_preview_at(target, cx);
        }
    }

    pub(crate) fn preview_zoom(&mut self, factor: f32, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut() {
            preview.zoom_by(factor);
            cx.notify();
        }
    }

    pub(crate) fn preview_zoom_reset(&mut self, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut() {
            preview.reset_view();
            cx.notify();
        }
    }

    pub(crate) fn preview_rotate(&mut self, cx: &mut Context<Self>) {
        let Some(preview) = self.preview.as_mut() else {
            return;
        };
        preview.rotate_cw();
        if preview.rotation == 0 {
            // Back upright: the cached image is the right one.
            let ready = preview
                .key
                .as_ref()
                .and_then(|key| match self.preview_cache.get(key) {
                    Some(Slot::Ready { shown, .. }) => shown.clone(),
                    _ => None,
                });
            if let Some(ready) = ready {
                preview.image = Some(ready);
            }
            self.drop_rotated(cx);
        } else {
            self.load_rotated(cx);
        }
        cx.notify();
    }

    /// Decodes the open item at its rotation in the background; the current frame stays up
    /// until the turned one is ready.
    fn load_rotated(&mut self, cx: &mut Context<Self>) {
        let Some(preview) = self.preview.as_ref() else {
            return;
        };
        let Some(key) = preview.key.clone() else {
            return;
        };
        let Some(Slot::Ready {
            payload: Some(payload),
            ..
        }) = self.preview_cache.get(&key)
        else {
            // Still loading: `preview_loaded` calls back here once it lands.
            return;
        };
        let payload = payload.clone();
        let degrees = preview.rotation;
        cx.spawn(async move |this, cx| {
            let shown = cx
                .background_executor()
                .spawn(async move { preview_image::decode_payload(&payload, degrees) })
                .await;
            this.update(cx, |this, cx| {
                let current = this
                    .preview
                    .as_ref()
                    .is_some_and(|p| p.key.as_ref() == Some(&key) && p.rotation == degrees);
                if !current {
                    if let Some(image) = shown.as_ref().and_then(|s| s.pixels().cloned()) {
                        cx.drop_image(image, None);
                    }
                    return;
                }
                this.drop_rotated(cx);
                if let Some(preview) = this.preview.as_mut()
                    && shown.is_some()
                {
                    preview.image = shown.clone();
                }
                this.preview_rotated = shown;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn preview_pan_start(
        &mut self,
        origin_x: f32,
        origin_y: f32,
        cx: &mut Context<Self>,
    ) {
        // Any zoom: a fitted or zoomed-out image can be moved around the stage too.
        if let Some(preview) = self.preview.as_mut() {
            preview.drag_origin = Some((origin_x, origin_y));
            cx.notify();
        }
    }

    pub(crate) fn preview_pan_move(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let Some(preview) = self.preview.as_mut() else {
            return;
        };
        let Some((ox, oy)) = preview.drag_origin else {
            return;
        };
        preview.offset = (x - ox, y - oy);
        cx.notify();
    }

    pub(crate) fn preview_pan_end(&mut self, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut() {
            preview.drag_origin = None;
            cx.notify();
        }
    }

    /// The button is down on the image: the closed hand shows (D-148).
    pub(crate) fn preview_is_dragging(&self) -> bool {
        self.preview
            .as_ref()
            .is_some_and(|preview| preview.drag_origin.is_some())
    }
}
