//! The player bar of a video or audio item in the full-size preview (D-170): play / pause,
//! frame steps (video), a seek track to click or drag along, the time, mute and volume, the
//! speed. Same look as the GIF bar (D-154): white on a dark rounded bar at the bottom of the
//! stage, fixed widths so the track stays under a dragging pointer.

use app_store::Locale;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::{StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, CursorStyle, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled, black, div, px, white,
};

use crate::app::RootApp;
use crate::i18n::t;
use crate::ops_media::MediaPreview;

/// Segments of the seek track: enough for a smooth drag, few enough to stay cheap.
pub const SEEK_SEGMENTS: usize = 240;
/// Segments of the volume track: steps of 10 %.
pub const VOLUME_SEGMENTS: usize = 10;
const CONTROLS_HEIGHT: f32 = 40.0;
const TRACK_WIDTH: f32 = 360.0;

/// Playback speeds, the same steps as the GIF player.
pub const SPEEDS: [f64; 5] = [0.25, 0.5, 1.0, 1.5, 2.0];

/// The next speed after `speed`, wrapping.
pub fn next_speed(speed: f64) -> f64 {
    SPEEDS
        .iter()
        .copied()
        .find(|s| *s > speed + 1e-6)
        .unwrap_or(SPEEDS[0])
}

/// `m:ss`, or `h:mm:ss` from an hour on. Fractions are cut, as players do.
pub fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The time a segment of the seek track stands for: its middle.
pub fn segment_time(segment: usize, duration: f64) -> f64 {
    (segment as f64 + 0.5) / SEEK_SEGMENTS as f64 * duration
}

/// How many segments of the track show as played.
pub fn played_segments(position: f64, duration: Option<f64>) -> usize {
    match duration {
        Some(d) if d > 0.0 => ((position / d) * SEEK_SEGMENTS as f64)
            .round()
            .clamp(0.0, SEEK_SEGMENTS as f64) as usize,
        _ => 0,
    }
}

fn speed_label(speed: f64) -> String {
    format!("{speed}×")
}

fn white_button(id: &'static str) -> Button {
    // White on the dark bar in either theme.
    Button::new(id).ghost().text_color(white())
}

