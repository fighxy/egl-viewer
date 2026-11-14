//! Map product theme prefs onto GPUI Kit (D-129, D-182): the light / dark mode, then the
//! accent over the loaded palette. The accent reaches every copy of the theme through
//! [`KitTheme::update`], so buttons, rings and the selection stay in step.

use std::rc::Rc;

use app_store::{Accent, Palette, Preferences, PreviewBackground, Theme};
use gpui_kit::App;
use gpui_kit::component::Colorize as _;
use gpui_kit::component::theme::{
    Theme as KitTheme, ThemeColor, ThemeConfig, ThemeMode, ThemeRegistry,
};
use gpui_kit::{Hsla, rgb, white};

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
    let foreground = hsla_of(contrast_foreground(accent));
    // Move away from the text so hover and active cannot reduce its contrast.
    let towards = if foreground == white() {
        gpui_kit::black()
    } else {
        white()
    };
    let hover = base.mix_oklab(towards, 0.12);
    let active = base.mix_oklab(towards, 0.22);

    colors.primary = base;
    colors.primary_hover = hover;
    colors.primary_active = active;
    colors.primary_foreground = foreground;
    colors.button_primary = base;
    colors.button_primary_hover = hover;
    colors.button_primary_active = active;
    colors.button_primary_foreground = foreground;
    colors.ring = base;
    colors.selection = base;
    colors.list_active = base.opacity(if dark { 0.25 } else { 0.14 });
    colors.list_active_border = base;
    colors.sidebar_primary = base;
    colors.sidebar_primary_foreground = foreground;
    colors.drag_border = base;
    colors.drop_target = base.opacity(0.25);
    colors.slider_bar = base;
    colors.progress_bar = base;
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
            for state in [colors.primary_hover, colors.primary_active] {
                let state = crate::ops_colour::rgb_of(state);
                assert!(
                    contrast_ratio(state, foreground) >= 4.5,
                    "{background:?} -> {state:?}"
                );
            }
        }
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
