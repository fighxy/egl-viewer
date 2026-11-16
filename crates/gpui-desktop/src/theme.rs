//! Map product theme prefs onto GPUI Kit (D-129, D-182): the light / dark mode, then the
//! accent over the loaded palette. The accent reaches every copy of the theme through
//! [`KitTheme::update`], so buttons, rings and the selection stay in step.

use std::rc::Rc;

use app_store::{Accent, Palette, Preferences, PreviewBackground, Theme};
use gpui_kit::App;
use gpui_kit::component::theme::{
    Theme as KitTheme, ThemeColor, ThemeConfig, ThemeMode, ThemeRegistry,
};
use gpui_kit::{Hsla, rgb};

/// The accent when Windows has none to offer (other platforms, a missing registry value):
/// the default Windows blue.
pub const FALLBACK_ACCENT: [u8; 3] = [0x00, 0x78, 0xd4];

const BUILTIN_JSON: [&str; 4] = [
    include_str!("../assets/themes/graphite.json"),
    include_str!("../assets/themes/oled-black.json"),
    include_str!("../assets/themes/warm.json"),
    include_str!("../assets/themes/high-contrast.json"),
];

// Keep embedded themes off the kit registry's observer, which would reload over the accent.
struct BuiltinThemes(ThemeRegistry);
impl gpui_kit::Global for BuiltinThemes {}

fn builtin_registry() -> ThemeRegistry {
    let mut registry = ThemeRegistry::default();
    for json in BUILTIN_JSON {
        registry
            .load_themes_from_str(json)
            .expect("valid embedded theme");
    }
    registry
}

pub fn init_themes(cx: &mut App) {
    cx.set_global(BuiltinThemes(builtin_registry()));
}

pub fn palette_label_key(palette: Palette) -> &'static str {
    match palette {
        Palette::Default => "palette.default",
        Palette::Graphite => "palette.graphite",
        Palette::OledBlack => "palette.oledBlack",
        Palette::Warm => "palette.warm",
    }
}

fn palette_names(palette: Palette, contrast: bool) -> Option<(&'static str, &'static str)> {
    if contrast {
        return Some(("High Contrast Light", "High Contrast Dark"));
    }
    match palette {
        Palette::Default => None,
        Palette::Graphite => Some(("Graphite Light", "Graphite Dark")),
        Palette::OledBlack => Some(("OLED Black Light", "OLED Black Dark")),
        Palette::Warm => Some(("Warm Light", "Warm Dark")),
    }
}

fn palette_pair(palette: Palette, contrast: bool, cx: &App) -> (Rc<ThemeConfig>, Rc<ThemeConfig>) {
    let Some((light, dark)) = palette_names(palette, contrast) else {
        let registry = ThemeRegistry::global(cx);
        return (
            registry.default_light_theme().clone(),
            registry.default_dark_theme().clone(),
        );
    };
    let themes = cx.global::<BuiltinThemes>().0.themes();
    (themes[light].clone(), themes[dark].clone())
}

pub fn palette_background(palette: Palette, dark: bool, cx: &App) -> Hsla {
    let (light, night) = palette_pair(palette, false, cx);
    let config = if dark { night } else { light };
    gpui_kit::component::theme::try_parse_color(
        config
            .colors
            .background
            .as_deref()
            .expect("theme background"),
    )
    .expect("valid theme background")
}

#[cfg(windows)]
fn high_contrast_setting() -> windows::core::Result<bool> {
    windows::UI::ViewManagement::AccessibilitySettings::new()?.HighContrast()
}

