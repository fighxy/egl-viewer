//! The start screen (D-150): what to do on the left, the recent libraries on the right.
//!
//! Left: the app, then actions as rows that say what they do (Create, Open; Eagle's Import and
//! Connect below a label). Right: recent libraries newest first, each with its folder, when it
//! was last opened and a × to forget it; a library whose folder is gone (an unplugged drive)
//! stays listed, marked, and does not open.

use std::path::PathBuf;

use app_store::{Locale, RecentLibrary};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{Icon, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};

use crate::app::RootApp;
use crate::browse::format::format_date;
use crate::i18n::t;
use crate::library;

/// Recent libraries as the list shows them: newest first.
pub fn recent_rows(mut recent: Vec<RecentLibrary>) -> Vec<RecentLibrary> {
    recent.sort_by_key(|lib| std::cmp::Reverse(lib.last_opened_at));
    recent
}

/// The folder a library sits in, for the line under its name.
pub fn parent_folder(path: &str) -> String {
    std::path::Path::new(path)
        .parent()
        .map(|parent| parent.display().to_string())
        .filter(|parent| !parent.is_empty())
        .unwrap_or_else(|| path.to_owned())
}

type OnClick = Box<dyn Fn(&mut RootApp, &mut Window, &mut Context<RootApp>) + 'static>;

