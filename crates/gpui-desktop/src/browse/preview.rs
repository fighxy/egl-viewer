//! Full-size preview overlay: fit / zoom / pan / rotate / prev-next (Phase 2).

use std::sync::Arc;
use std::time::{Duration, Instant};

use app_store::Locale;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::{Disableable as _, StyledExt as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Entity, Image, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString,
    Styled, StyledImage as _, black, div, img, px, white,
};

use crate::app::RootApp;
use crate::browse::space_preview::{self, SpaceDown};
use crate::i18n::t;
use crate::platform;

const MAX_ZOOM: f32 = 8.0;

#[derive(Clone)]
pub struct PreviewState {
    pub index: usize,
    pub total: u32,
    pub name: SharedString,
    pub image: Option<Arc<Image>>,
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
        image: Option<Arc<Image>>,
    ) -> Self {
        Self {
            index,
            total,
            name: name.into(),
            image,
            zoom: 1.0,
            offset: (0.0, 0.0),
            rotation: 0,
            drag_origin: None,
            appear: if platform::reduced_motion() { 1.0 } else { 0.0 },
            opened_at: Instant::now(),
        }
    }

    pub fn zoom_to(&mut self, next: f32) {
        let clamped = next.clamp(1.0, MAX_ZOOM);
        self.zoom = clamped;
        if (clamped - 1.0).abs() < f32::EPSILON {
            self.offset = (0.0, 0.0);
        }
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

pub fn render_preview(
    state: &PreviewState,
    locale: Locale,
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
        .flex()
        .flex_col()
        .bg(black().opacity(overlay_alpha))
        .text_color(white())
        .child(
            h_flex()
                .id("preview-chrome")
                .h(px(44.))
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
                .child(div().flex_1())
                .child(
                    Button::new("preview-zoom-out")
                        .ghost()
                        .label("−")
                        .disabled(state.zoom <= 1.0)
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
                .child(
                    Button::new("preview-close")
                        .ghost()
                        .label("×")
                        .on_click(move |_, _, cx| {
                            close_entity.update(cx, |this, cx| this.close_preview(cx));
                        }),
                ),
        )
        .child(
            div()
                .id("preview-stage")
                .relative()
                .flex_1()
                .overflow_hidden()
                .child(
                    // Letterbox dismiss layer behind the image (D-073).
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
                    div()
                        .id("preview-stage-inner")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_scroll(move |event, _, cx| {
                            let delta = event.delta.pixel_delta(px(1.)).y.as_f32();
                            let factor = if delta < 0.0 { 1.1 } else { 1.0 / 1.1 };
                            wheel_entity.update(cx, |this, cx| this.preview_zoom(factor, cx));
                        })
                        .child(
                            div()
                                .id("preview-image-wrap")
                                .relative()
                                .opacity(appear)
                                .when(zoom > 1.0 || ox != 0.0 || oy != 0.0, |el| {
                                    el.left(px(ox)).top(px(oy))
                                })
                                .child(match &state.image {
                                    Some(image) => {
                                        let pan = pan_entity.clone();
                                        let w = (920.0 * zoom).max(120.0);
                                        let h = (720.0 * zoom).max(120.0);
                                        // After 90/270 the fit box swaps (same idea as React).
                                        let (w, h) = if rotation % 180 != 0 {
                                            (h, w)
                                        } else {
                                            (w, h)
                                        };
                                        img(image.clone())
                                            .id("preview-image")
                                            .object_fit(gpui_kit::ObjectFit::Contain)
                                            .w(px(w))
                                            .h(px(h))
                                            .when(zoom > 1.0, |el| el.cursor_grab())
                                            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                                if zoom <= 1.0 {
                                                    return;
                                                }
                                                let pos = event.position;
                                                pan.update(cx, |this, cx| {
                                                    this.preview_pan_start(
                                                        pos.x.as_f32() - ox,
                                                        pos.y.as_f32() - oy,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .on_mouse_up(MouseButton::Left, {
                                                let pan = pan_entity.clone();
                                                move |_, _, cx| {
                                                    pan.update(cx, |this, cx| {
                                                        this.preview_pan_end(cx)
                                                    });
                                                }
                                            })
                                            .on_mouse_move({
                                                let pan = pan_entity.clone();
                                                move |event, _, cx| {
                                                    let pos = event.position;
                                                    pan.update(cx, |this, cx| {
                                                        this.preview_pan_move(
                                                            pos.x.as_f32(),
                                                            pos.y.as_f32(),
                                                            cx,
                                                        );
                                                    });
                                                }
                                            })
                                            .into_any_element()
                                    }
                                    None => Label::new(t(locale, "preview.unavailable"))
                                        .text_lg()
                                        .into_any_element(),
                                }),
                        ),
                )
                .child(
                    Button::new("preview-prev")
                        .absolute()
                        .left_3()
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
                        .ghost()
                        .label("›")
                        .disabled(!can_next)
                        .on_click(move |_, _, cx| {
                            next_entity.update(cx, |this, cx| this.preview_step(1, cx));
                        }),
                ),
        )
}
