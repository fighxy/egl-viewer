//! The window when it views loose files opened from Explorer (D-180): the full-size preview
//! fills it, its own shortcuts work, and no library action is bound.
//!
//! Which files and in what order is [`crate::file_viewer`]; opening, stepping and playing
//! are the preview's own (`ops_preview`, `ops_media`), which read loose files by path.

use std::path::{Path, PathBuf};

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{Context, InteractiveElement, IntoElement, ParentElement, Styled, Window};

use crate::app::RootApp;
use crate::browse::preview;
use crate::commands::*;
use crate::file_viewer::{self, LooseFiles};

impl RootApp {
    /// Shows `path` at once, then lists its folder in the background so Left / Right step
    /// through the photos and videos beside it.
    pub(crate) fn open_loose(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.set_window_title(&file_viewer::window_title(&path));
        self.loose = Some(LooseFiles::single(path.clone()));
        // The shortcuts are bound on the root: it takes the keyboard from the start.
        self.focus.focus(window, cx);
        self.open_preview_at(0, cx);
        cx.spawn(async move |this, cx| {
            let opened = path.clone();
            let (files, _) = cx
                .background_executor()
                .spawn(async move { LooseFiles::around(&opened) })
                .await;
            this.update(cx, |this, cx| this.adopt_loose_listing(files, &path, cx))
                .ok();
        })
        .detach();
    }

    /// The folder listing landed: the preview stays on the file it shows, now at its place
    /// among the others, and the neighbours start decoding.
    fn adopt_loose_listing(&mut self, files: LooseFiles, opened: &Path, cx: &mut Context<Self>) {
        let shown = self
            .preview
            .as_ref()
            .and_then(|preview| preview.key.as_ref())
            .map(|key| PathBuf::from(&key.id))
            .unwrap_or_else(|| opened.to_path_buf());
        let Some(index) = files.position(&shown) else {
            return;
        };
        let total = files.len() as u32;
        self.loose = Some(files);
        if let Some(preview) = self.preview.as_mut() {
            preview.index = index;
            preview.total = total;
        }
        self.load_preview_window(index, cx);
        cx.notify();
    }

    /// The title follows the file on screen as the preview steps.
    fn sync_viewer_title(&mut self, window: &mut Window) {
        let Some(preview) = self.preview.as_ref() else {
            return;
        };
        let Some(key) = preview.key.as_ref() else {
            return;
        };
        let title = file_viewer::window_title(Path::new(&key.id));
        if self.viewer_title.as_deref() != Some(title.as_str()) {
            window.set_window_title(&title);
            self.viewer_title = Some(title);
        }
    }

    pub(crate) fn render_viewer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.sync_viewer_title(window);
        let theme = cx.theme().clone();
        let size = window.viewport_size();
        let size = (size.width.as_f32(), size.height.as_f32());
        v_flex()
            .id("viewer-root")
            .key_context(crate::commands::ROOT_CONTEXT)
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ClosePreview, _, cx| {
                this.close_preview(cx);
            }))
            .on_action(cx.listener(|this, _: &Exit, _, cx| {
                this.close_preview(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleFullScreen, window, cx| {
                window.toggle_fullscreen();
                this.fullscreen = window.is_fullscreen();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &MoveFocusLeft, _, cx| {
                this.preview_step(-1, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusPageUp, _, cx| {
                this.preview_step(-1, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusRight, _, cx| {
                this.preview_step(1, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusPageDown, _, cx| {
                this.preview_step(1, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusHome, _, cx| {
                this.preview_jump(false, cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusEnd, _, cx| {
                this.preview_jump(true, cx);
            }))
            .on_action(cx.listener(|this, _: &LargerThumbs, _, cx| {
                this.preview_zoom(1.25, cx);
            }))
            .on_action(cx.listener(|this, _: &SmallerThumbs, _, cx| {
                this.preview_zoom(1.0 / 1.25, cx);
            }))
            .on_action(cx.listener(|this, _: &PreviewRotate, _, cx| {
                this.preview_rotate(cx);
            }))
            // Space plays and pauses here: there is no grid to go back to.
            .on_action(cx.listener(|this, _: &SpacePreviewDown, _, cx| {
                this.preview_gif_toggle(cx);
            }))
            .on_action(cx.listener(|this, _: &PreviewGifToggle, _, cx| {
                this.preview_gif_toggle(cx);
            }))
            .on_action(cx.listener(|this, _: &PreviewGifPrevFrame, _, cx| {
                this.preview_gif_step(-1, cx);
            }))
            .on_action(cx.listener(|this, _: &PreviewGifNextFrame, _, cx| {
                this.preview_gif_step(1, cx);
            }))
            .on_action(cx.listener(|this, _: &MediaSkipBack, _, cx| {
                this.media_skip(-crate::ops_media::SKIP_SECONDS, cx);
            }))
            .on_action(cx.listener(|this, _: &MediaSkipForward, _, cx| {
                this.media_skip(crate::ops_media::SKIP_SECONDS, cx);
            }))
            .on_action(cx.listener(|this, _: &MediaToggleMute, _, cx| {
                if this.media.is_some() {
                    this.media_toggle_mute(cx);
                }
            }))
            .when_some(self.preview.as_ref(), |el, state| {
                el.child(preview::render_preview(
                    state,
                    self.media.as_ref(),
                    self.locale,
                    size,
                    cx.entity(),
                ))
            })
    }
}
