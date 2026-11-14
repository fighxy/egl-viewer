//! Any colour for the colour filter (D-178): hue, saturation and lightness sliders and a hex
//! field inside the Colour popover, beside the swatches.
//!
//! The controls live in the popover itself rather than in a second popup, so picking a colour
//! never closes the filter. A drag shows its colour as it goes and filters once on release;
//! the hex field filters on Enter.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::{AppContext as _, Context, Entity, Hsla, Subscription, Window, hsla};
use library_core::ColorFilter;
use library_core::query::color::{SIMILAR_SHARE, hex, parse_hex};

use crate::app::RootApp;

/// The custom colour controls of the Colour popover.
pub(crate) struct CustomColour {
    pub hue: Entity<SliderState>,
    pub saturation: Entity<SliderState>,
    pub lightness: Entity<SliderState>,
    pub hex: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

fn unit_slider(default: f32) -> SliderState {
    SliderState::new()
        .min(0.)
        .max(1.)
        .step(0.005)
        .default_value(default)
}

impl CustomColour {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<RootApp>) -> Self {
        let hue = cx.new(|_| unit_slider(0.));
        let saturation = cx.new(|_| unit_slider(0.7));
        let lightness = cx.new(|_| unit_slider(0.5));
        let hex = cx.new(|cx| InputState::new(window, cx).placeholder("#rrggbb"));
        let mut subscriptions: Vec<Subscription> = [&hue, &saturation, &lightness]
            .into_iter()
            .map(|slider| {
                cx.subscribe_in(
                    slider,
                    window,
                    |this, _, event: &SliderEvent, window, cx| match event {
                        // Follow the drag in the preview and the hex field.
                        SliderEvent::Change(_) => this.show_slider_colour(window, cx),
                        // Filter once, when the slider is let go.
                        SliderEvent::Release(_) => {
                            this.show_slider_colour(window, cx);
                            let rgb = this.slider_rgb(cx);
                            this.set_color_filter(Some(this.similar_keeping_share(rgb)), cx);
                        }
                    },
                )
            })
            .collect();
        subscriptions.push(cx.subscribe_in(
            &hex,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let text = input.read(cx).value().to_string();
                    if let Some(rgb) = parse_hex(&text) {
                        this.pick_filter_colour(rgb, window, cx);
                    }
                }
            },
        ));
        Self {
            hue,
            saturation,
            lightness,
            hex,
            _subscriptions: subscriptions,
        }
    }
}

/// `[r, g, b]` of a colour.
pub(crate) fn rgb_of(color: Hsla) -> [u8; 3] {
    let rgba = color.to_rgb();
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    [channel(rgba.r), channel(rgba.g), channel(rgba.b)]
}

/// The colour of `[r, g, b]`.
pub(crate) fn hsla_of(rgb: [u8; 3]) -> Hsla {
    gpui_kit::rgb(u32::from(rgb[0]) << 16 | u32::from(rgb[1]) << 8 | u32::from(rgb[2])).into()
}

impl RootApp {
    /// The colour the three sliders make now.
    pub(crate) fn slider_colour(&self, cx: &gpui_kit::App) -> Hsla {
        let value = |slider: &Entity<SliderState>| slider.read(cx).value().start();
        let custom = &self.custom_colour;
        hsla(
            value(&custom.hue),
            value(&custom.saturation),
            value(&custom.lightness),
            1.,
        )
    }

    fn slider_rgb(&self, cx: &gpui_kit::App) -> [u8; 3] {
        rgb_of(self.slider_colour(cx))
    }

    /// A picked colour, with the share the filter already asks for.
    fn similar_keeping_share(&self, rgb: [u8; 3]) -> ColorFilter {
        let min_share = match self.library.as_ref().and_then(|lib| lib.filters.color) {
            Some(ColorFilter::Similar { min_share, .. }) => min_share,
            _ => SIMILAR_SHARE,
        };
        ColorFilter::Similar { rgb, min_share }
    }

    fn show_slider_colour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = hex(self.slider_rgb(cx));
        self.custom_colour.hex.update(cx, |input, cx| {
            input.set_value(text, window, cx);
        });
        cx.notify();
    }

    /// Puts the sliders and the hex field on `rgb`.
    pub(crate) fn show_custom_colour(
        &mut self,
        rgb: [u8; 3],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let color = hsla_of(rgb);
        let custom = &self.custom_colour;
        for (slider, value) in [
            (&custom.hue, color.h),
            (&custom.saturation, color.s),
            (&custom.lightness, color.l),
        ] {
            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        }
        custom
            .hex
            .update(cx, |input, cx| input.set_value(hex(rgb), window, cx));
        cx.notify();
    }

    /// A swatch or a typed hex: the controls show it and the filter uses it.
    pub(crate) fn pick_filter_colour(
        &mut self,
        rgb: [u8; 3],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_custom_colour(rgb, window, cx);
        let filter = self.similar_keeping_share(rgb);
        self.set_color_filter(Some(filter), cx);
    }

    /// The popover opened: the controls start from the colour being filtered by.
    pub(crate) fn sync_custom_colour(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ColorFilter::Similar { rgb, .. }) =
            self.library.as_ref().and_then(|lib| lib.filters.color)
        {
            self.show_custom_colour(rgb, window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_survive_the_trip_through_hsl() {
        for rgb in [
            [124, 179, 66],
            [0, 0, 0],
            [255, 255, 255],
            [229, 57, 53],
            [33, 33, 33],
            [215, 196, 163],
        ] {
            assert_eq!(rgb_of(hsla_of(rgb)), rgb, "{rgb:?}");
        }
    }
}
