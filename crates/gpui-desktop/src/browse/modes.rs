//! The media modes in the toolbar: All · Photo · Video · Audio (D-171). A mode is the type
//! filter set to a bucket (`image`, `video`, `audio`, docs/architecture.md §16.6), so it works
//! in any place (a folder, a tag, a smart folder, the trash) and the Type menu shows it too.
//! Audio is to get a layout of its own later; until then every mode shows the grid.

use app_store::Locale;
use gpui_kit::assets::IconName;
use gpui_kit::component::theme::Theme;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, px,
};

use crate::app::RootApp;
use crate::i18n::t;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaMode {
    All,
    Photo,
    Video,
    Audio,
}

impl MediaMode {
    pub const ALL: [Self; 4] = [Self::All, Self::Photo, Self::Video, Self::Audio];

    /// The type filter this mode sets.
    pub fn filter(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Photo => Some("image"),
            Self::Video => Some("video"),
            Self::Audio => Some("audio"),
        }
    }

    /// The mode a type filter stands for; a single extension (`png`) is no mode.
    pub fn from_filter(ext: Option<&str>) -> Option<Self> {
        match ext {
            None => Some(Self::All),
            Some("image") => Some(Self::Photo),
            Some("video" | "videos") => Some(Self::Video),
            Some("audio") => Some(Self::Audio),
            Some(_) => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::All => "tb-mode-all",
            Self::Photo => "tb-mode-photo",
            Self::Video => "tb-mode-video",
            Self::Audio => "tb-mode-audio",
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Self::All => "toolbar.mode.all",
            Self::Photo => "toolbar.mode.photo",
            Self::Video => "toolbar.mode.video",
            Self::Audio => "toolbar.mode.audio",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::All => IconName::LayoutGrid,
            Self::Photo => IconName::Image,
            Self::Video => IconName::Film,
            Self::Audio => IconName::Music,
        }
    }
}

/// What the toolbar's first row has room for at a window width (D-177). The row needs about
/// 1150 px with everything; the window can be 960 px wide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarFit {
    /// Mode names next to their icons; icons only below this.
    pub mode_labels: bool,
    /// The thumbnail size slider; `Ctrl+=` / `Ctrl+-` and the View menu stay when it hides.
    pub slider: bool,
    pub search_width: f32,
}

/// Mode names from this width on.
const LABELS_FROM: f32 = 1240.0;
/// The size slider from this width on.
const SLIDER_FROM: f32 = 1100.0;

pub fn toolbar_fit(width: f32) -> ToolbarFit {
    ToolbarFit {
        mode_labels: width >= LABELS_FROM,
        slider: width >= SLIDER_FROM,
        search_width: if width >= SLIDER_FROM { 220.0 } else { 170.0 },
    }
}

/// Height of the segmented control; it fits the 36 px toolbar like the other controls.
const SEGMENT_HEIGHT: f32 = 26.0;

/// The four modes as one segmented control in the middle of the toolbar: a muted track with
/// the current mode raised on it. A type filter that is a single extension raises none.
pub fn mode_buttons(
    ext: Option<&str>,
    locale: Locale,
    theme: &Theme,
    labels: bool,
    entity: Entity<RootApp>,
) -> AnyElement {
    let current = MediaMode::from_filter(ext);
    h_flex()
        .id("toolbar-modes")
        .flex_none()
        .h(px(SEGMENT_HEIGHT))
        .p(px(2.))
        .gap(px(2.))
        .items_center()
        .rounded(px(7.))
        .bg(theme.muted)
        .border_1()
        .border_color(theme.border)
        .children(MediaMode::ALL.into_iter().map(|mode| {
            let entity = entity.clone();
            let on = current == Some(mode);
            h_flex()
                .id(mode.id())
                .h_full()
                .px(px(if labels { 10. } else { 8. }))
                .gap_1()
                .items_center()
                .rounded(px(5.))
                .cursor_pointer()
                .text_xs()
                .when(on, |el| {
                    el.bg(theme.background)
                        .text_color(theme.foreground)
                        .shadow_xs()
                })
                .when(!on, |el| {
                    el.text_color(theme.muted_foreground)
                        .hover(|el| el.text_color(theme.foreground))
                })
                .child(Icon::new(mode.icon()).size(px(13.)))
                .when(labels, |el| el.child(t(locale, mode.label_key())))
                .on_click(move |_, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.set_ext_filter(mode.filter().map(str::to_string), cx);
                    });
                })
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toolbar_sheds_labels_then_the_slider() {
        let wide = toolbar_fit(1600.0);
        assert!(wide.mode_labels && wide.slider);
        assert_eq!(wide.search_width, 220.0);
        let medium = toolbar_fit(1180.0);
        assert!(!medium.mode_labels && medium.slider);
        // The smallest window (960 px, main.rs) keeps the modes as icons, without the slider.
        let narrow = toolbar_fit(crate::MIN_WINDOW_WIDTH);
        assert!(!narrow.mode_labels && !narrow.slider);
        assert_eq!(narrow.search_width, 170.0);
    }

    #[test]
    fn modes_are_type_buckets() {
        for mode in MediaMode::ALL {
            assert_eq!(MediaMode::from_filter(mode.filter()), Some(mode));
        }
        assert_eq!(
            MediaMode::from_filter(Some("videos")),
            Some(MediaMode::Video)
        );
        assert_eq!(MediaMode::from_filter(Some("png")), None);
        // Every bucket a mode sets is one the filter knows.
        assert!(library_core::type_matches("MP4", "video"));
        assert!(library_core::type_matches("flac", "audio"));
        assert!(library_core::type_matches("jpg", "image"));
    }
}
