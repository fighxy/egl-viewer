//! The full-size preview: open, step, zoom, pan, rotate (spec §7.3, D-143).
//!
//! Opening never reads or decodes the original on the UI thread. The preview shows the grid
//! thumbnail at once and swaps in the full image when the background decode lands; the
//! neighbours decode at the same time, so the next step is usually instant.
//!
//! Animated GIFs play through [`crate::browse::gif_player::GifPlayer`]: a task wakes when
//! the next frame is due while one plays. A GIF under the pointer in the grid plays too
//! (D-154).

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{App, Context, RenderImage};
use library_core::ItemCell;

use crate::app::RootApp;
use crate::browse::gif_player::GifPlayer;
use crate::browse::preview::PreviewState;
use crate::browse::preview_cache::{PreviewKey, Slot};
use crate::browse::preview_image::{self, Shown};

/// A GIF playing in a grid cell while the pointer is over it.
pub struct HoverGif {
    pub key: PreviewKey,
    /// `None` while its frames decode.
    pub frames: Option<Arc<RenderImage>>,
    /// Plays the frames once they are there, on the same clock as the preview's player.
    pub player: Option<GifPlayer>,
}

/// Whether hovering this cell plays it: a GIF with an original to read.
pub fn plays_on_hover(cell: &ItemCell) -> bool {
    cell.ext.eq_ignore_ascii_case("gif")
}

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
                .listed_ids()?
                .iter()
                .find_map(|id| lib.cells.position(id))
        });
        let Some(index) = index else {
            return;
        };
        self.open_preview_at(index, cx);
    }

    /// The cell at `index` of what the preview steps through: the open view, or the loose
    /// files being viewed (D-180).
    fn preview_cell(&self, index: usize) -> Option<ItemCell> {
        match self.loose.as_ref() {
            Some(loose) => loose.cell(index),
            None => self.library.as_ref()?.cells.get(index).cloned(),
        }
    }

    /// How many positions the preview steps through.
    fn preview_len(&self) -> usize {
        match self.loose.as_ref() {
            Some(loose) => loose.len(),
            None => self.library.as_ref().map_or(0, |lib| lib.cells.len()),
        }
    }

    pub(crate) fn open_preview_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let loose = self.loose.is_some();
        let total = if let Some(files) = self.loose.as_ref() {
            files.len() as u32
        } else {
            // The item and its neighbours, which the preview preloads, may be on pages not
            // here yet: stepping through the preview walks the whole view (D-167).
            self.fill_cells(index.saturating_sub(1)..index + 2);
            let Some(lib) = self.library.as_ref() else {
                return;
            };
            lib.total.max(lib.cells.len() as u32)
        };
        let Some(cell) = self.preview_cell(index) else {
            return;
        };
        if let Some(lib) = self.library.as_mut().filter(|_| !loose) {
            lib.selection.select_only(&cell.id.0, index);
            lib.persist_session(&self.session);
            // The right bar loads the thumbnail; the preview borrows it as its first frame.
            self.refresh_details(cx);
        }

        let key = key_of(&cell);
        let (ready, loading) = match self.preview_cache.get(&key) {
            Some(Slot::Ready { shown, .. }) => (shown.clone(), false),
            Some(Slot::Loading) => (None, true),
            None => (None, true),
        };
        let placeholder = (!loose)
            .then(|| self.details.thumb.clone().map(Shown::Encoded))
            .flatten();
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
        // Stepping inside an open preview does not fade in again. Viewing a file, the
        // preview is the whole window: it is there at once.
        if let Some(appear) = was_open {
            state.appear = appear;
        }
        if loose {
            state.appear = 1.0;
        }
        self.drop_rotated(cx);
        self.preview = Some(state);
        self.load_preview_window(index, cx);
        self.ensure_gif_ticker(cx);
        // Video and audio play from the original, read by path in Rust (D-170).
        self.open_media(&cell, cx);
        cx.notify();
    }

    /// Decodes the open item and its neighbours; forgets everything else.
    pub(crate) fn load_preview_window(&mut self, index: usize, cx: &mut Context<Self>) {
        // The open item first: it is what the user waits for.
        let cells: Vec<ItemCell> = [Some(index), index.checked_add(1), index.checked_sub(1)]
            .into_iter()
            .flatten()
            .filter_map(|i| self.preview_cell(i))
            .collect();
        let loose = self.loose.is_some();
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
                        let payload = if loose {
                            let path = crate::file_viewer::cell_path(&cell);
                            preview_image::file_payload(&path, &cell.ext)
                        } else {
                            preview_image::preview_payload(&session, &cell)
                        };
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
                preview.show(shown, Instant::now());
            }
        }
        if rotation != 0 {
            self.load_rotated(cx);
        }
        self.ensure_gif_ticker(cx);
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

    /// Closes the preview and frees every decoded image it holds, and the hover GIF's.
    pub(crate) fn release_preview(&mut self, cx: &mut App) {
        self.preview = None;
        self.drop_rotated(cx);
        self.close_media(cx);
        // Libraries open and close through here too: the hover GIF goes with them.
        self.stop_hover_gif(cx);
        self.stop_hover_scrub(cx);
        for image in self.preview_cache.clear() {
            cx.drop_image(image, None);
        }
    }

    pub(crate) fn preview_is_open(&self) -> bool {
        self.preview.is_some()
    }

    pub(crate) fn close_preview(&mut self, cx: &mut Context<Self>) {
        self.release_preview(cx);
        if self.loose.is_some() {
            // The window only viewed the file: closing the preview closes it (D-180).
            cx.quit();
            return;
        }
        // The grid shows the item the preview ended on; stepping selected it (D-145).
        self.reveal_focus();
        cx.notify();
    }

    pub(crate) fn preview_step(&mut self, step: i32, cx: &mut Context<Self>) {
        let Some(preview) = self.preview.as_ref() else {
            return;
        };
        let total = self.preview_len();
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
        let total = self.preview_len();
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
                preview.show(Some(ready), Instant::now());
            }
            self.drop_rotated(cx);
            self.ensure_gif_ticker(cx);
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
                    preview.show(shown.clone(), Instant::now());
                }
                this.preview_rotated = shown;
                this.ensure_gif_ticker(cx);
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

    fn gif_player_mut(&mut self) -> Option<&mut GifPlayer> {
        self.preview.as_mut()?.player.as_mut()
    }

    /// The preview's player and the hover GIF's: whatever the ticker moves on.
    fn gif_players(&mut self) -> [Option<&mut GifPlayer>; 2] {
        [
            self.preview.as_mut().and_then(|p| p.player.as_mut()),
            self.hover_gif.as_mut().and_then(|h| h.player.as_mut()),
        ]
    }

    /// `P` or the play button.
    pub(crate) fn preview_gif_toggle(&mut self, cx: &mut Context<Self>) {
        if self.media.is_some() {
            self.media_toggle(cx);
            return;
        }
        if let Some(player) = self.gif_player_mut() {
            player.toggle(Instant::now());
            self.ensure_gif_ticker(cx);
            cx.notify();
        }
    }

    /// `,` / `.` or the frame buttons: one frame back or forward, paused.
    pub(crate) fn preview_gif_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.media.is_some() {
            self.media_step(if delta < 0 { -1 } else { 1 }, cx);
            return;
        }
        if let Some(player) = self.gif_player_mut() {
            player.step(delta);
            cx.notify();
        }
    }

    /// A click or drag on the scrubber.
    pub(crate) fn preview_gif_seek(&mut self, frame: usize, cx: &mut Context<Self>) {
        if let Some(player) = self.gif_player_mut()
            && player.frame() != frame
        {
            player.seek(frame, Instant::now());
            cx.notify();
        }
    }

    pub(crate) fn preview_gif_speed(&mut self, cx: &mut Context<Self>) {
        if let Some(player) = self.gif_player_mut() {
            player.cycle_speed(Instant::now());
            cx.notify();
        }
    }

    /// Starts the task that moves a playing GIF on, unless one runs. It sleeps until the next
    /// frame is due and ends, in the same update that sees it, once nothing plays.
    pub(crate) fn ensure_gif_ticker(&mut self, cx: &mut Context<Self>) {
        let playing = self
            .gif_players()
            .into_iter()
            .flatten()
            .any(|player| player.is_playing());
        if self.gif_ticking || !playing {
            return;
        }
        self.gif_ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                let wait = this
                    .update(cx, |this, cx| {
                        let now = Instant::now();
                        let mut moved = false;
                        let wait = this
                            .gif_players()
                            .into_iter()
                            .flatten()
                            .filter_map(|player| {
                                moved |= player.advance_to(now);
                                player.until_next()
                            })
                            .min();
                        if moved {
                            cx.notify();
                        }
                        if wait.is_none() {
                            this.gif_ticking = false;
                        }
                        wait
                    })
                    .ok()
                    .flatten();
                let Some(wait) = wait else {
                    break;
                };
                // A floor keeps a run of zero-length waits from spinning.
                cx.background_executor()
                    .timer(wait.max(Duration::from_millis(4)))
                    .await;
            }
        })
        .detach();
    }

    /// The pointer entered (`true`) or left a grid cell that plays on hover.
    pub(crate) fn hover_gif(
        &mut self,
        cell: &ItemCell,
        edge: f32,
        entered: bool,
        cx: &mut Context<Self>,
    ) {
        let key = key_of(cell);
        if !entered {
            if self.hover_gif.as_ref().is_some_and(|h| h.key == key) {
                self.stop_hover_gif(cx);
                cx.notify();
            }
            return;
        }
        if self.hover_gif.as_ref().is_some_and(|h| h.key == key)
            || crate::platform::reduced_motion()
        {
            return;
        }
        self.stop_hover_gif(cx);
        self.hover_gif = Some(HoverGif {
            key: key.clone(),
            frames: None,
            player: None,
        });
        let session = self.session.clone();
        let cell = cell.clone();
        // Device pixels: a 2x screen needs twice the cell size to stay sharp.
        let edge = (edge * 2.0).round() as u32;
        cx.spawn(async move |this, cx| {
            let frames = cx
                .background_executor()
                .spawn(async move {
                    let original = session.image(&format!("/{}/original", cell.id.0), None)?;
                    preview_image::hover_animation(&original.bytes, edge)
                })
                .await;
            this.update(cx, |this, cx| match this.hover_gif.as_mut() {
                Some(hover) if hover.key == key && hover.frames.is_none() => {
                    hover.player = frames.as_ref().map(|frames| {
                        let delays = (0..frames.frame_count())
                            .map(|index| Duration::from(frames.delay(index)));
                        GifPlayer::new(delays, true, Instant::now())
                    });
                    hover.frames = frames;
                    this.ensure_gif_ticker(cx);
                    cx.notify();
                }
                // The pointer moved on while it decoded: never uploaded, nothing to free.
                _ => {}
            })
            .ok();
        })
        .detach();
    }

    /// Frees the frames of the GIF playing under the pointer, if any.
    pub(crate) fn stop_hover_gif(&mut self, cx: &mut App) {
        if let Some(frames) = self.hover_gif.take().and_then(|h| h.frames) {
            cx.drop_image(frames, None);
        }
    }

    /// The frames and the frame to draw in the grid cell of `cell`, while it plays under the
    /// pointer.
    pub(crate) fn hover_frames(&self, cell: &ItemCell) -> Option<(Arc<RenderImage>, usize)> {
        let hover = self.hover_gif.as_ref()?;
        if hover.key.id != cell.id.0 || hover.key.version != cell.thumb_version {
            return None;
        }
        Some((hover.frames.clone()?, hover.player.as_ref()?.frame()))
    }
}