/// The bar at the bottom of the stage.
pub fn media_controls(media: &MediaPreview, locale: Locale, entity: Entity<RootApp>) -> AnyElement {
    let playing = !media.paused;
    let duration = media.duration;
    let reached = played_segments(media.position, duration);
    let video = !media.audio_only;

    let track = h_flex()
        .id("media-track")
        .w(px(TRACK_WIDTH))
        .h(px(20.0))
        .items_center()
        .cursor(CursorStyle::PointingHand)
        .children((0..SEEK_SEGMENTS).map(|segment| {
            let (down, drag, up) = (entity.clone(), entity.clone(), entity.clone());
            div()
                .id(("media-segment", segment))
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .child(div().w_full().h(px(4.0)).bg(if segment < reached {
                    white().opacity(0.9)
                } else {
                    white().opacity(0.25)
                }))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    down.update(cx, |this, cx| this.media_scrub(segment, cx));
                })
                // Dragging along the track scrubs: key frames while moving, the exact frame
                // when the button comes up.
                .on_mouse_move(move |event, _, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        drag.update(cx, |this, cx| this.media_scrub(segment, cx));
                    }
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    up.update(cx, |this, cx| this.media_scrub_end(cx));
                })
        }));

    let volume = h_flex()
        .id("media-volume")
        .w(px(70.0))
        .h(px(20.0))
        .items_center()
        .cursor(CursorStyle::PointingHand)
        .children((0..VOLUME_SEGMENTS).map(|segment| {
            let (set, drag) = (entity.clone(), entity.clone());
            let level = (segment + 1) as f32 / VOLUME_SEGMENTS as f32;
            let lit = !media.muted && media.volume + 1e-3 >= level;
            div()
                .id(("media-volume-step", segment))
                .flex_1()
                .h_full()
                .flex()
                .items_end()
                .pb(px(4.0))
                .child(
                    div()
                        .w_full()
                        .h(px(4.0 + 8.0 * level))
                        .mx(px(1.0))
                        .bg(if lit {
                            white().opacity(0.9)
                        } else {
                            white().opacity(0.25)
                        }),
                )
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    set.update(cx, |this, cx| this.media_set_volume(level, cx));
                })
                .on_mouse_move(move |event, _, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        drag.update(cx, |this, cx| this.media_set_volume(level, cx));
                    }
                })
        }));

    let (toggle, back, forward, mute, speed) = (
        entity.clone(),
        entity.clone(),
        entity.clone(),
        entity.clone(),
        entity.clone(),
    );
    let time = format!(
        "{} / {}",
        format_time(media.position),
        duration.map_or_else(|| "–:––".to_string(), format_time)
    );

    h_flex()
        .id("media-controls")
        .absolute()
        .bottom(px(12.0))
        .left_0()
        .right_0()
        .justify_center()
        .child(
            h_flex()
                .id("media-controls-bar")
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
                .when(video, |bar| {
                    bar.child(
                        white_button("media-prev-frame")
                            .icon(IconName::StepBack)
                            .tooltip(t(locale, "preview.gif.prevFrame"))
                            .on_click(move |_, _, cx| {
                                back.update(cx, |this, cx| this.media_step(-1, cx));
                            }),
                    )
                })
                .child(
                    white_button("media-play")
                        .icon(if playing {
                            IconName::Pause
                        } else {
                            IconName::Play
                        })
                        .tooltip(t(
                            locale,
                            if playing {
                                "preview.media.pause"
                            } else {
                                "preview.media.play"
                            },
                        ))
                        .on_click(move |_, _, cx| {
                            toggle.update(cx, |this, cx| this.media_toggle(cx));
                        }),
                )
                .when(video, |bar| {
                    bar.child(
                        white_button("media-next-frame")
                            .icon(IconName::StepForward)
                            .tooltip(t(locale, "preview.gif.nextFrame"))
                            .on_click(move |_, _, cx| {
                                forward.update(cx, |this, cx| this.media_step(1, cx));
                            }),
                    )
                })
                .child(div().px_2().child(track))
                .child(
                    h_flex()
                        .w(px(96.0))
                        .justify_end()
                        .child(Label::new(time).text_xs().text_color(white().opacity(0.8))),
                )
                .child(
                    white_button("media-mute")
                        .icon(if media.muted || media.volume <= 0.0 {
                            IconName::VolumeX
                        } else if media.volume < 0.5 {
                            IconName::Volume1
                        } else {
                            IconName::Volume2
                        })
                        .tooltip(t(locale, "preview.media.mute"))
                        .on_click(move |_, _, cx| {
                            mute.update(cx, |this, cx| this.media_toggle_mute(cx));
                        }),
                )
                .child(volume)
                .child(
                    white_button("media-speed")
                        .w(px(56.0))
                        .label(speed_label(media.speed))
                        .tooltip(t(locale, "preview.gif.speed"))
                        .on_click(move |_, _, cx| {
                            speed.update(cx, |this, cx| this.media_cycle_speed(cx));
                        }),
                ),
        )
        .into_any_element()
}

/// What an audio item shows on the stage: a note, its name and its length.
pub fn audio_card(name: &str, media: &MediaPreview, locale: Locale) -> AnyElement {
    v_flex()
        .id("media-audio-card")
        .items_center()
        .gap_3()
        .child(
            div()
                .size(px(160.0))
                .rounded(px(16.0))
                .bg(white().opacity(0.08))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    gpui_kit::component::Icon::new(IconName::Music)
                        .size(px(72.0))
                        .text_color(white().opacity(0.8)),
                ),
        )
        .child(Label::new(name.to_string()).font_semibold())
        .child(
            Label::new(if media.failed {
                t(locale, "preview.media.failed")
            } else {
                media.duration.map_or_else(String::new, format_time)
            })
            .text_sm()
            .text_color(white().opacity(0.6)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_like_a_player() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(9.99), "0:09");
        assert_eq!(format_time(75.0), "1:15");
        assert_eq!(format_time(3600.0 + 61.0), "1:01:01");
        assert_eq!(format_time(-3.0), "0:00");
    }

    #[test]
    fn the_track_maps_segments_to_times_and_back() {
        let duration = 120.0;
        assert!((segment_time(0, duration) - 0.25).abs() < 1e-9);
        assert!((segment_time(SEEK_SEGMENTS - 1, duration) - 119.75).abs() < 1e-9);
        assert_eq!(played_segments(0.0, Some(duration)), 0);
        assert_eq!(played_segments(60.0, Some(duration)), SEEK_SEGMENTS / 2);
        assert_eq!(played_segments(500.0, Some(duration)), SEEK_SEGMENTS);
        assert_eq!(played_segments(10.0, None), 0);
    }

    #[test]
    fn speeds_cycle_through_the_steps() {
        assert_eq!(next_speed(1.0), 1.5);
        assert_eq!(next_speed(2.0), 0.25);
        assert_eq!(next_speed(0.25), 0.5);
        assert_eq!(speed_label(0.5), "0.5×");
    }
}
