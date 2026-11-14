//! Video and audio items: the player in the full-size preview and scrubbing a video's
//! frames under the pointer in the grid (D-170).
//!
//! The engine is `av::Player` (FFmpeg + the Scrobb audio path, D-168): it decodes on its own
//! threads and hands out upright BGRA frames when the clock reaches them. One task here (the
//! media ticker) takes each due frame, uploads it as a `RenderImage`, frees the previous one
//! and keeps the bar's time current; it stops once nothing plays and no answer is awaited.
//! Originals are opened by path, in Rust; their bytes never reach the UI.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use gpui_kit::{App, Context, RenderImage};
use library_core::{ItemCell, ItemId};

use crate::app::RootApp;
use crate::browse::media_bar;
use crate::browse::preview_cache::PreviewKey;

/// The longest edge of a frame in the preview. 4K stays sharp on large screens; larger video
/// is scaled down on the decode thread.
const PREVIEW_EDGE: u32 = 3840;
/// After a command, the ticker keeps looking this long for the frame it asked for, even paused.
const AWAKE: Duration = Duration::from_millis(1500);
/// How often a paused ticker looks for an awaited frame.
const POLL: Duration = Duration::from_millis(15);
/// While audio plays without video, the time on the bar moves this often.
const AUDIO_TICK: Duration = Duration::from_millis(200);
/// `J` / `L` jump this far.
pub const SKIP_SECONDS: f64 = 5.0;
/// Strips a video cell is split into for scrubbing under the pointer.
pub const SCRUB_STRIPS: usize = 24;

/// Whether the preview plays this item: a video or audio extension the engine knows.
pub fn is_media(ext: &str) -> bool {
    av::is_video(ext) || av::is_audio(ext)
}

/// Whether hovering this cell scrubs its frames: a video with an original and a poster.
pub fn scrubs_on_hover(cell: &ItemCell) -> bool {
    av::is_video(&cell.ext) && cell.has_thumbnail && !cell.pending
}

/// A video or audio item open in the preview.
pub struct MediaPreview {
    pub key: PreviewKey,
    pub audio_only: bool,
    /// `None` while the file opens (or when it could not be opened).
    pub player: Option<av::Player>,
    /// The frame on screen, ours to free.
    pub frame: Option<Arc<RenderImage>>,
    /// The file could not be played.
    pub failed: bool,
    /// The bar's copy of the player state, refreshed by the ticker.
    pub position: f64,
    pub duration: Option<f64>,
    pub paused: bool,
    pub speed: f64,
    pub volume: f32,
    pub muted: bool,
    /// The last segment dragged to on the seek track.
    scrub: Option<usize>,
    awake_until: Instant,
}

impl MediaPreview {
    fn new(key: PreviewKey, audio_only: bool, volume: f32, muted: bool) -> Self {
        Self {
            key,
            audio_only,
            player: None,
            frame: None,
            failed: false,
            position: 0.0,
            duration: None,
            paused: false,
            speed: 1.0,
            volume,
            muted,
            scrub: None,
            awake_until: Instant::now() + AWAKE,
        }
    }

    /// The displayed size of the video, from the frame on screen.
    pub fn natural(&self) -> Option<(f32, f32)> {
        let frame = self.frame.as_ref()?;
        let size = frame.size(0);
        Some((size.width.0 as f32, size.height.0 as f32))
    }

    fn wake(&mut self) {
        self.awake_until = Instant::now() + AWAKE;
    }

    /// Copies the player's state for the bar; true when something visible changed.
    fn sync(&mut self) -> bool {
        let Some(player) = self.player.as_ref() else {
            return false;
        };
        let position = player.position();
        let paused = player.is_paused();
        let changed = media_bar::format_time(position) != media_bar::format_time(self.position)
            || media_bar::played_segments(position, self.duration)
                != media_bar::played_segments(self.position, self.duration)
            || paused != self.paused;
        self.position = position;
        self.paused = paused;
        self.speed = player.speed();
        changed
    }
}

/// A video whose frames follow the pointer in a grid cell. The reader lives on its own thread
/// (FFmpeg's scaler cannot move between threads); the newest wanted time wins.
pub struct HoverScrub {
    pub key: PreviewKey,
    /// Where the pointer is along the cell, 0…1, for the progress line.
    pub fraction: f32,
    pub frame: Option<Arc<RenderImage>>,
    wanted: Sender<f32>,
    frames: Receiver<av::VideoFrame>,
    /// A time was asked for and its frame has not come yet.
    waiting: bool,
}

