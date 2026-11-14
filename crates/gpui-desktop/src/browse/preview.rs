//! Full-size preview overlay: fit / zoom / pan / rotate / prev-next, and the GIF player
//! (D-154).

use std::sync::Arc;
use std::time::{Duration, Instant};

use app_store::Locale;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::{Disableable as _, StyledExt as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Bounds, Corners, CursorStyle, Entity, InteractiveElement, IntoElement, MouseButton,
    ObjectFit, ParentElement, Pixels, RenderImage, SharedString, Styled, StyledImage as _, black,
    canvas, div, img, px, white,
};

use crate::app::RootApp;
use crate::browse::gif_player::{self, GifPlayer};
use crate::browse::media_bar;
use crate::browse::preview_cache::PreviewKey;
use crate::browse::preview_image::Shown;
use crate::browse::space_preview::{self, SpaceDown};
use crate::i18n::t;
use crate::ops_media::MediaPreview;
use crate::platform;

/// Zoom limits, relative to the fit (1.0 = 100 %).
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;

#[derive(Clone)]
pub struct PreviewState {
    pub index: usize,
    pub total: u32,
    pub name: SharedString,
    /// The thumbnail until the full image is decoded, then the full image (D-143).
    pub image: Option<Shown>,
    /// The item and version this state shows; background loads match against it.
    pub key: Option<PreviewKey>,
    /// The full image is still decoding.
    pub loading: bool,
    /// The original's pixel size from the library, used to size the thumbnail placeholder
    /// exactly where the full image will land.
    pub natural: Option<(f32, f32)>,
    pub zoom: f32,
    pub offset: (f32, f32),
    /// Clockwise degrees: 0, 90, 180, 270. Session-only (D-058).
    pub rotation: u16,
    pub drag_origin: Option<(f32, f32)>,
    /// 0.0…1.0 appear progress for best-effort fade (2.4).
    pub appear: f32,
    pub opened_at: Instant,
    /// Playback of an animated GIF; set whenever [`Self::image`] is an animation (D-154).
    pub player: Option<GifPlayer>,
}

impl PreviewState {
    pub fn new(
        index: usize,
        total: u32,
        name: impl Into<SharedString>,
        image: Option<Shown>,
    ) -> Self {
        Self {
            index,
            total,
            name: name.into(),
            image,
            key: None,
            loading: false,
            natural: None,
            zoom: 1.0,
            offset: (0.0, 0.0),
            rotation: 0,
            drag_origin: None,
            appear: if platform::reduced_motion() { 1.0 } else { 0.0 },
            opened_at: Instant::now(),
            player: None,
        }
        .with_player()
    }

    fn with_player(mut self) -> Self {
        self.sync_player(Instant::now());
        self
    }

    /// Shows `image`. An animation gets a player; the same GIF turned (same frame count)
    /// keeps its frame, speed and play state.
    pub fn show(&mut self, image: Option<Shown>, now: Instant) {
        self.image = image;
        self.sync_player(now);
    }

    fn sync_player(&mut self, now: Instant) {
        let Some(Shown::Animation(frames)) = &self.image else {
            self.player = None;
            return;
        };
        let count = frames.frame_count();
        if self.player.as_ref().map(GifPlayer::frame_count) == Some(count) {
            return;
        }
        let delays = (0..count).map(|index| Duration::from(frames.delay(index)));
        self.player = Some(GifPlayer::new(delays, !platform::reduced_motion(), now));
    }

    /// Sets the zoom (1.0 is the fit), between [`MIN_ZOOM`] and [`MAX_ZOOM`]. Steps that land
    /// within 1 % of the fit snap to it, so wheel steps come back to exactly 100 %.
    pub fn zoom_to(&mut self, next: f32) {
        let clamped = next.clamp(MIN_ZOOM, MAX_ZOOM);
        self.zoom = if (clamped - 1.0).abs() < 0.01 {
            1.0
        } else {
            clamped
        };
    }

    /// Zooms about the middle of the stage: the point there stays put, so the pan offset
    /// scales by the same factor as the image.
    pub fn zoom_by(&mut self, factor: f32) {
        let before = self.zoom;
        self.zoom_to(before * factor);
        let applied = self.zoom / before;
        self.offset = (self.offset.0 * applied, self.offset.1 * applied);
    }

