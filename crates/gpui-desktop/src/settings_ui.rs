//! Settings in the shape of Eagle's preferences: a searchable section list on the left,
//! the chosen section on the right, and Apply / Save. Sections we do not have (AI, plugins)
//! are absent. Interface zoom percent is not a preference; General shows thumbnail size,
//! which is the grid zoom the app stores.

use app_store::{BrowsePreferences, Locale, LoginMethod, Preferences, Theme};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::label::Label;
use gpui_kit::component::{StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, PathPromptOptions, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use crate::app::RootApp;
use crate::i18n::t;
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsSection {
    General,
    Library,
    Login,
}

const SECTIONS: &[SettingsSection] = &[
    SettingsSection::General,
    SettingsSection::Library,
    SettingsSection::Login,
];

pub(crate) fn visible_sections(locale: Locale, query: &str) -> Vec<SettingsSection> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return SECTIONS.to_vec();
    }
    SECTIONS
        .iter()
        .copied()
        .filter(|section| {
            section_haystack(locale, *section).contains(&q) || zoom_alias(*section, &q)
        })
        .collect()
}

fn zoom_alias(section: SettingsSection, query: &str) -> bool {
    section == SettingsSection::General && "zoom".contains(query) && !query.is_empty()
}

fn section_haystack(locale: Locale, section: SettingsSection) -> String {
    let keys: &[&str] = match section {
        SettingsSection::General => &[
            "settings.general",
            "settings.theme",
            "settings.language",
            "settings.thumbnailSize",
            "settings.thumbnailHint",
            "settings.reopen",
            "settings.confirmTrash",
            "themes.system",
            "themes.light",
            "themes.dark",
        ],
        SettingsSection::Library => &["settings.library", "settings.defaultFolder"],
        SettingsSection::Login => &[
            "settings.login",
            "settings.loginMethod",
            "settings.setPassword",
            "settings.setPin",
            "settings.setWebAuthn",
            "settings.clearLogin",
            "login.password",
            "login.pin",
        ],
    };
    keys.iter()
        .map(|key| t(locale, key))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn section_title(section: SettingsSection) -> &'static str {
    match section {
        SettingsSection::General => "settings.general",
        SettingsSection::Library => "settings.library",
        SettingsSection::Login => "settings.login",
    }
}