impl RootApp {
    pub(crate) fn render_welcome(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let recent = recent_rows(self.session.recent_libraries());
        let status = self.status.clone();

        let action = |id: &'static str,
                      icon: IconName,
                      title: &'static str,
                      hint: &'static str,
                      primary: bool,
                      on_click: OnClick,
                      cx: &mut Context<Self>| {
            // One quiet row style for every action; only the main one is filled. The icon
            // sits in a small tile, so the rows line up whatever the icon (D-150).
            let (bg, fg, sub, tile, tile_fg) = if primary {
                (
                    theme.primary,
                    theme.primary_foreground,
                    theme.primary_foreground.opacity(0.7),
                    theme.primary_foreground.opacity(0.14),
                    theme.primary_foreground,
                )
            } else {
                (
                    gpui_kit::transparent_black(),
                    theme.foreground,
                    theme.muted_foreground,
                    theme.muted,
                    theme.foreground,
                )
            };
            let hover = if primary {
                theme.primary_hover
            } else {
                theme.list_hover
            };
            h_flex()
                .id(id)
                .w_full()
                .gap_3()
                .px_2()
                .py(px(8.))
                .rounded(theme.radius)
                .bg(bg)
                .hover(move |el| el.bg(hover))
                .cursor_pointer()
                .child(
                    div()
                        .size(px(32.))
                        .flex_none()
                        .rounded(px(8.))
                        .bg(tile)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(icon).small().text_color(tile_fg)),
                )
                .child(
                    v_flex()
                        .min_w_0()
                        .child(
                            Label::new(t(locale, title))
                                .text_sm()
                                .font_semibold()
                                .text_color(fg),
                        )
                        .child(
                            Label::new(t(locale, hint))
                                .text_xs()
                                .text_color(sub)
                                .whitespace_nowrap()
                                .text_ellipsis(),
                        ),
                )
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, window, cx| on_click(this, window, cx)),
                )
        };

        let left = v_flex()
            .w(px(320.))
            .flex_none()
            .p_6()
            .gap_4()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        div()
                            .size(px(40.))
                            .flex_none()
                            .rounded(px(10.))
                            .bg(theme.primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::LibraryBig)
                                    .text_color(theme.primary_foreground),
                            ),
                    )
                    .child(
                        v_flex()
                            .child(Label::new("egl-viewer").text_lg().font_bold())
                            .child(
                                Label::new(format!("v{}", library::APP_VERSION))
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            ),
                    ),
            )
            .child(
                Label::new(t(locale, "welcome.description"))
                    .text_sm()
                    .text_color(theme.muted_foreground),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(action(
                        "welcome-create",
                        IconName::FolderPlus,
                        "welcome.create",
                        "welcome.createHint",
                        true,
                        Box::new(|this, _, cx| this.begin_create_library(cx)),
                        cx,
                    ))
                    .child(action(
                        "welcome-open",
                        IconName::FolderOpen,
                        "welcome.open",
                        "welcome.openHint",
                        false,
                        Box::new(|this, window, cx| this.pick_open_library(window, cx)),
                        cx,
                    )),
            )
            .child(
                v_flex()
                    .gap_2()
                    .pt_2()
                    .child(
                        Label::new(t(locale, "welcome.eagle"))
                            .text_xs()
                            .font_semibold()
                            .text_color(theme.muted_foreground),
                    )
                    .child(action(
                        "welcome-import-eagle",
                        IconName::Import,
                        "welcome.importEagle",
                        "welcome.importEagleHint",
                        false,
                        Box::new(|this, window, cx| this.begin_import_eagle(window, cx)),
                        cx,
                    ))
                    .child(action(
                        "welcome-connect-eagle",
                        IconName::Plug,
                        "welcome.connectEagle",
                        "welcome.connectEagleHint",
                        false,
                        Box::new(|this, window, cx| this.begin_connect_eagle(window, cx)),
                        cx,
                    )),
            )
            .child(div().flex_1())
            .when(!status.is_empty(), |el| {
                el.child(
                    Label::new(status)
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
            });

        let count = recent.len();
        let rows = recent.into_iter().map(|lib| {
            let open = lib.path.clone();
            let forget = lib.path.clone();
            let missing = lib.missing;
            h_flex()
                .id(SharedString::from(format!("recent-{}", lib.path)))
                .group("recent-row")
                .w_full()
                .gap_3()
                .px_2()
                .py(px(7.))
                .rounded(theme.radius)
                .when(!missing, |el| {
                    let hover = theme.list_hover;
                    el.hover(move |el| el.bg(hover)).cursor_pointer()
                })
                .child(
                    div()
                        .size(px(32.))
                        .flex_none()
                        .rounded(px(8.))
                        .bg(theme.muted)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::new(if missing {
                                IconName::TriangleAlert
                            } else {
                                IconName::Library
                            })
                            .small()
                            .text_color(theme.muted_foreground),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Label::new(lib.name.clone())
                                        .text_sm()
                                        .font_semibold()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .when(missing, |el| el.text_color(theme.muted_foreground)),
                                )
                                .when(missing, |el| {
                                    el.child(
                                        div().px_1p5().rounded(px(4.)).bg(theme.muted).child(
                                            Label::new(t(locale, "welcome.missing"))
                                                .text_xs()
                                                .text_color(theme.muted_foreground),
                                        ),
                                    )
                                }),
                        )
                        .child(
                            Label::new(parent_folder(&lib.path))
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .whitespace_nowrap()
                                .text_ellipsis(),
                        ),
                )
                .child(
                    Label::new(format_date(Some(lib.last_opened_at)))
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .whitespace_nowrap(),
                )
                // The × shows on the row under the pointer only.
                .child(
                    div()
                        .invisible()
                        .group_hover("recent-row", |style| style.visible())
                        .child(
                            Button::new(SharedString::from(format!("forget-{}", lib.path)))
                                .ghost()
                                .xsmall()
                                .icon(IconName::X)
                                .tooltip(t(locale, "welcome.removeRecentTitle"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let _ = this.session.forget_recent_library(&forget);
                                    cx.notify();
                                })),
                        ),
                )
                .when(!missing, |el| {
                    el.on_click(cx.listener(move |this, _, window, cx| {
                        this.open_path(PathBuf::from(&open), window, cx);
                    }))
                })
        });

        let right = v_flex()
            .flex_1()
            .min_w_0()
            .p_6()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .px_2()
                    .child(Label::new(t(locale, "welcome.recent")).font_semibold())
                    .when(count > 0, |el| {
                        el.child(
                            div().px_1p5().rounded(px(6.)).bg(theme.muted).child(
                                Label::new(count.to_string())
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            ),
                        )
                    }),
            )
            .child(if count == 0 {
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Library)
                            .large()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        Label::new(t(locale, "welcome.noRecent"))
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .into_any_element()
            } else {
                v_flex()
                    .id("recent-list")
                    .flex_1()
                    .min_h_0()
                    .gap_0p5()
                    .overflow_y_scrollbar()
                    .children(rows)
                    .into_any_element()
            });

        v_flex()
            .id("welcome")
            .size_full()
            .items_center()
            .justify_center()
            .p_8()
            .bg(theme.background)
            .child(
                h_flex()
                    .w(px(820.))
                    .max_w_full()
                    .h(px(480.))
                    .max_h_full()
                    .items_stretch()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(theme.border)
                    // Its own background: without one the shadow showed through the right pane.
                    .bg(theme.background)
                    .overflow_hidden()
                    .shadow_lg()
                    .child(left)
                    .child(right),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib(name: &str, at: i64) -> RecentLibrary {
        RecentLibrary {
            path: format!("D:\\Libraries\\{name}.eglib"),
            name: name.into(),
            library_id: None,
            last_opened_at: at,
            missing: false,
        }
    }

    #[test]
    fn recent_libraries_newest_first() {
        let rows = recent_rows(vec![lib("Old", 1), lib("New", 3), lib("Mid", 2)]);
        let names: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["New", "Mid", "Old"]);
    }

    #[test]
    fn the_line_under_a_name_is_its_folder() {
        assert_eq!(
            parent_folder("D:\\Libraries\\RSPECT.eglib"),
            "D:\\Libraries"
        );
        assert_eq!(parent_folder("RSPECT.eglib"), "RSPECT.eglib");
    }
}