    /// Fit and centred again (`0`, or a click on the zoom percentage).
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.offset = (0.0, 0.0);
    }

    pub fn rotate_cw(&mut self) {
        self.rotation = (self.rotation + 90) % 360;
        self.offset = (0.0, 0.0);
    }

    pub fn tick_appear(&mut self) -> bool {
        if self.appear >= 1.0 || platform::reduced_motion() {
            self.appear = 1.0;
            return false;
        }
        let elapsed = self.opened_at.elapsed().as_secs_f32();
        let next = (elapsed / 0.14).min(1.0);
        let changed = (next - self.appear).abs() > 0.01;
        self.appear = next;
        changed
    }
}

/// Space hold tracking while a key is down.
#[derive(Debug, Clone)]
pub struct SpaceGesture {
    pub down: SpaceDown,
    pub since: Instant,
}

impl SpaceGesture {
    pub fn begin(is_open: bool) -> (Self, bool) {
        let (down, open) = space_preview::space_key_down(is_open);
        (
            Self {
                down,
                since: Instant::now(),
            },
            open,
        )
    }

    pub fn mark_held_if_due(&mut self) -> bool {
        if self.down.held {
            return false;
        }
        if self.since.elapsed() >= Duration::from_millis(space_preview::SPACE_HOLD_MS) {
            self.down = space_preview::space_hold(self.down);
            return true;
        }
        false
    }

    pub fn end(self) -> bool {
        space_preview::space_key_up(self.down)
    }
}

/// Height of the preview's top bar; the stage is the window below it.
pub const CHROME_HEIGHT: f32 = 44.0;

/// The cursor over the draggable image: an open hand that closes while dragging. On Windows
/// both come from the exe's cursor resources through the patched GPUI backend (D-148).
pub fn drag_cursor(dragging: bool) -> CursorStyle {
    if dragging {
        CursorStyle::ClosedHand
    } else {
        CursorStyle::OpenHand
    }
}

/// One wheel notch zooms by this factor.
const WHEEL_ZOOM: f32 = 1.1;

/// Wheel forward (away from the user) zooms in, back zooms out. GPUI reports a forward
/// notch as a positive `y` (Windows: `WM_MOUSEWHEEL` `hiword(wParam) / WHEEL_DELTA`).
pub fn wheel_zoom_factor(delta_y: f32) -> Option<f32> {
    if delta_y > 0.0 {
        Some(WHEEL_ZOOM)
    } else if delta_y < 0.0 {
        Some(1.0 / WHEEL_ZOOM)
    } else {
        None
    }
}

/// The image box on the stage: the natural size scaled to fit (never enlarged past 100 %),
/// times `zoom`. Without a known size the box is the whole stage and `Contain` fits inside.
pub fn fit_box(natural: Option<(f32, f32)>, stage: (f32, f32), zoom: f32) -> (f32, f32) {
    let (sw, sh) = (stage.0.max(1.0), stage.1.max(1.0));
    let Some((w, h)) = natural.filter(|(w, h)| *w > 0.0 && *h > 0.0) else {
        return (sw * zoom, sh * zoom);
    };
    let fit = (sw / w).min(sh / h).min(1.0);
    (w * fit * zoom, h * fit * zoom)
}

/// The size the shown frame stands for: decoded pixels know theirs (rotation included);
/// the thumbnail placeholder stands in for the original.
fn natural_size(state: &PreviewState) -> Option<(f32, f32)> {
    match &state.image {
        Some(Shown::Pixels(image) | Shown::Animation(image)) => {
            let size = image.size(0);
            Some((size.width.0 as f32, size.height.0 as f32))
        }
        _ => state.natural,
    }
}

