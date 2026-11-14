//! Map product theme prefs onto GPUI Kit (D-129, D-182): the light / dark mode, then the
//! accent over the loaded palette. The accent reaches every copy of the theme through
//! [`KitTheme::update`], so buttons, rings and the selection stay in step.

use app_store::{Accent, Preferences, PreviewBackground, Theme};
use gpui_kit::App;
use gpui_kit::component::Colorize as _;
use gpui_kit::component::theme::{Theme as KitTheme, ThemeColor, ThemeMode};
use gpui_kit::{Hsla, rgb, white};

/// The accent when Windows has none to offer (other platforms, a missing registry value):
/// the default Windows blue.
pub const FALLBACK_ACCENT: [u8; 3] = [0x00, 0x78, 0xd4];

/// Applies the stored preferences. `System` follows the OS appearance.
pub fn apply_theme(prefs: &Preferences, cx: &mut App) {
    match prefs.theme {
        Theme::System => KitTheme::sync_system_appearance(None, cx),
        Theme::Light => KitTheme::change(ThemeMode::Light, None, cx),
        Theme::Dark => KitTheme::change(ThemeMode::Dark, None, cx),
    }
    let dark = KitTheme::global(cx).is_dark();
    let accent = accent_rgb(prefs.accent.clone());
    KitTheme::update(cx, |theme| apply_accent(&mut theme.colors, accent, dark));
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
