//! Name, note and link edited in place in the right bar (D-151).
//!
//! The three fields are inputs kept for the app's life. They show the item the right bar
//! shows; a change is saved when the field loses focus (and on Enter for the name and the
//! link). It is saved to the item whose values the field was showing, not to whatever is
//! selected by then: a click on another cell blurs the field after the selection moved.

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{AppContext as _, Context, Entity, Focusable as _, Subscription, Window};
use library_core::ItemEdit;

use crate::app::RootApp;
use crate::i18n::t;
use crate::ops_mutations::EditField;

/// The inputs of the right bar.
#[derive(Clone)]
pub struct DetailEditors {
    pub name: Entity<InputState>,
    pub note: Entity<TextareaState>,
    pub link: Entity<InputState>,
}

/// The item the fields were filled from, and its values then.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundItem {
    pub id: String,
    pub name: String,
    pub note: String,
    pub link: String,
}

/// The edit a field's text makes, or none when it is unchanged (or an empty name).
pub fn field_edit(field: EditField, bound: &BoundItem, text: &str) -> Option<ItemEdit> {
    match field {
        EditField::Name => {
            let name = text.trim();
            (!name.is_empty() && name != bound.name).then(|| ItemEdit::Rename(name.to_owned()))
        }
        EditField::Note => (text != bound.note).then(|| ItemEdit::SetAnnotation(text.to_owned())),
        EditField::Link => {
            let link = text.trim();
            (link != bound.link).then(|| ItemEdit::SetUrl(link.to_owned()))
        }
    }
}

impl DetailEditors {
    pub fn new(locale: app_store::Locale, window: &mut Window, cx: &mut Context<RootApp>) -> Self {
        Self {
            name: cx.new(|cx| InputState::new(window, cx)),
            note: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(2, 8)
                    .placeholder(t(locale, "details.notePlaceholder"))
            }),
            link: cx.new(|cx| InputState::new(window, cx).placeholder("https://")),
        }
    }
}

impl RootApp {
    /// Folds or unfolds the right bar's Info (`exif == false`) or EXIF section.
    pub(crate) fn toggle_details_section(&mut self, exif: bool, cx: &mut Context<Self>) {
        if exif {
            self.exif_collapsed = !self.exif_collapsed;
        } else {
            self.info_collapsed = !self.info_collapsed;
        }
        cx.notify();
    }

    /// Saves on blur, and on Enter where Enter does not start a new line.
    pub(crate) fn subscribe_detail_editors(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        let editors = self.detail_editors.clone();
        vec![
            cx.subscribe_in(
                &editors.name,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        this.commit_detail(EditField::Name, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &editors.note,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Blur) {
                        this.commit_detail(EditField::Note, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &editors.link,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        this.commit_detail(EditField::Link, cx);
                    }
                },
            ),
        ]
    }

    /// Fills the fields from the right bar's item when it changes; a field being typed in
    /// keeps its text.
    pub(crate) fn sync_detail_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bound = self.details.item.as_ref().map(|item| BoundItem {
            id: item.id.0.clone(),
            name: item.name.clone(),
            note: item.annotation.clone(),
            link: item.url.clone(),
        });
        if bound == self.detail_bound {
            return;
        }
        if let Some(values) = bound.as_ref() {
            let editors = self.detail_editors.clone();
            if !editors.name.focus_handle(cx).is_focused(window) {
                editors.name.update(cx, |input, cx| {
                    input.set_value(values.name.clone(), window, cx)
                });
            }
            if !editors.note.focus_handle(cx).is_focused(window) {
                editors.note.update(cx, |input, cx| {
                    input.set_value(values.note.clone(), window, cx)
                });
            }
            if !editors.link.focus_handle(cx).is_focused(window) {
                editors.link.update(cx, |input, cx| {
                    input.set_value(values.link.clone(), window, cx)
                });
            }
        }
        self.detail_bound = bound;
    }

    fn commit_detail(&mut self, field: EditField, cx: &mut Context<Self>) {
        let Some(bound) = self.detail_bound.clone() else {
            return;
        };
        let text = match field {
            EditField::Name => self.detail_editors.name.read(cx).value().to_string(),
            EditField::Note => self.detail_editors.note.read(cx).value().to_string(),
            EditField::Link => self.detail_editors.link.read(cx).value().to_string(),
        };
        let Some(edit) = field_edit(field, &bound, &text) else {
            // Unchanged, or an empty name: show the item's value again.
            self.detail_bound = None;
            cx.notify();
            return;
        };
        if self.library.as_ref().is_some_and(|lib| lib.info.read_only) {
            return;
        }
        match self.session.apply_item_edit(&bound.id, edit) {
            Ok(_) => {
                self.status = t(self.locale, "edit.saved").into();
                self.reload_after_write(cx);
            }
            Err(error) => {
                self.status = format!("{error}").into();
                self.detail_bound = None;
                self.refresh_details(cx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound() -> BoundItem {
        BoundItem {
            id: "a".into(),
            name: "P1011258".into(),
            note: String::new(),
            link: String::new(),
        }
    }

    #[test]
    fn unchanged_text_saves_nothing() {
        assert!(field_edit(EditField::Name, &bound(), "P1011258").is_none());
        assert!(field_edit(EditField::Note, &bound(), "").is_none());
        assert!(field_edit(EditField::Link, &bound(), "  ").is_none());
    }

    #[test]
    fn an_empty_name_is_refused_and_names_are_trimmed() {
        assert!(field_edit(EditField::Name, &bound(), "   ").is_none());
        assert_eq!(
            field_edit(EditField::Name, &bound(), " Beach "),
            Some(ItemEdit::Rename("Beach".into()))
        );
    }

    #[test]
    fn notes_keep_their_text_and_links_are_trimmed() {
        assert_eq!(
            field_edit(EditField::Note, &bound(), " line\nnext "),
            Some(ItemEdit::SetAnnotation(" line\nnext ".into()))
        );
        assert_eq!(
            field_edit(EditField::Link, &bound(), " https://example.com "),
            Some(ItemEdit::SetUrl("https://example.com".into()))
        );
    }
}