pub fn high_contrast_enabled() -> bool {
    #[cfg(windows)]
    {
        high_contrast_setting().unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn system_state(cx: &App) -> (bool, ThemeMode, [u8; 3]) {
    (
        high_contrast_enabled(),
        cx.window_appearance().into(),
        accent_rgb(Accent::Windows),
    )
}

pub fn is_high_contrast(theme: &KitTheme) -> bool {
    let config = if theme.is_dark() {
        &theme.dark_theme
    } else {
        &theme.light_theme
    };
    matches!(
        config.name.as_ref(),
        "High Contrast Light" | "High Contrast Dark"
    )
}

/// Applies the stored preferences. `System` follows the OS appearance.
pub fn apply_theme(prefs: &Preferences, cx: &mut App) {
    apply_theme_with_contrast(prefs, high_contrast_enabled(), cx);
}

fn apply_theme_with_contrast(prefs: &Preferences, contrast: bool, cx: &mut App) {
    let (light, dark) = palette_pair(prefs.palette, contrast, cx);
    KitTheme::update(cx, |theme| {
        theme.light_theme = light;
        theme.dark_theme = dark;
    });
    match prefs.theme {
        Theme::System => KitTheme::sync_system_appearance(None, cx),
        Theme::Light => KitTheme::change(ThemeMode::Light, None, cx),
        Theme::Dark => KitTheme::change(ThemeMode::Dark, None, cx),
    }
    let dark = KitTheme::global(cx).is_dark();
    let accent = accent_rgb(prefs.accent.clone());
    KitTheme::update(cx, |theme| {
        theme.shadow = !contrast;
        if !contrast {
            apply_accent(&mut theme.colors, accent, dark);
        }
    });
}

/// The accent's colour: Windows' own when asked and available, the preset or custom colour
/// otherwise, the fallback when a custom value is not hex.
pub fn accent_rgb(accent: Accent) -> [u8; 3] {
    match accent {
        Accent::Windows => windows_accent().unwrap_or(FALLBACK_ACCENT),
        other => other.rgb().unwrap_or(FALLBACK_ACCENT),
    }
}

/// `HKCU\Software\Microsoft\Windows\DWM\AccentColor` as `[r, g, b]`. The DWORD is ABGR.
#[cfg(windows)]
fn windows_accent() -> Option<[u8; 3]> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let dwm = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\DWM")
        .ok()?;
    let value: u32 = dwm.get_value("AccentColor").ok()?;
    Some(dword_to_rgb(value))
}

#[cfg(not(windows))]
fn windows_accent() -> Option<[u8; 3]> {
    None
}

/// The DWM `AccentColor` DWORD is `0xAABBGGRR`.
pub fn dword_to_rgb(value: u32) -> [u8; 3] {
    [
        (value & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        ((value >> 16) & 0xff) as u8,
    ]
}

/// Paints the accent over a loaded palette: the primary family (buttons read their own
/// `button_primary` tokens), the focus ring, the input selection, the left bar's and the
/// grid's selected row, and the drag tint. Text on the accent takes the contrast foreground.
pub fn apply_accent(colors: &mut ThemeColor, accent: [u8; 3], dark: bool) {
    let base = hsla_of(accent);
    let states = accent_states(accent);
    let rest = hsla_of(states.rest);
    let hover = hsla_of(states.hover);
    let active = hsla_of(states.active);
    let foreground = hsla_of(states.foreground);

    colors.primary = rest;
    colors.primary_hover = hover;
    colors.primary_active = active;
    colors.primary_foreground = foreground;
    colors.button_primary = rest;
    colors.button_primary_hover = hover;
    colors.button_primary_active = active;
    colors.button_primary_foreground = foreground;
    colors.ring = base;
    colors.selection = base;
    colors.list_active = base.opacity(if dark { 0.25 } else { 0.14 });
    colors.list_active_border = base;
    colors.sidebar_primary = rest;
    colors.sidebar_primary_foreground = foreground;
    colors.drag_border = base;
    colors.drop_target = base.opacity(0.25);
    colors.slider_bar = base;
    colors.progress_bar = base;
}

/// How far hover raises the fill's OKLab lightness, at most and at least (D-186).
const HOVER_LIFT: [f32; 2] = [0.06, 0.035];
/// How far a press lowers it, at most and at least.
const PRESS_SINK: [f32; 2] = [0.06, 0.02];
/// The step by which the resting fill is darkened when hover cannot lift it readably.
const REST_STEP: f32 = 0.01;

/// The fills of a control painted with the accent, and the text on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccentStates {
    pub rest: [u8; 3],
    /// Lighter than `rest`: the control lights up under the pointer.
    pub hover: [u8; 3],
    /// Darker than `rest` (or equal, for a pale accent with no room): it sinks when pressed.
    pub active: [u8; 3],
    pub foreground: [u8; 3],
}

fn srgb_to_linear(channel: u8) -> f32 {
    let c = channel as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> u8 {
    let c = value.clamp(0.0, 1.0);
    let c = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round() as u8
}

/// sRGB to OKLab (Björn Ottosson's matrices).
fn oklab(rgb: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(srgb_to_linear);
    let l = (0.412_221_5 * r + 0.536_332_5 * g + 0.051_445_99 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn from_oklab([lightness, a, b]: [f32; 3]) -> [u8; 3] {
    let l = (lightness + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (lightness - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (lightness - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
    .map(linear_to_srgb)
}

/// The colour with its OKLab lightness moved by `delta` and its hue and chroma kept, so a lit
/// accent stays as saturated as the accent instead of washing out towards white.
fn shift_lightness(rgb: [u8; 3], delta: f32) -> [u8; 3] {
    let [lightness, a, b] = oklab(rgb);
    from_oklab([(lightness + delta).clamp(0.0, 1.0), a, b])
}

/// The first shift, from `range[0]` down to `range[1]` by 0.005 (negated when `darker`), that
/// changes `rest` and keeps `foreground` readable on it.
fn readable_shift(
    rest: [u8; 3],
    darker: bool,
    range: [f32; 2],
    foreground: [u8; 3],
) -> Option<[u8; 3]> {
    let steps = ((range[0] - range[1]) / 0.005).round() as i32;
    let sign = if darker { -1.0 } else { 1.0 };
    (0..=steps)
        .map(|step| shift_lightness(rest, sign * (range[0] - step as f32 * 0.005)))
        .find(|shifted| *shifted != rest && contrast_ratio(*shifted, foreground) >= 4.5)
}

/// Hover lifts the accent, a press sinks it, and the text stays at WCAG AA (4.5:1) in every
/// state (D-186). An accent with no room to lift readably (Windows' own blue is right at 4.5
/// with white text) rests a little darker, so that hover comes back to about the accent itself.
pub fn accent_states(accent: [u8; 3]) -> AccentStates {
    for step in 0..=40 {
        let rest = shift_lightness(accent, -(step as f32) * REST_STEP);
        let foreground = contrast_foreground(rest);
        let Some(hover) = readable_shift(rest, false, HOVER_LIFT, foreground) else {
            continue;
        };
        let active = readable_shift(rest, true, PRESS_SINK, foreground).unwrap_or(rest);
        return AccentStates {
            rest,
            hover,
            active,
            foreground,
        };
    }
    // Unreachable for real colours: 0.4 darker in lightness always leaves room. Stay readable.
    let foreground = contrast_foreground(accent);
    AccentStates {
        rest: accent,
        hover: accent,
        active: accent,
        foreground,
    }
}

/// The foreground on `background`: white when it reaches WCAG AA (4.5:1), black otherwise.
/// White is preferred because Windows paints white on its own accent; when white fails,
/// black always passes (the two curves cross above 4.5, near luminance 0.18).
pub fn contrast_foreground(background: [u8; 3]) -> [u8; 3] {
    if contrast_ratio(background, [255, 255, 255]) >= 4.5 {
        [255, 255, 255]
    } else {
        [0, 0, 0]
    }
}

/// The WCAG 2.x contrast ratio of two colours, from 1 to 21.
pub fn contrast_ratio(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// WCAG relative luminance: sRGB channels linearised, then weighted.
fn luminance(rgb: [u8; 3]) -> f32 {
    let linear = |channel: u8| {
        let c = f32::from(channel) / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgb[0]) + 0.7152 * linear(rgb[1]) + 0.0722 * linear(rgb[2])
}

pub fn hsla_of(channels: [u8; 3]) -> Hsla {
    rgb(u32::from(channels[0]) << 16 | u32::from(channels[1]) << 8 | u32::from(channels[2])).into()
}

pub fn preview_colors(background: PreviewBackground, theme: &KitTheme) -> (Hsla, Hsla) {
    if is_high_contrast(theme) {
        return (theme.background, theme.foreground);
    }
    match background.rgb() {
        Some(rgb) => (hsla_of(rgb), hsla_of(contrast_foreground(rgb))),
        None => (theme.background, theme.foreground),
    }
}

pub fn preview_background_key(background: PreviewBackground) -> &'static str {
    match background {
        PreviewBackground::Theme => "previewBg.theme",
        PreviewBackground::Black => "previewBg.black",
        PreviewBackground::Gray18 => "previewBg.gray",
        PreviewBackground::White => "previewBg.white",
    }
}

pub fn theme_label_key(theme: Theme) -> &'static str {
    match theme {
        Theme::System => "themes.system",
        Theme::Light => "themes.light",
        Theme::Dark => "themes.dark",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_pairs_parse_and_define_every_colour_read_by_the_app() {
        use gpui_kit::component::theme::{ThemeSet, try_parse_color};
        let required = [
            "accent.background",
            "background",
            "base.blue",
            "base.yellow",
            "border",
            "danger.background",
            "foreground",
            "list.active.background",
            "list.hover.background",
            "muted.background",
            "muted.foreground",
            "popover.background",
            "primary.background",
            "primary.foreground",
            "primary.hover.background",
            "ring",
            "secondary.background",
            "sidebar.background",
            "success.background",
        ];
        let registry = builtin_registry();
        assert_eq!(registry.themes().len(), 8);
        for json in BUILTIN_JSON {
            let raw: serde_json::Value = serde_json::from_str(json).unwrap();
            let set: ThemeSet = serde_json::from_str(json).unwrap();
            assert_eq!(set.themes.len(), 2);
            assert_eq!(set.themes[0].mode, ThemeMode::Light);
            assert_eq!(set.themes[1].mode, ThemeMode::Dark);
            for (index, config) in set.themes.into_iter().enumerate() {
                let colors = serde_json::to_value(&config.colors).unwrap();
                for key in required {
                    assert!(colors[key].is_string(), "{}: missing {key}", config.name);
                }
                for (key, value) in raw["themes"][index]["colors"].as_object().unwrap() {
                    assert_eq!(&colors[key], value, "{}: unrecognised {key}", config.name);
                    assert!(try_parse_color(value.as_str().unwrap()).is_ok(), "{key}");
                }
                let config = registry.themes()[config.name.as_ref()].clone();
                let mut theme = KitTheme::default();
                theme.apply_config(&config);
                assert_eq!(
                    theme.background,
                    try_parse_color(colors["background"].as_str().unwrap()).unwrap()
                );
                assert_eq!(
                    theme.foreground,
                    try_parse_color(colors["foreground"].as_str().unwrap()).unwrap()
                );
                assert!(
                    contrast_ratio(
                        crate::ops_colour::rgb_of(theme.background),
                        crate::ops_colour::rgb_of(theme.foreground)
                    ) >= 4.5
                );
            }
        }
        let oled = &registry.themes()["OLED Black Dark"];
        let mut theme = KitTheme::default();
        theme.apply_config(oled);
        assert_eq!(theme.background, gpui_kit::black());
        assert_eq!(theme.sidebar, gpui_kit::black());
    }

    #[test]
    fn contrast_overrides_every_palette_and_releases_it_without_changing_preferences() {
        let registry = builtin_registry();
        for palette in Palette::ALL {
            let prefs = Preferences {
                palette,
                accent: Accent::Custom("#123456".into()),
                preview_background: PreviewBackground::White,
                ..Default::default()
            };
            let original = prefs.clone();
            let normal = palette_names(prefs.palette, false);
            assert_eq!(
                palette_names(prefs.palette, true),
                Some(("High Contrast Light", "High Contrast Dark"))
            );
            for name in ["High Contrast Light", "High Contrast Dark"] {
                let mut theme = KitTheme::default();
                theme.apply_config(&registry.themes()[name]);
                assert!(is_high_contrast(&theme));
                for background in PreviewBackground::ALL {
                    assert_eq!(
                        preview_colors(background, &theme),
                        (theme.background, theme.foreground)
                    );
                }
                let rgb = crate::ops_colour::rgb_of;
                for surface in [
                    theme.background,
                    theme.sidebar,
                    theme.popover,
                    theme.list_active,
                    theme.list_hover,
                ] {
                    assert!(contrast_ratio(rgb(surface), rgb(theme.foreground)) >= 7.0);
                }
                assert!(contrast_ratio(rgb(theme.primary), rgb(theme.primary_foreground)) >= 7.0);
                assert!(contrast_ratio(rgb(theme.yellow), rgb(theme.background)) >= 7.0);
            }
            assert_eq!(palette_names(prefs.palette, false), normal);
            assert_eq!(prefs, original);
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_contrast_api_is_available_without_unsafe() {
        assert!(high_contrast_setting().is_ok());
    }

    #[test]
    fn the_dwm_dword_is_abgr() {
        // Windows' default blue #0078d4 is stored as 0xffd47800.
        assert_eq!(dword_to_rgb(0xffd4_7800), [0x00, 0x78, 0xd4]);
        assert_eq!(dword_to_rgb(0xff00_00ff), [0xff, 0x00, 0x00]);
        assert_eq!(dword_to_rgb(0xff00_ff00), [0x00, 0xff, 0x00]);
    }

    #[test]
    fn presets_and_custom_resolve_windows_falls_back() {
        assert_eq!(accent_rgb(Accent::Blue), [0x3b, 0x82, 0xf6]);
        assert_eq!(
            accent_rgb(Accent::Custom("#0f766e".into())),
            [0x0f, 0x76, 0x6e]
        );
        assert_eq!(
            accent_rgb(Accent::Custom("garbage".into())),
            FALLBACK_ACCENT
        );
        // Windows resolves to the OS accent or the fallback, never to nothing.
        let _ = accent_rgb(Accent::Windows);
    }

    #[test]
    fn known_contrasts() {
        assert_eq!(contrast_foreground([255, 255, 255]), [0, 0, 0]);
        assert_eq!(contrast_foreground([0, 0, 0]), [255, 255, 255]);
        assert_eq!(contrast_foreground([0x00, 0x78, 0xd4]), [255, 255, 255]);
        assert_eq!(contrast_foreground([0xf5, 0xb3, 0x01]), [0, 0, 0]);
        assert!((contrast_ratio([0, 0, 0], [255, 255, 255]) - 21.).abs() < 0.01);
        assert!((contrast_ratio([128, 128, 128], [128, 128, 128]) - 1.).abs() < 0.001);
    }

    /// Every colour keeps text readable: the chosen foreground reaches WCAG AA (4.5:1).
    #[test]
    fn the_contrast_foreground_always_reaches_wcag_aa() {
        let mut colours = vec![];
        for gray in 0..=255u8 {
            colours.push([gray, gray, gray]);
        }
        for r in (0..=255u8).step_by(17) {
            for g in (0..=255u8).step_by(17) {
                for b in (0..=255u8).step_by(17) {
                    colours.push([r, g, b]);
                }
            }
        }
        for background in colours {
            let foreground = contrast_foreground(background);
            let ratio = contrast_ratio(background, foreground);
            assert!(ratio >= 4.5, "{background:?} -> {foreground:?}: {ratio}");
            let mut colors = ThemeColor::default();
            apply_accent(&mut colors, background, true);
            let text = crate::ops_colour::rgb_of(colors.primary_foreground);
            for state in [colors.primary, colors.primary_hover, colors.primary_active] {
                let state = crate::ops_colour::rgb_of(state);
                assert!(
                    contrast_ratio(state, text) >= 4.5,
                    "{background:?} -> {state:?}"
                );
            }
        }
    }

    fn test_accents() -> Vec<[u8; 3]> {
        let mut colours: Vec<[u8; 3]> = (0..=255u8).map(|gray| [gray, gray, gray]).collect();
        for r in (0..=255u8).step_by(17) {
            for g in (0..=255u8).step_by(17) {
                for b in (0..=255u8).step_by(17) {
                    colours.push([r, g, b]);
                }
            }
        }
        colours
    }

    /// D-186: the control lights up under the pointer and sinks when pressed, for every
    /// accent, and its text stays readable in all three states.
    #[test]
    fn hover_lifts_and_press_sinks_while_text_stays_readable() {
        for accent in test_accents() {
            let s = accent_states(accent);
            assert!(
                luminance(s.hover) > luminance(s.rest),
                "{accent:?}: hover {:?} is not lighter than {:?}",
                s.hover,
                s.rest
            );
            assert!(
                luminance(s.active) <= luminance(s.rest),
                "{accent:?}: press {:?} is lighter than {:?}",
                s.active,
                s.rest
            );
            for state in [s.rest, s.hover, s.active] {
                assert!(
                    contrast_ratio(state, s.foreground) >= 4.5,
                    "{accent:?}: {state:?} on {:?}",
                    s.foreground
                );
            }
            // A dark accent never rests darker than it has to.
            assert!(luminance(s.rest) <= luminance(accent) + 1e-6, "{accent:?}");
        }
    }

    /// Windows' default blue sits right at 4.5:1 with white text: it rests a little darker,
    /// hover comes back to about the accent, and a press sinks below the resting fill.
    #[test]
    fn windows_blue_rests_slightly_darker_and_hover_returns_to_it() {
        let accent = [0x00, 0x78, 0xd4];
        let s = accent_states(accent);
        assert_eq!(s.foreground, [255, 255, 255]);
        assert_ne!(s.rest, accent);
        // Perceptual distance in OKLab; 0.02 is about a just-noticeable difference.
        let distance = |a: [u8; 3], b: [u8; 3]| {
            let (a, b) = (oklab(a), oklab(b));
            a.iter()
                .zip(b)
                .map(|(x, y)| (x - y).powi(2))
                .sum::<f32>()
                .sqrt()
        };
        assert!(distance(s.rest, accent) <= 0.06, "rest {:?}", s.rest);
        assert!(distance(s.hover, accent) <= 0.03, "hover {:?}", s.hover);
        assert!(luminance(s.active) < luminance(s.rest));
    }

    /// The OKLab round trip is lossless to a step, and a lightness shift keeps the hue.
    #[test]
    fn lightness_shifts_keep_the_colour() {
        for rgb in [
            [0, 120, 212],
            [239, 68, 68],
            [234, 179, 8],
            [0, 0, 0],
            [255, 255, 255],
        ] {
            let back = from_oklab(oklab(rgb));
            for (x, y) in rgb.iter().zip(back) {
                assert!((*x as i32 - y as i32).abs() <= 1, "{rgb:?} -> {back:?}");
            }
        }
        let lit = shift_lightness([0, 120, 212], 0.05);
        assert!(luminance(lit) > luminance([0, 120, 212]));
        // Blue stays the dominant channel and red stays low: no wash towards white.
        assert!(lit[2] > lit[1] && lit[1] > lit[0] && lit[0] < 40, "{lit:?}");
    }

    #[test]
    fn preview_backgrounds_are_independent_and_readable() {
        let mut theme = KitTheme::default();
        for (background, foreground) in [(0xffffff, 0x202020), (0x202020, 0xffffff)] {
            theme.colors.background = rgb(background).into();
            theme.colors.foreground = rgb(foreground).into();
            assert_eq!(
                preview_colors(PreviewBackground::Theme, &theme),
                (theme.background, theme.foreground)
            );
            for preset in [
                PreviewBackground::Black,
                PreviewBackground::Gray18,
                PreviewBackground::White,
            ] {
                let (bg, fg) = preview_colors(preset, &theme);
                assert_eq!(bg, hsla_of(preset.rgb().unwrap()));
                assert!(
                    contrast_ratio(crate::ops_colour::rgb_of(bg), crate::ops_colour::rgb_of(fg))
                        >= 4.5
                );
            }
        }
        assert!((luminance(PreviewBackground::Gray18.rgb().unwrap()) - 0.18).abs() < 0.005);
    }

    #[test]
    fn the_accent_reaches_the_selection_and_the_primary_tokens() {
        let mut colors = ThemeColor::default();
        apply_accent(&mut colors, [0xef, 0x44, 0x44], true);
        let base = hsla_of([0xef, 0x44, 0x44]);
        assert_eq!(colors.primary, base);
        assert_eq!(colors.button_primary, base);
        assert_eq!(colors.ring, base);
        assert_eq!(colors.selection, base);
        assert_eq!(colors.list_active_border, base);
        // Red is too light for white text under WCAG: it takes black.
        assert_eq!(colors.primary_foreground, hsla_of([0, 0, 0]));
        assert!(colors.list_active.a < 1.);
        // A light theme tints the selected row more quietly than a dark one.
        let mut light = ThemeColor::default();
        apply_accent(&mut light, [0xef, 0x44, 0x44], false);
        assert!(light.list_active.a < colors.list_active.a);
        assert_eq!(light.primary_foreground, hsla_of([0, 0, 0]));
        // A pale accent takes dark text instead.
        let mut pale = ThemeColor::default();
        apply_accent(&mut pale, [0xea, 0xb3, 0x08], false);
        assert_eq!(pale.primary_foreground, hsla_of([0, 0, 0]));
    }
}