pub fn render_preview(
    state: &PreviewState,
    media: Option<&MediaPreview>,
    locale: Locale,
    window_size: (f32, f32),
    entity: Entity<RootApp>,
) -> impl IntoElement {
    let appear = state.appear.clamp(0.0, 1.0);
    let overlay_alpha = 0.88 * appear;
    let can_prev = state.index > 0;
    let can_next = (state.index as u32 + 1) < state.total;
    let zoom_pct = (state.zoom * 100.0).round() as i32;
    let (ox, oy) = state.offset;
    let zoom = state.zoom;
    let rotation = state.rotation;
    let dragging = state.drag_origin.is_some();
    let stage = (window_size.0, (window_size.1 - CHROME_HEIGHT).max(1.0));
    // A playing video sizes its box by its frames (D-170).
    let natural = media
        .and_then(MediaPreview::natural)
        .or_else(|| natural_size(state));
    let (box_w, box_h) = fit_box(natural, stage, zoom);

    let close_entity = entity.clone();
    let prev_entity = entity.clone();
    let next_entity = entity.clone();
    let zoom_out = entity.clone();
    let zoom_in = entity.clone();
    let zoom_reset = entity.clone();
    let rotate = entity.clone();
    let pan_entity = entity.clone();
    let wheel_entity = entity.clone();
    let dismiss = entity.clone();

    div()
        .id("preview-overlay")
        .absolute()
        .inset_0()
        // The library under the preview gets no mouse at all: no hover, no clicks and, above
        // all, no wheel, which zooms the image and must not scroll the grid behind it.
        .occlude()
        .on_scroll(|_, _, cx| cx.stop_propagation())
        .flex()
        .flex_col()
        .bg(black().opacity(overlay_alpha))
        .text_color(white())
        .child(
            h_flex()
                .id("preview-chrome")
                .h(px(CHROME_HEIGHT))
                .px_3()
                .gap_2()
                .items_center()
                .child(Label::new(state.name.clone()).font_semibold())
                .child(
                    Label::new(format!("{} / {}", state.index + 1, state.total))
                        .text_sm()
                        .text_color(white().opacity(0.65)),
                )
                .when(rotation != 0, |el| {
                    el.child(
                        Label::new(format!("{rotation}°"))
                            .text_xs()
                            .text_color(white().opacity(0.55)),
                    )
                })
                .when(state.loading, |el| {
                    el.child(
                        Label::new(t(locale, "preview.loading"))
                            .text_xs()
                            .text_color(white().opacity(0.55)),
                    )
                })
                .child(div().flex_1())
                .child(
                    Button::new("preview-zoom-out")
                        .ghost()
                        .label("−")
                        .disabled(state.zoom <= MIN_ZOOM)
                        .on_click(move |_, _, cx| {
                            zoom_out.update(cx, |this, cx| this.preview_zoom(1.0 / 1.25, cx));
                        }),
                )
                .child(
                    Button::new("preview-zoom-label")
                        .ghost()
                        .label(format!("{zoom_pct}%"))
                        .on_click(move |_, _, cx| {
                            zoom_reset.update(cx, |this, cx| this.preview_zoom_reset(cx));
                        }),
                )
                .child(
                    Button::new("preview-zoom-in")
                        .ghost()
                        .label("+")
                        .disabled(state.zoom >= MAX_ZOOM)
                        .on_click(move |_, _, cx| {
                            zoom_in.update(cx, |this, cx| this.preview_zoom(1.25, cx));
                        }),
                )
                .child(
                    Button::new("preview-rotate")
                        .ghost()
                        .label(t(locale, "preview.rotate"))
                        .on_click(move |_, _, cx| {
                            rotate.update(cx, |this, cx| this.preview_rotate(cx));
                        }),
                )
                .child(Button::new("preview-close").ghost().label("×").on_click(
                    move |_, _, cx| {
                        close_entity.update(cx, |this, cx| this.close_preview(cx));
                    },
                )),
        )
        .child(
            div()
                .id("preview-stage")
                .relative()
                .flex_1()
                .overflow_hidden()
                .child(
                    // Letterbox dismiss layer behind the image (D-073). The image box is the
                    // image's own size, so a click beside it lands here.
                    div()
                        .id("preview-letterbox")
                        .absolute()
                        .inset_0()
                        .on_mouse_down(MouseButton::Left, {
                            let dismiss = dismiss.clone();
                            move |_, _, cx| {
                                dismiss.update(cx, |this, cx| this.close_preview(cx));
                            }
                        })
                        .on_mouse_down(MouseButton::Right, {
                            let dismiss = dismiss.clone();
                            move |_, _, cx| {
                                dismiss.update(cx, |this, cx| this.close_preview(cx));
                            }
                        }),
                )
                .child(
                    // The whole stage tracks the drag, so a pan keeps going when the
                    // pointer leaves the image; only the image moves.
                    div()
                        .id("preview-stage-inner")
                        .absolute()
                        .inset_0()
                        // The hand stays while dragging, also past the image's edge.
                        .when(dragging, |el| el.cursor(drag_cursor(true)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .opacity(appear)
                        .on_scroll(move |event, _, cx| {
                            cx.stop_propagation();
                            let delta = event.delta.pixel_delta(px(1.)).y.as_f32();
                            if let Some(factor) = wheel_zoom_factor(delta) {
                                wheel_entity.update(cx, |this, cx| this.preview_zoom(factor, cx));
                            }
                        })
                        .on_mouse_up(MouseButton::Left, {
                            let pan = pan_entity.clone();
                            move |_, _, cx| {
                                pan.update(cx, |this, cx| {
                                    this.preview_pan_end(cx);
                                    // A seek drag released off the track still ends.
                                    this.media_scrub_end(cx);
                                });
                            }
                        })
                        .on_mouse_move({
                            let pan = pan_entity.clone();
                            move |event, _, cx| {
                                let pos = event.position;
                                let held = event.pressed_button == Some(MouseButton::Left);
                                pan.update(cx, |this, cx| {
                                    // Released outside the window: the mouse-up never came here,
                                    // so the hand opens again on the next move.
                                    if held {
                                        this.preview_pan_move(pos.x.as_f32(), pos.y.as_f32(), cx);
                                    } else if this.preview_is_dragging() {
                                        this.preview_pan_end(cx);
                                    }
                                });
                            }
                        })
                        .child(match (media, &state.image) {
                            // Audio: a card with the name and the length (D-170).
                            (Some(media), _) if media.audio_only => {
                                media_bar::audio_card(&state.name, media, locale)
                            }
                            // Video: its frames once they come, the poster until then.
                            (Some(media), shown) if media.frame.is_some() || shown.is_none() => {
                                let pan = pan_entity.clone();
                                let grab = move |event: &gpui_kit::MouseDownEvent,
                                                 _: &mut gpui_kit::Window,
                                                 cx: &mut gpui_kit::App| {
                                    cx.stop_propagation();
                                    let pos = event.position;
                                    pan.update(cx, |this, cx| {
                                        this.preview_pan_start(
                                            pos.x.as_f32() - ox,
                                            pos.y.as_f32() - oy,
                                            cx,
                                        );
                                    });
                                };
                                div()
                                    .id("preview-video")
                                    .flex_none()
                                    .relative()
                                    .left(px(ox))
                                    .top(px(oy))
                                    .w(px(box_w))
                                    .h(px(box_h))
                                    .cursor(drag_cursor(dragging))
                                    .on_mouse_down(MouseButton::Left, grab)
                                    .when_some(media.frame.clone(), |el, frame| {
                                        el.child(animation_frame(frame, 0, ObjectFit::Contain, 0.0))
                                    })
                                    .when(media.failed, |el| {
                                        el.flex().items_center().justify_center().child(
                                            Label::new(t(locale, "preview.media.failed")).text_lg(),
                                        )
                                    })
                                    .into_any_element()
                            }
                            (_, Some(shown)) => {
                                let pan = pan_entity.clone();
                                // Grab and drag at any zoom. A click on the image never
                                // reaches the letterbox, so it does not close the preview.
                                let grab = move |event: &gpui_kit::MouseDownEvent,
                                                 _: &mut gpui_kit::Window,
                                                 cx: &mut gpui_kit::App| {
                                    cx.stop_propagation();
                                    let pos = event.position;
                                    pan.update(cx, |this, cx| {
                                        this.preview_pan_start(
                                            pos.x.as_f32() - ox,
                                            pos.y.as_f32() - oy,
                                            cx,
                                        );
                                    });
                                };
                                match (shown, &state.player) {
                                    (Shown::Animation(frames), Some(player)) => div()
                                        .id("preview-image")
                                        .flex_none()
                                        .relative()
                                        .left(px(ox))
                                        .top(px(oy))
                                        .w(px(box_w))
                                        .h(px(box_h))
                                        .cursor(drag_cursor(dragging))
                                        .on_mouse_down(MouseButton::Left, grab)
                                        .child(animation_frame(
                                            frames.clone(),
                                            player.frame(),
                                            ObjectFit::Contain,
                                            0.0,
                                        ))
                                        .into_any_element(),
                                    _ => img(shown.source())
                                        .id("preview-image")
                                        .flex_none()
                                        .relative()
                                        .left(px(ox))
                                        .top(px(oy))
                                        .object_fit(ObjectFit::Contain)
                                        .w(px(box_w))
                                        .h(px(box_h))
                                        .cursor(drag_cursor(dragging))
                                        .on_mouse_down(MouseButton::Left, grab)
                                        .into_any_element(),
                                }
                            }
                            (_, None) => Label::new(t(locale, "preview.unavailable"))
                                .text_lg()
                                .into_any_element(),
                        }),
                )
                .child(
                    Button::new("preview-prev")
                        .absolute()
                        .left_3()
                        .top(px(stage.1 / 2.0 - 16.0))
                        .ghost()
                        .label("‹")
                        .disabled(!can_prev)
                        .on_click(move |_, _, cx| {
                            prev_entity.update(cx, |this, cx| this.preview_step(-1, cx));
                        }),
                )
                .child(
                    Button::new("preview-next")
                        .absolute()
                        .right_3()
                        .top(px(stage.1 / 2.0 - 16.0))
                        .ghost()
                        .label("›")
                        .disabled(!can_next)
                        .on_click(move |_, _, cx| {
                            next_entity.update(cx, |this, cx| this.preview_step(1, cx));
                        }),
                )
                .when_some(state.player.as_ref(), |el, player| {
                    el.child(gif_controls(player, locale, entity.clone()))
                })
                .when_some(media, |el, media| {
                    el.child(media_bar::media_controls(media, locale, entity.clone()))
                }),
        )
}

/// One frame of an animation, fitted into its parent. GPUI's `img` would play the frames on
/// its own clock (racing through 0 ms delays, stopping in an inactive window); painting the
/// frame directly lets [`GifPlayer`] pause, step, seek and set the speed. Also the grid's
/// hover playback (D-154).
pub fn animation_frame(
    frames: Arc<RenderImage>,
    frame: usize,
    fit: ObjectFit,
    radius: f32,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window, _| {
            let size = frames.size(frame);
            let fitted = fit.get_bounds(bounds, size);
            window
                .paint_image(
                    bounds,
                    fitted,
                    Corners::all(px(radius)),
                    frames,
                    frame,
                    false,
                )
                .ok();
        },
    )
    .size_full()
}

/// Height of the GIF player bar and its gap to the bottom of the stage.
const CONTROLS_HEIGHT: f32 = 40.0;
/// Width of the scrubber track.
const TRACK_WIDTH: f32 = 320.0;

fn speed_label(speed: f32) -> String {
    let text = format!("{speed}");
    format!("{text}×")
}

/// The GIF player bar at the bottom of the stage: previous frame, play / pause, next frame,
/// a scrubber to click or drag, the frame number and the speed (D-154).
fn gif_controls(player: &GifPlayer, locale: Locale, entity: Entity<RootApp>) -> AnyElement {
    let frames = player.frame_count();
    let segments = gif_player::segment_count(frames);
    let reached = gif_player::segment_for_frame(player.frame(), frames);
    let playing = player.is_playing();
    let (back, toggle, forward, speed) = (
        entity.clone(),
        entity.clone(),
        entity.clone(),
        entity.clone(),
    );
    let seconds = player.duration().as_secs_f32();

    let track = h_flex()
        .id("gif-track")
        .w(px(TRACK_WIDTH))
        .h(px(20.0))
        .items_center()
        .cursor(CursorStyle::PointingHand)
        .children((0..segments).map(|segment| {
            let seek = entity.clone();
            let drag = entity.clone();
            let frame = gif_player::frame_for_segment(segment, frames);
            div()
                .id(("gif-segment", segment))
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .child(div().w_full().h(px(4.0)).bg(if segment <= reached {
                    white().opacity(0.9)
                } else {
                    white().opacity(0.25)
                }))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    seek.update(cx, |this, cx| this.preview_gif_seek(frame, cx));
                })
                // Dragging along the track scrubs through the frames.
                .on_mouse_move(move |event, _, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        drag.update(cx, |this, cx| this.preview_gif_seek(frame, cx));
                    }
                })
        }));

    h_flex()
        .id("gif-controls")
        .absolute()
        .bottom(px(12.0))
        .left_0()
        .right_0()
        .justify_center()
        .child(
            h_flex()
                .id("gif-controls-bar")
                // Clicks on the bar neither close the preview nor start a pan.
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .h(px(CONTROLS_HEIGHT))
                .px_2()
                .gap_1()
                .items_center()
                .rounded(px(8.0))
                .bg(black().opacity(0.7))
                .child(
                    Button::new("gif-prev-frame")
                        .ghost()
                        // White on the dark bar in either theme.
                        .text_color(white())
                        .icon(IconName::StepBack)
                        .tooltip(t(locale, "preview.gif.prevFrame"))
                        .on_click(move |_, _, cx| {
                            back.update(cx, |this, cx| this.preview_gif_step(-1, cx));
                        }),
                )
                .child(
                    Button::new("gif-play")
                        .ghost()
                        .text_color(white())
                        .icon(if playing {
                            IconName::Pause
                        } else {
                            IconName::Play
                        })
                        .tooltip(t(
                            locale,
                            if playing {
                                "preview.gif.pause"
                            } else {
                                "preview.gif.play"
                            },
                        ))
                        .on_click(move |_, _, cx| {
                            toggle.update(cx, |this, cx| this.preview_gif_toggle(cx));
                        }),
                )
                .child(
                    Button::new("gif-next-frame")
                        .ghost()
                        .text_color(white())
                        .icon(IconName::StepForward)
                        .tooltip(t(locale, "preview.gif.nextFrame"))
                        .on_click(move |_, _, cx| {
                            forward.update(cx, |this, cx| this.preview_gif_step(1, cx));
                        }),
                )
                .child(div().px_2().child(track))
                // Fixed widths: the bar keeps its place (and the track stays under a
                // dragging pointer) while the numbers change.
                .child(
                    h_flex().w(px(72.0)).justify_end().child(
                        Label::new(format!("{} / {frames}", player.frame() + 1))
                            .text_xs()
                            .text_color(white().opacity(0.8)),
                    ),
                )
                .child(
                    h_flex().w(px(48.0)).justify_end().child(
                        Label::new(format!("{seconds:.1} s"))
                            .text_xs()
                            .text_color(white().opacity(0.55)),
                    ),
                )
                .child(
                    Button::new("gif-speed")
                        .ghost()
                        .text_color(white())
                        .w(px(56.0))
                        .label(speed_label(player.speed()))
                        .tooltip(t(locale, "preview.gif.speed"))
                        .on_click(move |_, _, cx| {
                            speed.update(cx, |this, cx| this.preview_gif_speed(cx));
                        }),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_box_scales_down_large_images() {
        assert_eq!(
            fit_box(Some((4000.0, 2000.0)), (1000.0, 800.0), 1.0),
            (1000.0, 500.0)
        );
        assert_eq!(
            fit_box(Some((1000.0, 4000.0)), (1000.0, 800.0), 1.0),
            (200.0, 800.0)
        );
    }

    #[test]
    fn fit_box_keeps_small_images_at_actual_size() {
        assert_eq!(
            fit_box(Some((300.0, 200.0)), (1000.0, 800.0), 1.0),
            (300.0, 200.0)
        );
        assert_eq!(
            fit_box(Some((300.0, 200.0)), (1000.0, 800.0), 2.0),
            (600.0, 400.0)
        );
    }

    #[test]
    fn fit_box_without_a_size_fills_the_stage() {
        assert_eq!(fit_box(None, (1000.0, 800.0), 1.0), (1000.0, 800.0));
        assert_eq!(
            fit_box(Some((0.0, 10.0)), (1000.0, 800.0), 1.0),
            (1000.0, 800.0)
        );
    }

    #[test]
    fn zoom_by_keeps_the_middle_point() {
        let mut state = PreviewState::new(0, 1, "a", None);
        state.zoom_to(2.0);
        state.offset = (10.0, -20.0);
        state.zoom_by(2.0);
        assert_eq!(state.zoom, 4.0);
        assert_eq!(state.offset, (20.0, -40.0));
        // Below the fit too: the offset keeps scaling, the image stays where it was dragged.
        state.zoom_by(0.1);
        assert!((state.zoom - 0.4).abs() < 1e-6);
        assert!((state.offset.0 - 2.0).abs() < 1e-5 && (state.offset.1 + 4.0).abs() < 1e-5);
        state.reset_view();
        assert_eq!((state.zoom, state.offset), (1.0, (0.0, 0.0)));
    }

    #[test]
    fn zoom_goes_below_the_fit_and_snaps_back_to_it() {
        let mut state = PreviewState::new(0, 1, "a", None);
        state.zoom_by(1.0 / 100.0);
        assert_eq!(state.zoom, MIN_ZOOM);
        let mut wheel = PreviewState::new(0, 1, "a", None);
        for _ in 0..3 {
            wheel.zoom_by(1.0 / WHEEL_ZOOM);
        }
        for _ in 0..3 {
            wheel.zoom_by(WHEEL_ZOOM);
        }
        assert_eq!(wheel.zoom, 1.0);
    }

    #[test]
    fn the_hand_closes_while_dragging() {
        assert_eq!(drag_cursor(false), CursorStyle::OpenHand);
        assert_eq!(drag_cursor(true), CursorStyle::ClosedHand);
    }

    #[test]
    fn the_hand_cursors_are_embedded_cursor_files() {
        // The resources app.rc points at: a .cur (ICONDIR type 2) of one 32x32 image.
        for name in ["grab.cur", "grabbing.cur"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/cursors")
                .join(name);
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(&bytes[..6], &[0, 0, 2, 0, 1, 0], "{name}");
            assert_eq!((bytes[6], bytes[7]), (32, 32), "{name}");
        }
        let rc = include_str!("../../app.rc");
        assert!(rc.contains("101 CURSOR \"assets/cursors/grabbing.cur\""));
        assert!(rc.contains("102 CURSOR \"assets/cursors/grab.cur\""));
    }

    #[test]
    fn wheel_forward_zooms_in_and_back_zooms_out() {
        assert_eq!(wheel_zoom_factor(1.0), Some(WHEEL_ZOOM));
        assert_eq!(wheel_zoom_factor(-1.0), Some(1.0 / WHEEL_ZOOM));
        assert_eq!(wheel_zoom_factor(0.0), None);
    }

    fn animation(frames: usize) -> Shown {
        let frames: Vec<image::Frame> = (0..frames)
            .map(|_| {
                image::Frame::from_parts(
                    image::RgbaImage::new(4, 2),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(50, 1),
                )
            })
            .collect();
        Shown::Animation(Arc::new(RenderImage::new(frames)))
    }

    #[test]
    fn an_animation_gets_a_player_and_a_still_does_not() {
        let now = Instant::now();
        let mut state = PreviewState::new(0, 1, "a.gif", Some(animation(3)));
        let player = state.player.as_ref().expect("a player");
        assert_eq!(player.frame_count(), 3);
        assert_eq!(player.duration(), Duration::from_millis(150));
        assert_eq!(natural_size(&state), Some((4.0, 2.0)));

        // The same GIF turned keeps where it was.
        state.player.as_mut().unwrap().step(1);
        state.show(Some(animation(3)), now);
        assert_eq!(state.player.as_ref().unwrap().frame(), 1);

        // Another GIF starts over; a still has no player.
        state.show(Some(animation(5)), now);
        assert_eq!(state.player.as_ref().unwrap().frame(), 0);
        let still = Shown::Pixels(Arc::new(RenderImage::new(vec![image::Frame::new(
            image::RgbaImage::new(1, 1),
        )])));
        state.show(Some(still), now);
        assert!(state.player.is_none());
    }

    #[test]
    fn speeds_read_as_multipliers() {
        assert_eq!(speed_label(1.0), "1×");
        assert_eq!(speed_label(0.25), "0.25×");
        assert_eq!(speed_label(1.5), "1.5×");
    }

    #[test]
    fn zoom_stops_at_the_limit() {
        let mut state = PreviewState::new(0, 1, "a", None);
        state.zoom_to(2.0);
        state.offset = (5.0, 5.0);
        state.zoom_by(100.0);
        assert_eq!(state.zoom, MAX_ZOOM);
        assert_eq!(state.offset, (20.0, 20.0));
    }
}