/// Opens the reader for `path` on a thread of its own; it answers each wanted fraction with
/// the key frame there, dropping fractions that a newer one overtook.
fn spawn_scrubber(path: PathBuf, edge: u32) -> (Sender<f32>, Receiver<av::VideoFrame>) {
    let (wanted_tx, wanted_rx) = mpsc::channel::<f32>();
    let (frames_tx, frames_rx) = mpsc::channel();
    let _ = thread::Builder::new()
        .name("av-scrub".into())
        .spawn(move || {
            let Ok(mut reader) = av::FrameReader::open(&path, edge, av::PixelOrder::Bgra) else {
                return;
            };
            let Some(duration) = reader.info().duration else {
                return;
            };
            while let Ok(mut fraction) = wanted_rx.recv() {
                while let Ok(newer) = wanted_rx.try_recv() {
                    fraction = newer;
                }
                let at = f64::from(fraction.clamp(0.0, 1.0)) * duration;
                // Key frames only: instant on long files, and close enough in a small cell.
                let frame = reader
                    .seek_key(at)
                    .ok()
                    .and_then(|()| reader.next_frame().ok().flatten());
                if let Some(frame) = frame
                    && frames_tx.send(frame).is_err()
                {
                    return;
                }
            }
        });
    (wanted_tx, frames_rx)
}

fn render_image(frame: av::VideoFrame) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(frame.image)]))
}

fn key_of(cell: &ItemCell) -> PreviewKey {
    PreviewKey {
        id: cell.id.0.clone(),
        version: cell.thumb_version,
    }
}

