//! Shared library chrome snippets (status / banners).

use app_store::Locale;
use gpui_kit::component::{h_flex, label::Label};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement, IntoElement, ParentElement, Styled, div};

use crate::i18n::{t, t_n};
use crate::library::OpenLibraryState;

pub fn render_status_bar(state: &OpenLibraryState, locale: Locale) -> impl IntoElement {
    h_flex()
        .id("status-bar")
        .w_full()
        .items_center()
        .gap_3()
        .px_3()
        .py_1()
        .border_t_1()
        .child(Label::new(t_n(locale, "shell.statusItems", state.total)).text_xs())
        .when(state.selection.count() > 0, |el| {
            el.child(
                Label::new(t_n(
                    locale,
                    "shell.statusSelected",
                    state.selection.count() as u32,
                ))
                .text_xs(),
            )
        })
        .child(div().flex_1())
        .when(state.info.read_only, |el| {
            el.child(Label::new(t(locale, "common.readOnly")).text_xs())
        })
}

pub fn render_read_only_banner(locale: Locale) -> impl IntoElement {
    h_flex()
        .id("readonly-banner")
        .w_full()
        .px_3()
        .py_1()
        .child(Label::new(t(locale, "shell.readOnlyBanner")).text_sm())
}

