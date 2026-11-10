//! Full-size preview overlay: fit / zoom / pan / rotate / prev-next (Phase 2).

use std::time::{Duration, Instant};

use app_store::Locale;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::{Disableable as _, StyledExt as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    CursorStyle, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString,
    Styled, StyledImage as _, black, div, img, px, white,
};

use crate::app::RootApp;
use crate::browse::preview_cache::PreviewKey;
use crate::browse::preview_image::Shown;
use crate::browse::space_preview::{self, SpaceDown};
use crate::i18n::t;
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
        }
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
        Some(Shown::Pixels(image)) => {
            let size = image.size(0);
            Some((size.width.0 as f32, size.height.0 as f32))
        }
        _ => state.natural,
    }
}

pub fn render_preview(
    state: &PreviewState,
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
    let (box_w, box_h) = fit_box(natural_size(state), stage, zoom);

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
                                pan.update(cx, |this, cx| this.preview_pan_end(cx));
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
                        .child(match &state.image {
                            Some(shown) => {
                                let pan = pan_entity.clone();
                                img(shown.source())
                                    .id("preview-image")
                                    .flex_none()
                                    .relative()
                                    .left(px(ox))
                                    .top(px(oy))
                                    .object_fit(gpui_kit::ObjectFit::Contain)
                                    .w(px(box_w))
                                    .h(px(box_h))
                                    .cursor(drag_cursor(dragging))
                                    // Grab and drag at any zoom. A click on the image never
                                    // reaches the letterbox, so it does not close the preview.
                                    .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                        cx.stop_propagation();
                                        let pos = event.position;
                                        pan.update(cx, |this, cx| {
                                            this.preview_pan_start(
                                                pos.x.as_f32() - ox,
                                                pos.y.as_f32() - oy,
                                                cx,
                                            );
                                        });
                                    })
                                    .into_any_element()
                            }
                            None => Label::new(t(locale, "preview.unavailable"))
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
                ),
        )
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