impl RootApp {
    /// Starts the player for the item the preview just opened, or stops it for anything else.
    pub(crate) fn open_media(&mut self, cell: &ItemCell, cx: &mut Context<Self>) {
        let key = key_of(cell);
        if self.media.as_ref().is_some_and(|media| media.key == key) {
            return;
        }
        self.close_media(cx);
        if !is_media(&cell.ext) {
            return;
        }
        let audio_only = av::is_audio(&cell.ext);
        let mut media =
            MediaPreview::new(key.clone(), audio_only, self.media_volume, self.media_muted);
        media.duration = cell_duration(cell);
        self.media = Some(media);
        let session = self.session.clone();
        let id = ItemId(cell.id.0.clone());
        // A loose file is opened by its own path (D-180).
        let loose_path = self
            .loose
            .is_some()
            .then(|| crate::file_viewer::cell_path(cell));
        let (volume, muted) = (self.media_volume, self.media_muted);
        cx.spawn(async move |this, cx| {
            let opened = cx
                .background_executor()
                .spawn(async move {
                    let path = match loose_path {
                        Some(path) => path,
                        None => session.original_paths(&[id]).ok()?.into_iter().next()?,
                    };
                    let options = av::PlayerOptions {
                        max_edge: PREVIEW_EDGE,
                        volume,
                        muted,
                        ..av::PlayerOptions::default()
                    };
                    av::Player::open(&path, options).ok()
                })
                .await;
            this.update(cx, |this, cx| {
                let Some(media) = this.media.as_mut().filter(|media| media.key == key) else {
                    // The preview moved on: dropping the player stops its threads.
                    return;
                };
                match opened {
                    Some(player) => {
                        media.duration = player.duration().or(media.duration);
                        media.paused = player.is_paused();
                        media.player = Some(player);
                        media.wake();
                    }
                    None => media.failed = true,
                }
                this.ensure_media_ticker(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Stops the player and frees its frame.
    pub(crate) fn close_media(&mut self, cx: &mut App) {
        if let Some(frame) = self.media.take().and_then(|media| media.frame) {
            cx.drop_image(frame, None);
        }
    }

    fn with_player(
        &mut self,
        cx: &mut Context<Self>,
        act: impl FnOnce(&mut MediaPreview, &av::Player),
    ) {
        let Some(media) = self.media.as_mut() else {
            return;
        };
        if let Some(player) = media.player.take() {
            act(media, &player);
            media.player = Some(player);
        }
        media.wake();
        media.sync();
        self.ensure_media_ticker(cx);
        cx.notify();
    }

    /// `P`, `K` or the play button.
    pub(crate) fn media_toggle(&mut self, cx: &mut Context<Self>) {
        self.with_player(cx, |_, player| player.toggle());
    }

    /// `,` / `.` or the frame buttons.
    pub(crate) fn media_step(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.with_player(cx, |_, player| {
            if delta < 0 {
                player.step_back();
            } else {
                player.step_forward();
            }
        });
    }

    /// `J` / `L`: back or forward by [`SKIP_SECONDS`].
    pub(crate) fn media_skip(&mut self, seconds: f64, cx: &mut Context<Self>) {
        self.with_player(cx, |_, player| {
            player.seek(player.position() + seconds, av::SeekMode::Exact);
        });
    }

    /// A click or drag on the seek track: key frames while the button is down.
    pub(crate) fn media_scrub(&mut self, segment: usize, cx: &mut Context<Self>) {
        let Some(media) = self.media.as_mut() else {
            return;
        };
        if media.scrub == Some(segment) {
            return;
        }
        media.scrub = Some(segment);
        let duration = media.duration;
        self.with_player(cx, |_, player| {
            if let Some(duration) = duration {
                player.seek(
                    media_bar::segment_time(segment, duration),
                    av::SeekMode::Key,
                );
            }
        });
    }

    /// The button came up after a scrub: the exact frame where it stopped, sound again.
    pub(crate) fn media_scrub_end(&mut self, cx: &mut Context<Self>) {
        let Some(segment) = self.media.as_mut().and_then(|media| media.scrub.take()) else {
            return;
        };
        let duration = self.media.as_ref().and_then(|media| media.duration);
        self.with_player(cx, |_, player| {
            if let Some(duration) = duration {
                player.seek(
                    media_bar::segment_time(segment, duration),
                    av::SeekMode::Exact,
                );
            }
        });
    }

    pub(crate) fn media_cycle_speed(&mut self, cx: &mut Context<Self>) {
        self.with_player(cx, |media, player| {
            player.set_speed(media_bar::next_speed(media.speed));
        });
    }

    /// `M` or the speaker button. Remembered for the next item.
    pub(crate) fn media_toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.media_muted = !self.media_muted;
        self.apply_media_volume(cx);
    }

    pub(crate) fn media_set_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        self.media_volume = volume.clamp(0.0, 1.0);
        self.media_muted = false;
        self.apply_media_volume(cx);
    }

    fn apply_media_volume(&mut self, cx: &mut Context<Self>) {
        let (volume, muted) = (self.media_volume, self.media_muted);
        if let Some(media) = self.media.as_mut() {
            media.volume = volume;
            media.muted = muted;
            if let Some(player) = media.player.as_mut() {
                player.set_volume(volume);
                player.set_muted(muted);
            }
        }
        cx.notify();
    }

    /// Starts the task that shows due frames, unless one runs. It sleeps until the next frame
    /// is due and ends, in the same update that sees it, once nothing plays or is awaited.
    pub(crate) fn ensure_media_ticker(&mut self, cx: &mut Context<Self>) {
        if self.media_ticking || self.media_tick_wait(Instant::now()).is_none() {
            return;
        }
        self.media_ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                let wait = this
                    .update(cx, |this, cx| {
                        let wait = this.media_tick(cx);
                        if wait.is_none() {
                            this.media_ticking = false;
                        }
                        wait
                    })
                    .ok()
                    .flatten();
                let Some(wait) = wait else {
                    break;
                };
                cx.background_executor()
                    .timer(wait.max(Duration::from_millis(4)))
                    .await;
            }
        })
        .detach();
    }

    /// One step of the ticker: new frames on screen, the bar's time. Returns when to look again.
    fn media_tick(&mut self, cx: &mut Context<Self>) -> Option<Duration> {
        let mut changed = false;
        let mut freed = Vec::new();
        if let Some(media) = self.media.as_mut()
            && let Some(player) = media.player.as_ref()
        {
            if let Some(frame) = player.take_frame() {
                if let Some(old) = media.frame.replace(render_image(frame)) {
                    freed.push(old);
                }
                changed = true;
            }
            if !player.is_paused() && player.ended() {
                player.pause();
                media.wake();
            }
            changed |= media.sync();
        }
        if let Some(scrub) = self.hover_scrub.as_mut() {
            let mut newest = None;
            loop {
                match scrub.frames.try_recv() {
                    Ok(frame) => newest = Some(frame),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        scrub.waiting = false;
                        break;
                    }
                }
            }
            if let Some(frame) = newest {
                if let Some(old) = scrub.frame.replace(render_image(frame)) {
                    freed.push(old);
                }
                scrub.waiting = false;
                changed = true;
            }
        }
        for image in freed {
            cx.drop_image(image, None);
        }
        if changed {
            cx.notify();
        }
        self.media_tick_wait(Instant::now())
    }

