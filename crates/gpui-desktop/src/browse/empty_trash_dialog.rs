//! The window that Empty Trash opens once confirmed (D-172): a progress bar with Cancel while
//! the files go, then what was deleted and how much disk it freed (or why it stopped), and
//! Close. Modal: the library behind it takes no input until it is closed.

use app_store::Locale;
use gpui_kit::assets::IconName;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::theme::Theme;
use gpui_kit::component::{Icon, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, Entity, InteractiveElement, IntoElement, ParentElement, Styled, black, div, px,
    relative,
};
use library_core::TrashInfo;

use crate::app::RootApp;
use crate::i18n::{t, t_n};

/// Empty Trash from confirmation to Close.
#[derive(Debug, Clone, PartialEq)]
pub struct EmptyTrashDialog {
    /// What the confirmation said would go.
    pub info: TrashInfo,
    /// Steps done and in all (rows, then files), from the background thread.
    pub progress: (u32, u32),
    /// Cancel was pressed; the thread stops after the file it is on.
    pub cancelled: bool,
    /// Items deleted, or the error; `None` while it runs.
    pub outcome: Option<Result<u32, String>>,
}

impl EmptyTrashDialog {
    pub fn new(info: TrashInfo) -> Self {
        Self {
            info,
            progress: (0, 0),
            cancelled: false,
            outcome: None,
        }
    }

    /// 0…1 for the bar; full once finished.
    pub fn fraction(&self) -> f32 {
        if self.outcome.is_some() {
            return 1.0;
        }
        let (done, total) = self.progress;
        if total == 0 {
            0.0
        } else {
            (done as f32 / total as f32).clamp(0.0, 1.0)
        }
    }

    pub fn running(&self) -> bool {
        self.outcome.is_none()
    }

    /// The window's title.
    pub fn title(&self, locale: Locale) -> String {
        match &self.outcome {
            None => t(locale, "trash.emptying"),
            Some(Err(_)) => t(locale, "trash.failedTitle"),
            Some(Ok(_)) if self.cancelled => t(locale, "trash.emptyCancelled"),
            Some(Ok(_)) => t(locale, "trash.doneTitle"),
        }
    }

    /// The line under the bar: what goes while it runs, then the result.
    pub fn message(&self, locale: Locale) -> String {
        let size = crate::browse::format::format_bytes(Some(self.info.bytes));
        match &self.outcome {
            None => format!(
                "{} · {size} — {}%",
                t_n(locale, "trash.items", self.info.items),
                (self.fraction() * 100.0).round() as u32
            ),
            Some(Err(error)) => error.clone(),
            Some(Ok(n)) if self.cancelled || *n < self.info.items => {
                t_n(locale, "trash.partialMessage", *n)
            }
            Some(Ok(n)) => t_n(locale, "trash.doneMessage", *n).replace("{size}", &size),
        }
    }
}

const WIDTH: f32 = 380.0;

pub fn render(
    dialog: &EmptyTrashDialog,
    locale: Locale,
    theme: &Theme,
    entity: Entity<RootApp>,
) -> AnyElement {
    let running = dialog.running();
    let failed = matches!(dialog.outcome, Some(Err(_)));
    let (icon, tint) = match &dialog.outcome {
        None => (IconName::Trash, theme.muted_foreground),
        Some(Err(_)) => (IconName::TriangleAlert, theme.danger),
        Some(Ok(_)) => (IconName::CircleCheck, theme.success),
    };
    let button = Button::new("empty-trash-dialog-button");
    let button = if running {
        let entity = entity.clone();
        button
            .outline()
            .label(t(locale, "trash.cancelEmpty"))
            .disabled(dialog.cancelled)
            .on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| this.cancel_empty_trash(cx));
            })
    } else {
        let entity = entity.clone();
        button
            .primary()
            .label(t(locale, "common.close"))
            .on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| this.close_empty_trash_dialog(cx));
            })
    };

    div()
        .id("empty-trash-overlay")
        .absolute()
        .inset_0()
        // Modal: the library under it gets no clicks, hover or wheel.
        .occlude()
        .bg(black().opacity(0.35))
        .flex()
        .items_center()
        .justify_center()
        .child(
            v_flex()
                .id("empty-trash-dialog")
                .w(px(WIDTH))
                .p_4()
                .gap_3()
                .rounded(px(10.))
                .bg(theme.popover)
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(icon).size(px(18.)).text_color(tint))
                        .child(Label::new(dialog.title(locale)).font_semibold()),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(6.))
                        .rounded(px(3.))
                        .bg(theme.muted)
                        .child(
                            div()
                                .h_full()
                                .w(relative(dialog.fraction()))
                                .rounded(px(3.))
                                .bg(if failed { theme.danger } else { theme.primary }),
                        ),
                )
                .child(
                    Label::new(dialog.message(locale))
                        .text_sm()
                        .text_color(theme.muted_foreground),
                )
                .child(h_flex().justify_end().child(button)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog() -> EmptyTrashDialog {
        EmptyTrashDialog::new(TrashInfo {
            items: 61,
            bytes: 49 << 20,
        })
    }

    #[test]
    fn progress_runs_to_full() {
        let mut d = dialog();
        assert_eq!(d.fraction(), 0.0);
        d.progress = (61, 122);
        assert!((d.fraction() - 0.5).abs() < 1e-6);
        assert!(d.running());
        d.outcome = Some(Ok(61));
        assert_eq!(d.fraction(), 1.0);
        assert!(!d.running());
    }

    #[test]
    fn messages_follow_the_outcome() {
        let locale = Locale::En;
        let mut d = dialog();
        d.progress = (30, 122);
        assert!(d.message(locale).contains("25%"), "{}", d.message(locale));
        d.outcome = Some(Ok(61));
        let done = d.message(locale);
        assert!(done.contains("61") && done.contains("49"), "{done}");
        assert_eq!(d.title(locale), t(locale, "trash.doneTitle"));
        d.cancelled = true;
        d.outcome = Some(Ok(12));
        assert!(d.message(locale).contains("12"));
        assert_eq!(d.title(locale), t(locale, "trash.emptyCancelled"));
        d.outcome = Some(Err("disk full".into()));
        assert_eq!(d.message(locale), "disk full");
        assert_eq!(d.title(locale), t(locale, "trash.failedTitle"));
    }
}