impl RootApp {
    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_draft = self.session.preferences();
        self.settings_section = SettingsSection::General;
        self.settings_query = SharedString::default();
        self.settings_search.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.settings_password
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.settings_pin
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.show_settings = true;
        cx.notify();
    }

    /// Writes the draft. `close` is Save; Apply leaves the window open.
    /// Login actions are not part of the draft: they already ran.
    pub(crate) fn apply_settings(&mut self, close: bool, cx: &mut Context<Self>) {
        let mut prefs = self.settings_draft.clone();
        prefs.browse.thumbnail_size = prefs.browse.thumbnail_size.clamp(
            BrowsePreferences::MIN_THUMBNAIL,
            BrowsePreferences::MAX_THUMBNAIL,
        );
        if let Some(lib) = self.library.as_mut() {
            lib.browse.thumbnail_size = prefs.browse.thumbnail_size;
            lib.browse = lib.browse.clamped();
            prefs.browse = lib.browse;
            prefs.layout.left_open = lib.left_open;
            prefs.layout.right_open = lib.right_open;
            prefs.layout.left_width = lib.left_width;
            prefs.layout.right_width = lib.right_width;
        } else {
            prefs.browse = prefs.browse.clamped();
        }
        match self.session.save_preferences(prefs) {
            Ok(saved) => {
                self.settings_draft = saved.clone();
                self.theme_pref = saved.theme;
                self.locale = saved.locale;
                theme::apply_theme(saved.theme, cx);
                self.recompute_justified();
                self.status = t(self.locale, "settings.saved").into();
                if close {
                    self.show_settings = false;
                }
            }
            Err(error) => {
                self.status = error.to_string().into();
            }
        }
        cx.notify();
    }

    pub(crate) fn render_settings(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let visible = visible_sections(locale, &self.settings_query);
        let section = self.settings_section;
        v_flex()
            .id("settings-overlay")
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .bg(theme.background.opacity(0.55))
            .child(
                v_flex()
                    .w(px(760.))
                    .h(px(520.))
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .w_full()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .items_center()
                            .child(Label::new(t(locale, "settings.title")).font_bold())
                            .child(div().flex_1())
                            .child(
                                Button::new("settings-close")
                                    .ghost()
                                    .compact()
                                    .label("×")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_settings = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .w_full()
                            .min_h(px(0.))
                            .child(self.render_settings_nav(theme, locale, &visible, cx))
                            .child(self.render_settings_pane(theme, locale, section, &visible, cx)),
                    )
                    .child(self.render_settings_footer(theme, locale, cx)),
            )
    }

    fn render_settings_nav(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        visible: &[SettingsSection],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .w(px(200.))
            .h_full()
            .gap_1()
            .p_3()
            .border_r_1()
            .border_color(theme.border)
            .child(Input::new(&self.settings_search))
            .child(
                Label::new(t(locale, "settings.search"))
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
            .children(visible.iter().copied().map(|section| {
                let selected = section == self.settings_section;
                let id = match section {
                    SettingsSection::General => "settings-nav-general",
                    SettingsSection::Library => "settings-nav-library",
                    SettingsSection::Login => "settings-nav-login",
                };
                Button::new(id)
                    .when(selected, |button| button.primary())
                    .when(!selected, |button| button.ghost())
                    .label(t(locale, section_title(section)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_section = section;
                        cx.notify();
                    }))
                    .into_any_element()
            }))
            .when(visible.is_empty(), |el| {
                el.child(
                    Label::new(t(locale, "settings.noMatch"))
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
            })
    }

    fn render_settings_pane(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        section: SettingsSection,
        visible: &[SettingsSection],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let body = if !visible.contains(&section) {
            Label::new(t(locale, "settings.noMatch"))
                .text_sm()
                .text_color(theme.muted_foreground)
                .into_any_element()
        } else {
            match section {
                SettingsSection::General => self.render_settings_general(theme, locale, cx),
                SettingsSection::Library => self.render_settings_library(theme, locale, cx),
                SettingsSection::Login => self.render_settings_login(theme, locale, cx),
            }
        };
        v_flex()
            .id("settings-pane")
            .flex_1()
            .h_full()
            .min_w(px(0.))
            .overflow_y_scroll()
            .p_4()
            .gap_3()
            .child(Label::new(t(locale, section_title(section))).font_bold())
            .child(body)
    }

    fn render_settings_footer(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .gap_2()
            .px_4()
            .py_3()
            .justify_end()
            .border_t_1()
            .border_color(theme.border)
            .child(
                Button::new("settings-cancel")
                    .ghost()
                    .label(t(locale, "common.cancel"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_settings = false;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("settings-apply")
                    .outline()
                    .label(t(locale, "settings.apply"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.apply_settings(false, cx);
                    })),
            )
            .child(
                Button::new("settings-save")
                    .primary()
                    .label(t(locale, "settings.save"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.apply_settings(true, cx);
                    })),
            )
    }

    fn render_settings_general(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let draft = &self.settings_draft;
        v_flex()
            .gap_3()
            .child(Label::new(t(locale, "settings.theme")).text_sm())
            .child(
                h_flex()
                    .gap_3()
                    .child(theme_swatch(
                        "swatch-system",
                        t(locale, theme::theme_label_key(Theme::System)),
                        draft.theme == Theme::System,
                        theme,
                        swatch_fill_system(),
                        cx,
                        |draft| draft.theme = Theme::System,
                    ))
                    .child(theme_swatch(
                        "swatch-light",
                        t(locale, theme::theme_label_key(Theme::Light)),
                        draft.theme == Theme::Light,
                        theme,
                        swatch_fill_solid(rgb(0xf4f4f5)),
                        cx,
                        |draft| draft.theme = Theme::Light,
                    ))
                    .child(theme_swatch(
                        "swatch-dark",
                        t(locale, theme::theme_label_key(Theme::Dark)),
                        draft.theme == Theme::Dark,
                        theme,
                        swatch_fill_solid(rgb(0x27272a)),
                        cx,
                        |draft| draft.theme = Theme::Dark,
                    )),
            )
            .child(Label::new(t(locale, "settings.language")).text_sm())
            .child(
                h_flex()
                    .gap_2()
                    .child(locale_choice(
                        "set-locale-en",
                        "English",
                        Locale::En,
                        draft.locale,
                        cx,
                    ))
                    .child(locale_choice(
                        "set-locale-ru",
                        "Русский",
                        Locale::Ru,
                        draft.locale,
                        cx,
                    ))
                    .child(locale_choice(
                        "set-locale-zh",
                        "简体中文",
                        Locale::ZhCn,
                        draft.locale,
                        cx,
                    )),
            )
            .child(Label::new(t(locale, "settings.thumbnailSize")).text_sm())
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("thumb-smaller")
                            .outline()
                            .label("−")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let size = this.settings_draft.browse.thumbnail_size;
                                this.settings_draft.browse.thumbnail_size = size
                                    .saturating_sub(20)
                                    .max(BrowsePreferences::MIN_THUMBNAIL);
                                cx.notify();
                            })),
                    )
                    .child(Label::new(format!("{} px", draft.browse.thumbnail_size)))
                    .child(
                        Button::new("thumb-larger")
                            .outline()
                            .label("+")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let size = this.settings_draft.browse.thumbnail_size;
                                this.settings_draft.browse.thumbnail_size =
                                    (size + 20).min(BrowsePreferences::MAX_THUMBNAIL);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                Label::new(t(locale, "settings.thumbnailHint"))
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
            .child(check_row(
                "settings-reopen",
                t(locale, "settings.reopen"),
                draft.reopen_last_library,
                theme,
                cx,
                |draft| draft.reopen_last_library = !draft.reopen_last_library,
            ))
            .child(check_row(
                "settings-trash",
                t(locale, "settings.confirmTrash"),
                draft.confirm_before_trash,
                theme,
                cx,
                |draft| draft.confirm_before_trash = !draft.confirm_before_trash,
            ))
            .into_any_element()
    }

    fn render_settings_library(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let folder = self
            .settings_draft
            .default_library_parent
            .clone()
            .unwrap_or_else(|| self.session.default_library_location());
        v_flex()
            .gap_2()
            .child(Label::new(t(locale, "settings.defaultFolder")).text_sm())
            .child(
                Label::new(folder)
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
            .child(
                Button::new("settings-browse-default")
                    .outline()
                    .label(t(locale, "common.browse"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let prompt = SharedString::from(t(this.locale, "settings.defaultFolder"));
                        let rx = cx.prompt_for_paths(PathPromptOptions {
                            files: false,
                            directories: true,
                            multiple: false,
                            prompt: Some(prompt),
                        });
                        cx.spawn_in(window, async move |this, cx| {
                            if let Ok(Ok(Some(paths))) = rx.await
                                && let Some(path) = paths.into_iter().next()
                            {
                                let _ = this.update(cx, |this, cx| {
                                    this.settings_draft.default_library_parent =
                                        Some(path.to_string_lossy().into_owned());
                                    cx.notify();
                                });
                            }
                        })
                        .detach();
                    })),
            )
            .into_any_element()
    }

    fn render_settings_login(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let method = self
            .session
            .login_status()
            .map(|status| status.method)
            .unwrap_or(LoginMethod::None);
        v_flex()
            .gap_2()
            .child(
                Label::new(t(locale, "settings.loginImmediate"))
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
            .child(Label::new(format!(
                "{}: {}",
                t(locale, "settings.loginMethod"),
                Self::login_method_label(locale, method)
            )))
            .child(Label::new(t(locale, "login.password")).text_sm())
            .child(Input::new(&self.settings_password))
            .child(
                Button::new("set-login-password")
                    .outline()
                    .label(t(locale, "settings.setPassword"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let secret = this.settings_password.read(cx).value().to_string();
                        note_login(this, this.session.set_login_password(secret), cx);
                    })),
            )
            .child(Label::new(t(locale, "login.pin")).text_sm())
            .child(Input::new(&self.settings_pin))
            .child(
                Button::new("set-login-pin")
                    .outline()
                    .label(t(locale, "settings.setPin"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let secret = this.settings_pin.read(cx).value().to_string();
                        note_login(this, this.session.set_login_pin(secret), cx);
                    })),
            )
            .child(
                Button::new("set-login-webauthn")
                    .outline()
                    .label(t(locale, "settings.setWebAuthn"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        note_login(this, this.session.set_login_webauthn(), cx);
                    })),
            )
            .child(
                Button::new("clear-login")
                    .ghost()
                    .label(t(locale, "settings.clearLogin"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        match this.session.clear_login() {
                            Ok(_) => {
                                this.status = t(this.locale, "settings.loginCleared").into();
                            }
                            Err(error) => {
                                this.status = error.to_string().into();
                            }
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}

fn note_login(
    app: &mut RootApp,
    result: Result<app_store::LoginStatus, library_core::SourceError>,
    cx: &mut Context<RootApp>,
) {
    match result {
        Ok(status) => {
            app.status = format!(
                "{}: {}",
                t(app.locale, "settings.loginUpdated"),
                RootApp::login_method_label(app.locale, status.method)
            )
            .into();
        }
        Err(error) => {
            app.status = error.to_string().into();
        }
    }
    cx.notify();
}

fn theme_swatch(
    id: &'static str,
    label: String,
    selected: bool,
    theme: &gpui_kit::component::theme::Theme,
    fill: impl IntoElement,
    cx: &mut Context<RootApp>,
    pick: impl Fn(&mut Preferences) + 'static,
) -> impl IntoElement {
    let border = if selected {
        theme.primary
    } else {
        theme.border
    };
    v_flex()
        .id(id)
        .w(px(108.))
        .gap_1()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            pick(&mut this.settings_draft);
            cx.notify();
        }))
        .child(
            div()
                .h(px(48.))
                .w_full()
                .rounded(theme.radius)
                .border_2()
                .border_color(border)
                .overflow_hidden()
                .child(fill),
        )
        .child(Label::new(label).text_xs())
}