    /// When the ticker should look again; `None` when nothing plays or is awaited.
    fn media_tick_wait(&self, now: Instant) -> Option<Duration> {
        let scrubbing = self.hover_scrub.as_ref().is_some_and(|scrub| scrub.waiting);
        let media = self.media.as_ref();
        let playing = media
            .and_then(|media| media.player.as_ref())
            .is_some_and(|player| !player.is_paused());
        let awake = media.is_some_and(|media| now < media.awake_until);
        if playing {
            let player = media.and_then(|media| media.player.as_ref())?;
            let next = if media.is_some_and(|media| media.audio_only) {
                AUDIO_TICK
            } else {
                player.next_frame_in().unwrap_or(POLL).min(AUDIO_TICK)
            };
            return Some(if scrubbing { next.min(POLL) } else { next });
        }
        (awake || scrubbing).then_some(POLL)
    }

    /// The pointer is over strip `strip` of a video cell (`None`: it left the cell).
    pub(crate) fn hover_scrub(
        &mut self,
        cell: &ItemCell,
        strip: Option<usize>,
        edge: f32,
        cx: &mut Context<Self>,
    ) {
        let key = key_of(cell);
        let Some(strip) = strip else {
            if self
                .hover_scrub
                .as_ref()
                .is_some_and(|scrub| scrub.key == key)
            {
                self.stop_hover_scrub(cx);
                cx.notify();
            }
            return;
        };
        let fraction = (strip as f32 + 0.5) / SCRUB_STRIPS as f32;
        if self
            .hover_scrub
            .as_ref()
            .is_none_or(|scrub| scrub.key != key)
        {
            self.stop_hover_scrub(cx);
            // The original's path is cheap to look up; the reader opens on its own thread.
            let Ok(Some(path)) = self
                .session
                .original_paths(&[ItemId(cell.id.0.clone())])
                .map(|paths| paths.into_iter().next())
            else {
                return;
            };
            // Device pixels: a 2x screen needs twice the cell size to stay sharp.
            let (wanted, frames) = spawn_scrubber(path, ((edge * 2.0).round() as u32).max(64));
            self.hover_scrub = Some(HoverScrub {
                key,
                fraction,
                frame: None,
                wanted,
                frames,
                waiting: false,
            });
        }
        if let Some(scrub) = self.hover_scrub.as_mut() {
            scrub.fraction = fraction;
            scrub.waiting = scrub.wanted.send(fraction).is_ok();
        }
        self.ensure_media_ticker(cx);
        cx.notify();
    }

    /// Frees the frame of the video scrubbed under the pointer, if any; its thread ends.
    pub(crate) fn stop_hover_scrub(&mut self, cx: &mut App) {
        if let Some(frame) = self.hover_scrub.take().and_then(|scrub| scrub.frame) {
            cx.drop_image(frame, None);
        }
    }

    /// The frame to draw in `cell` and where the pointer is along it, while it is scrubbed.
    pub(crate) fn scrub_frame(&self, cell: &ItemCell) -> Option<(Option<Arc<RenderImage>>, f32)> {
        let scrub = self.hover_scrub.as_ref()?;
        (scrub.key.id == cell.id.0 && scrub.key.version == cell.thumb_version)
            .then(|| (scrub.frame.clone(), scrub.fraction))
    }
}

/// The duration the library knows for a cell, before the player has read the file.
fn cell_duration(cell: &ItemCell) -> Option<f64> {
    cell.duration_ms.map(|ms| ms as f64 / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_extensions() {
        assert!(is_media("MP4") && is_media("webm") && is_media("flac"));
        assert!(!is_media("gif") && !is_media("png"));
        let mut cell = ItemCell {
            ext: "mov".into(),
            has_thumbnail: true,
            ..ItemCell::default()
        };
        assert!(scrubs_on_hover(&cell));
        cell.has_thumbnail = false;
        assert!(!scrubs_on_hover(&cell));
        cell.ext = "mp3".into();
        cell.has_thumbnail = true;
        assert!(!scrubs_on_hover(&cell));
    }

    #[test]
    fn scrubber_answers_with_frames() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../av/fixtures/clip.mp4");
        let (wanted, frames) = spawn_scrubber(path, 32);
        wanted.send(0.9).unwrap();
        let frame = frames.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(frame.image.dimensions(), (32, 18));
        // Key frames every second: 0.9 of 2 s lands on the one at 1 s.
        assert!((frame.time - 1.0).abs() < 0.05, "{}", frame.time);
    }
}