fn swatch_fill_system() -> impl IntoElement {
    h_flex()
        .size_full()
        .child(div().flex_1().h_full().bg(rgb(0xf4f4f5)))
        .child(div().flex_1().h_full().bg(rgb(0x18181b)))
}

fn swatch_fill_solid(color: impl Into<gpui_kit::Hsla>) -> impl IntoElement {
    div().size_full().bg(color.into())
}

fn locale_choice(
    id: &'static str,
    label: &'static str,
    locale: Locale,
    current: Locale,
    cx: &mut Context<RootApp>,
) -> impl IntoElement {
    Button::new(id)
        .when(locale == current, |button| button.primary())
        .when(locale != current, |button| button.ghost())
        .label(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.settings_draft.locale = locale;
            cx.notify();
        }))
}

fn check_row(
    id: &'static str,
    label: String,
    on: bool,
    theme: &gpui_kit::component::theme::Theme,
    cx: &mut Context<RootApp>,
    toggle: impl Fn(&mut Preferences) + 'static,
) -> impl IntoElement {
    h_flex()
        .id(id)
        .w_full()
        .gap_2()
        .items_center()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            toggle(&mut this.settings_draft);
            cx.notify();
        }))
        .child(
            div()
                .w(px(16.))
                .h(px(16.))
                .rounded(px(3.))
                .border_1()
                .border_color(theme.border)
                .items_center()
                .justify_center()
                .child(if on { "✓" } else { "" }),
        )
        .child(Label::new(label).text_sm())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_general_library_and_login() {
        assert_eq!(
            visible_sections(Locale::En, "theme"),
            vec![SettingsSection::General]
        );
        assert_eq!(
            visible_sections(Locale::En, "zoom"),
            vec![SettingsSection::General]
        );
        assert_eq!(
            visible_sections(Locale::En, "folder"),
            vec![SettingsSection::Library]
        );
        assert_eq!(
            visible_sections(Locale::En, "password"),
            vec![SettingsSection::Login]
        );
        assert!(visible_sections(Locale::En, "zzzz").is_empty());
        assert_eq!(visible_sections(Locale::En, "  ").len(), 3);
    }

    #[test]
    fn search_uses_the_active_language() {
        assert_eq!(
            visible_sections(Locale::Ru, "тема"),
            vec![SettingsSection::General]
        );
        assert_eq!(
            visible_sections(Locale::ZhCn, "缩略图"),
            vec![SettingsSection::General]
        );
    }
}
