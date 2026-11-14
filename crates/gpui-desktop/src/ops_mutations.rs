//! Phase 3 Session-backed mutations: edits, trash, folders, import, locks, maintenance.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::{StyledExt as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, ExternalPaths, IntoElement, ParentElement, PathPromptOptions,
    SharedString, Styled, Window, px,
};
use library_core::{FolderChange, ItemEdit, SmartFolderChange};
use library_native::DuplicateChoice;

use crate::app::RootApp;
use crate::i18n::{t, t_n};
use crate::library::Place;
use crate::platform;

/// How long the search field waits after the last key before it searches (D-179).
const SEARCH_PAUSE: std::time::Duration = std::time::Duration::from_millis(250);

/// Which ⊕ of the left bar a name popover belongs to (D-161, D-163).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamePromptKind {
    NewFolder,
    SmartFolder,
}

/// The open name popover and what it will create.
#[derive(Debug, Clone)]
pub(crate) enum NamePrompt {
    NewFolder,
    /// The rules of the view when the popover opened.
    SmartFolder(desktop_session::SmartDraft),
}

impl NamePrompt {
    pub(crate) fn kind(&self) -> NamePromptKind {
        match self {
            Self::NewFolder => NamePromptKind::NewFolder,
            Self::SmartFolder(_) => NamePromptKind::SmartFolder,
        }
    }
}

impl RootApp {
    pub(crate) fn apply_edit_to_selection(&mut self, edit: ItemEdit, cx: &mut Context<Self>) {
        if self.library.as_ref().is_some_and(|lib| lib.info.read_only) {
            self.status = SharedString::from(t(self.locale, "errors.readOnly"));
            cx.notify();
            return;
        }
        let Some(target) = self.selection_target() else {
            return;
        };
        let result = if target.ids.len() == 1 {
            self.session
                .apply_item_edit(&target.ids[0].0, edit)
                .map(|_| 1u32)
        } else {
            self.session.apply_batch_edit(target, edit)
        };
        match result {
            Ok(n) => {
                self.status = SharedString::from(t_n(self.locale, "edit.changed", n));
                self.reload_after_write(cx);
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                self.refresh_details(cx);
            }
        }
    }

    pub(crate) fn set_rating(&mut self, star: u8, cx: &mut Context<Self>) {
        self.apply_edit_to_selection(ItemEdit::SetStar(star), cx);
    }

    pub(crate) fn add_tag_to_selection(&mut self, name: String, cx: &mut Context<Self>) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        self.apply_edit_to_selection(ItemEdit::AddTags(vec![name]), cx);
    }

    pub(crate) fn remove_tag_from_selection(&mut self, name: String, cx: &mut Context<Self>) {
        self.apply_edit_to_selection(ItemEdit::RemoveTags(vec![name]), cx);
    }

    pub(crate) fn undo_last(&mut self, cx: &mut Context<Self>) {
        match self.session.undo() {
            Ok(Some(status)) => {
                self.status = SharedString::from(format!(
                    "{} ({})",
                    t(self.locale, "commands.edit.undo"),
                    status.items
                ));
                self.reload_after_write(cx);
            }
            Ok(None) => {
                self.status = SharedString::from(t(self.locale, "edit.nothingToUndo"));
                cx.notify();
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn trash_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .library
            .as_ref()
            .is_some_and(|lib| matches!(lib.place, Place::Trash))
        {
            return;
        }
        let Some(target) = self.selection_target() else {
            return;
        };
        let confirm = self.session.preferences().confirm_before_trash;
        if confirm {
            let locale = self.locale;
            let entity = cx.entity();
            let count = target.ids.len();
            window.open_dialog(cx, move |dialog, _, _| {
                dialog.title(t(locale, "trash.confirmTitle")).child(
                    v_flex()
                        .gap_3()
                        .p_2()
                        .w(px(360.))
                        .child(Label::new(t_n(
                            locale,
                            "trash.confirmMessage",
                            count as u32,
                        )))
                        .child(
                            h_flex()
                                .gap_2()
                                .justify_end()
                                .child(
                                    Button::new("trash-cancel")
                                        .label(t(locale, "common.cancel"))
                                        .on_click(|_, window, cx| {
                                            window.close_dialog(cx);
                                        }),
                                )
                                .child({
                                    let entity = entity.clone();
                                    Button::new("trash-ok")
                                        .primary()
                                        .label(t(locale, "trash.move"))
                                        .on_click(move |_, window, cx| {
                                            window.close_dialog(cx);
                                            entity.update(cx, |this, cx| {
                                                this.apply_edit_to_selection(ItemEdit::Trash, cx);
                                            });
                                        })
                                }),
                        ),
                )
            });
        } else {
            self.apply_edit_to_selection(ItemEdit::Trash, cx);
        }
    }

    pub(crate) fn restore_selection(&mut self, cx: &mut Context<Self>) {
        self.apply_edit_to_selection(ItemEdit::Restore, cx);
    }

    pub(crate) fn delete_permanently_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.selection_target() else {
            return;
        };
        let locale = self.locale;
        let entity = cx.entity();
        let count = target.ids.len();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "trash.deleteTitle")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t_n(locale, "trash.deleteMessage", count as u32)))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("del-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                Button::new("del-ok")
                                    .primary()
                                    .label(t(locale, "trash.delete"))
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.run_delete_permanently(cx);
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    fn run_delete_permanently(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.selection_target() else {
            return;
        };
        match self.session.delete_permanently(target) {
            Ok(n) => {
                self.status = SharedString::from(t_n(self.locale, "trash.deleted", n));
                if let Some(lib) = self.library.as_mut() {
                    lib.selection.clear();
                }
                self.reload_after_write(cx);
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
            }
        }
    }

    /// Restore All from the Trash's context menu (D-164). `Ctrl+Z` takes it back.
    pub(crate) fn restore_all_trash(&mut self, cx: &mut Context<Self>) {
        match self.session.restore_trash() {
            Ok(n) => {
                self.status = SharedString::from(t_n(self.locale, "trash.restoredAll", n));
                self.reload_after_write(cx);
            }
            Err(error) => self.status = SharedString::from(format!("{error}")),
        }
        cx.notify();
    }

    /// What Empty Trash would delete, or `None` (with the reason in the status bar) when there
    /// is nothing to delete.
    fn trash_to_empty(&mut self, cx: &mut Context<Self>) -> Option<library_core::TrashInfo> {
        let info = match self.session.trash_info() {
            Ok(info) => info,
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
                return None;
            }
        };
        if info.items == 0 {
            self.status = SharedString::from(t(self.locale, "trash.alreadyEmpty"));
            cx.notify();
            return None;
        }
        Some(info)
    }

    /// Empty Trash… of the Trash row's menu: the confirmation opens as a popover at the row
    /// (D-165).
    pub(crate) fn ask_empty_trash_here(&mut self, cx: &mut Context<Self>) {
        self.trash_confirm = self.trash_to_empty(cx);
        cx.notify();
    }

    /// The popover's answer: empty the trash, or just close.
    pub(crate) fn answer_empty_trash(&mut self, confirmed: bool, cx: &mut Context<Self>) {
        let info = self.trash_confirm.take();
        if confirmed && let Some(info) = info {
            self.run_empty_trash(info, cx);
        }
        cx.notify();
    }

    /// Close of the Empty Trash window, once it finished (D-172).
    pub(crate) fn close_empty_trash_dialog(&mut self, cx: &mut Context<Self>) {
        if self
            .empty_trash_dialog
            .as_ref()
            .is_some_and(|dialog| !dialog.running())
        {
            self.empty_trash_dialog = None;
            cx.notify();
        }
    }

    /// File → Empty Trash: the same confirmation in a dialog, since the left bar may be hidden.
    pub(crate) fn empty_trash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(info) = self.trash_to_empty(cx) else {
            return;
        };
        let locale = self.locale;
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "trash.emptyTitle")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    // What goes and how much disk it frees; the files leave the disk for good
                    // (D-164).
                    .child(Label::new(t(locale, "trash.emptyMessage").replace(
                        "{what}",
                        &crate::ops_organize::trash_summary(locale, info),
                    )))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("empty-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                Button::new("empty-ok")
                                    .primary()
                                    .label(t(locale, "trash.empty"))
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.run_empty_trash(info, cx);
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    /// Deletes the trash on a thread; a window shows the progress and then the result (D-172).
    fn run_empty_trash(&mut self, info: library_core::TrashInfo, cx: &mut Context<Self>) {
        if self.emptying_trash {
            return;
        }
        self.empty_trash_dialog = Some(crate::browse::empty_trash_dialog::EmptyTrashDialog::new(
            info,
        ));
        let session = Arc::clone(&self.session);
        let progress = Arc::new(Mutex::new((0u32, 0u32)));
        let progress_ui = Arc::clone(&progress);
        let result: crate::app::EmptyTrashResult = Arc::new(Mutex::new(None));
        let result_thread = Arc::clone(&result);
        self.emptying_trash = true;
        self.empty_trash_progress = (0, 0);
        self.empty_trash_result = Some(result);
        self.status = SharedString::from(t(self.locale, "trash.emptying"));
        cx.notify();

        // Progress Arc is polled from RootApp; Cancel calls session.cancel_empty_trash().
        let progress_poll = Arc::clone(&progress_ui);
        std::thread::spawn(move || {
            let outcome = session.empty_trash(&|done, total| {
                if let Ok(mut g) = progress_poll.lock() {
                    *g = (done, total);
                }
            });
            let mapped = outcome.map_err(|e| e.to_string());
            if let Ok(mut g) = result_thread.lock() {
                *g = Some(mapped);
            }
        });

        // Mirror progress into the entity on the UI poll loop via empty_trash_progress.
        let entity = cx.entity();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let cont = this
                    .update(cx, |this, cx| {
                        if let Ok(g) = progress_ui.lock() {
                            this.empty_trash_progress = *g;
                            if let Some(dialog) = this.empty_trash_dialog.as_mut() {
                                dialog.progress = *g;
                            }
                        }
                        if !this.emptying_trash {
                            return false;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !cont {
                    break;
                }
                let _ = &entity;
            }
        })
        .detach();
    }

    pub(crate) fn cancel_empty_trash(&mut self, cx: &mut Context<Self>) {
        self.session.cancel_empty_trash();
        if let Some(dialog) = self.empty_trash_dialog.as_mut() {
            dialog.cancelled = true;
        }
        self.status = SharedString::from(t(self.locale, "trash.emptyCancelled"));
        cx.notify();
    }

    pub(crate) fn open_add_tag_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let input = cx.new(|cx| InputState::new(window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "details.addTag")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Input::new(&input))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("tag-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let input = input.clone();
                                Button::new("tag-ok")
                                    .primary()
                                    .label(t(locale, "common.save"))
                                    .on_click(move |_, window, cx| {
                                        let name = input.read(cx).value().to_string();
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.add_tag_to_selection(name, cx);
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    pub(crate) fn open_edit_field_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        field: EditField,
    ) {
        let Some(item) = self.details.item.clone() else {
            return;
        };
        let locale = self.locale;
        let initial = match field {
            EditField::Name => item.name.clone(),
            EditField::Note => item.annotation.clone(),
            EditField::Link => item.url.clone(),
        };
        let title = match field {
            EditField::Name => t(locale, "details.name"),
            EditField::Note => t(locale, "details.note"),
            EditField::Link => t(locale, "details.link"),
        };
        let entity = cx.entity();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(initial));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(title.clone()).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(420.))
                    .child(Input::new(&input))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("edit-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let input = input.clone();
                                Button::new("edit-save")
                                    .primary()
                                    .label(t(locale, "common.save"))
                                    .on_click(move |_, window, cx| {
                                        let value = input.read(cx).value().to_string();
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            let edit = match field {
                                                EditField::Name => ItemEdit::Rename(value),
                                                EditField::Note => ItemEdit::SetAnnotation(value),
                                                EditField::Link => ItemEdit::SetUrl(value),
                                            };
                                            this.apply_edit_to_selection(edit, cx);
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    /// Searches for `text` unless the view already shows it (typing, then Enter, is one search).
    pub(crate) fn apply_search_if_changed(&mut self, text: String, cx: &mut Context<Self>) {
        self.search_generation = self.search_generation.wrapping_add(1);
        if self
            .library
            .as_ref()
            .is_some_and(|lib| lib.filters.search_text != text)
        {
            self.apply_search(text, cx);
        }
    }

    /// Searches as the user types: the newest text, [`SEARCH_PAUSE`] after the last key, so a
    /// word typed quickly is one search and not one per letter (D-179).
    pub(crate) fn schedule_search(&mut self, text: String, cx: &mut Context<Self>) {
        self.search_generation = self.search_generation.wrapping_add(1);
        let generation = self.search_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_PAUSE).await;
            this.update(cx, |this, cx| {
                if this.search_generation == generation {
                    this.apply_search_if_changed(text, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn apply_search(&mut self, text: String, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.filters.search_text = text;
            lib.selection.clear();
        }
        self.refresh_view(cx);
    }

    pub(crate) fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn clear_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.filters.clear();
            lib.selection.clear();
        }
        self.search_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.refresh_view(cx);
    }

    pub(crate) fn set_rating_filter(&mut self, rating: Option<u8>, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.filters.rating = rating;
            lib.selection.clear();
        }
        self.refresh_view(cx);
    }

    pub(crate) fn toggle_tag_filter(&mut self, tag: String, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            if let Some(pos) = lib.filters.tags.iter().position(|t| t == &tag) {
                lib.filters.tags.remove(pos);
            } else {
                lib.filters.tags.push(tag);
            }
            lib.selection.clear();
        }
        self.refresh_view(cx);
    }

    /// Opens or closes the name popover under a header's ⊕ (D-161, D-163). Opening fills in
    /// the default name, selected, so typing replaces it. A smart folder takes its rules from
    /// the view now; a view the rules cannot express is refused here, before the popover opens.
    pub(crate) fn set_name_prompt(
        &mut self,
        kind: Option<NamePromptKind>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.name_prompt = match kind {
            None => None,
            Some(NamePromptKind::NewFolder) => Some(NamePrompt::NewFolder),
            Some(NamePromptKind::SmartFolder) => match self.smart_draft() {
                Ok(draft) => Some(NamePrompt::SmartFolder(draft)),
                Err(error) => {
                    self.status = SharedString::from(format!("{error}"));
                    None
                }
            },
        };
        let default = match &self.name_prompt {
            Some(NamePrompt::NewFolder) => Some(t(self.locale, "folder.newTitle")),
            Some(NamePrompt::SmartFolder(_)) => Some(t(self.locale, "smart.defaultName")),
            None => None,
        };
        if let Some(name) = default {
            self.name_input.update(cx, |input, cx| {
                input.set_value(name, window, cx);
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        }
        cx.notify();
    }

    /// Creates what the open name popover is for, with the typed name.
    pub(crate) fn confirm_name_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().trim().to_string();
        let prompt = self.name_prompt.take();
        self.set_name_prompt(None, window, cx);
        if name.is_empty() {
            return;
        }
        match prompt {
            Some(NamePrompt::NewFolder) => self.create_folder_named(name, cx),
            Some(NamePrompt::SmartFolder(draft)) => self.save_smart_folder(name, draft, cx),
            None => {}
        }
    }

    /// The rules of the view now, for a new smart folder.
    fn smart_draft(&self) -> Result<desktop_session::SmartDraft, library_core::SourceError> {
        let Some(lib) = self.library.as_ref() else {
            return Err(library_core::SourceError::Invalid(
                "Open a library first.".into(),
            ));
        };
        let query = crate::library::query_for(
            &lib.place,
            &lib.browse,
            lib.sort,
            lib.ascending,
            &lib.filters,
        );
        self.session.smart_rules(&query)
    }

    fn save_smart_folder(
        &mut self,
        name: String,
        draft: desktop_session::SmartDraft,
        cx: &mut Context<Self>,
    ) {
        match self.session.save_smart_folder(SmartFolderChange::Create {
            name,
            parent: draft.parent,
            conditions: draft.conditions,
        }) {
            Ok(folders) => {
                if let Some(lib) = self.library.as_mut() {
                    lib.smart_folders = folders;
                }
                self.status = SharedString::from(t(self.locale, "smart.saved"));
            }
            Err(error) => self.status = SharedString::from(format!("{error}")),
        }
        cx.notify();
    }

    /// A folder inside the open folder, or at the root.
    fn create_folder_named(&mut self, name: String, cx: &mut Context<Self>) {
        let parent = self
            .library
            .as_ref()
            .and_then(|lib| lib.place.open_folder_id().map(str::to_owned));
        self.run_save_folder(
            FolderChange::Create {
                name,
                parent,
                description: String::new(),
                icon_color: None,
            },
            cx,
        );
    }

    pub(crate) fn create_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let parent = self
            .library
            .as_ref()
            .and_then(|lib| lib.place.open_folder_id().map(str::to_owned));
        let entity = cx.entity();
        let input = cx.new(|cx| InputState::new(window, cx).default_value("New folder"));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "folder.newTitle")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Input::new(&input))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("folder-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let input = input.clone();
                                let parent = parent.clone();
                                Button::new("folder-ok")
                                    .primary()
                                    .label(t(locale, "common.save"))
                                    .on_click(move |_, window, cx| {
                                        let name = input.read(cx).value().to_string();
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.run_save_folder(
                                                FolderChange::Create {
                                                    name: name.trim().to_string(),
                                                    parent: parent.clone(),
                                                    description: String::new(),
                                                    icon_color: None,
                                                },
                                                cx,
                                            );
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    pub(crate) fn rename_open_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let Some(id) = lib.place.open_folder_id().map(str::to_owned) else {
            self.status = SharedString::from(t(self.locale, "folder.selectFirst"));
            cx.notify();
            return;
        };
        let folder = find_folder(&lib.folders, &id);
        let Some(folder) = folder else {
            return;
        };
        let locale = self.locale;
        let entity = cx.entity();
        let name = folder.name.clone();
        let description = folder.description.clone();
        let tags = folder.tags.clone();
        let icon_color = folder.icon_color.clone();
        let order_by = folder.order_by;
        let sort_ascending = folder.sort_ascending;
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "folder.renameTitle")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Input::new(&input))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("rename-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let input = input.clone();
                                let id = id.clone();
                                let description = description.clone();
                                let tags = tags.clone();
                                let icon_color = icon_color.clone();
                                Button::new("rename-ok")
                                    .primary()
                                    .label(t(locale, "common.save"))
                                    .on_click(move |_, window, cx| {
                                        let name = input.read(cx).value().to_string();
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.run_save_folder(
                                                FolderChange::Update {
                                                    id: id.clone(),
                                                    name: name.trim().to_string(),
                                                    description: description.clone(),
                                                    tags: tags.clone(),
                                                    icon_color: icon_color.clone(),
                                                    order_by,
                                                    sort_ascending,
                                                },
                                                cx,
                                            );
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    pub(crate) fn delete_open_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let Some(id) = lib.place.open_folder_id().map(str::to_owned) else {
            self.status = SharedString::from(t(self.locale, "folder.selectFirst"));
            cx.notify();
            return;
        };
        let locale = self.locale;
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "folder.deleteTitle")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(400.))
                    .child(Label::new(t(locale, "folder.deleteMessage")))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("df-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let id = id.clone();
                                Button::new("df-keep")
                                    .label(t(locale, "folder.deleteKeep"))
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.run_delete_folder(&id, false, cx);
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                let id = id.clone();
                                Button::new("df-trash")
                                    .primary()
                                    .label(t(locale, "folder.deleteTrash"))
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.run_delete_folder(&id, true, cx);
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    fn run_save_folder(&mut self, change: FolderChange, cx: &mut Context<Self>) {
        match self.session.save_folder(change) {
            Ok(tree) => {
                if let Some(lib) = self.library.as_mut() {
                    lib.folders = tree;
                }
                self.reload_after_write(cx);
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
            }
        }
    }

    fn run_delete_folder(&mut self, id: &str, trash_items: bool, cx: &mut Context<Self>) {
        match self.session.delete_folder(id, trash_items) {
            Ok(tree) => {
                if let Some(lib) = self.library.as_mut() {
                    lib.folders = tree;
                    if matches!(lib.place, Place::Folder { id: ref open } if open == id) {
                        lib.place = Place::All;
                    }
                }
                self.reload_after_write(cx);
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn pick_add_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: Some(SharedString::from(t(self.locale, "commands.file.addFiles"))),
        });
        cx.spawn_in(window, async move |this, cx| {
            let paths = match rx.await {
                Ok(Ok(Some(paths))) => paths,
                _ => return,
            };
            let _ = this.update(cx, |this, cx| {
                this.start_import(paths, cx);
            });
        })
        .detach();
    }

    pub(crate) fn pick_import_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_import_folder_reviewed(window, cx);
    }

    pub(crate) fn paste_files(&mut self, cx: &mut Context<Self>) {
        let clipboard = OsClipboard;
        let folder = self
            .library
            .as_ref()
            .and_then(|lib| lib.place.open_folder_id().map(str::to_owned));
        match self.session.paste(&clipboard, folder) {
            Ok(Some(ticket)) => {
                self.status = SharedString::from(format!(
                    "{} #{}",
                    t(self.locale, "import.started"),
                    ticket.id
                ));
                self.imports_open = true;
                cx.notify();
            }
            Ok(None) => {
                self.status = SharedString::from(t(self.locale, "import.emptyClipboard"));
                cx.notify();
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn start_import(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        let folder = self
            .library
            .as_ref()
            .and_then(|lib| lib.place.open_folder_id().map(str::to_owned));
        let paths: Vec<String> = paths
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        match self.session.add_files_filtered(
            paths,
            folder,
            library_native::ImportFilter::default(),
        ) {
            Ok(ticket) => {
                self.status = SharedString::from(format!(
                    "{} #{}",
                    t(self.locale, "import.started"),
                    ticket.id
                ));
                self.imports_open = true;
                cx.notify();
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn handle_external_drop(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        if self.library.is_none() {
            return;
        }
        self.start_import(paths.0.iter().cloned().collect(), cx);
    }

    pub(crate) fn answer_duplicate(
        &mut self,
        id: u64,
        choice: DuplicateChoice,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.session.answer_import(id, choice) {
            self.status = SharedString::from(format!("{error}"));
        }
        self.pending_duplicate = None;
        cx.notify();
    }

    pub(crate) fn cancel_import(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Err(error) = self.session.cancel_import(id) {
            self.status = SharedString::from(format!("{error}"));
        }
        cx.notify();
    }

    pub(crate) fn unlock_folder_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let input = cx.new(|cx| InputState::new(window, cx).masked(true));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.unlockTitle")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "locks.password")))
                    .child(Input::new(&input))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("unlock-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let input = input.clone();
                                let folder_id = folder_id.clone();
                                Button::new("unlock-ok")
                                    .primary()
                                    .label(t(locale, "locks.unlock"))
                                    .on_click(move |_, window, cx| {
                                        let password = input.read(cx).value().to_string();
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            match this.session.unlock_folder(&folder_id, password) {
                                                Ok(()) => {
                                                    this.set_place(
                                                        Place::Folder {
                                                            id: folder_id.clone(),
                                                        },
                                                        cx,
                                                    );
                                                }
                                                Err(error) => {
                                                    this.status =
                                                        SharedString::from(format!("{error}"));
                                                    cx.notify();
                                                }
                                            }
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    pub(crate) fn lock_open_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let Some(id) = lib.place.open_folder_id().map(str::to_owned) else {
            self.status = SharedString::from(t(self.locale, "folder.selectFirst"));
            cx.notify();
            return;
        };
        let locale = self.locale;
        let entity = cx.entity();
        let input = cx.new(|cx| InputState::new(window, cx).masked(true));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.lockTitle")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "locks.password")))
                    .child(Input::new(&input))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("lock-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let input = input.clone();
                                let id = id.clone();
                                Button::new("lock-ok")
                                    .primary()
                                    .label(t(locale, "locks.lock"))
                                    .on_click(move |_, window, cx| {
                                        let password = input.read(cx).value().to_string();
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            match this.session.lock_folder(
                                                &id,
                                                password,
                                                None,
                                                None,
                                                &|_, _| {},
                                            ) {
                                                Ok(key) => {
                                                    this.status = SharedString::from(format!(
                                                        "{} — recovery: {}",
                                                        t(this.locale, "locks.locked"),
                                                        key.key
                                                    ));
                                                    if let Some(lib) = this.library.as_mut() {
                                                        lib.place = Place::All;
                                                    }
                                                    this.reload_after_write(cx);
                                                    this.offer_save_recovery(window, &key, cx);
                                                }
                                                Err(error) => {
                                                    this.status =
                                                        SharedString::from(format!("{error}"));
                                                    cx.notify();
                                                }
                                            }
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    pub(crate) fn lock_all_unlocked(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let unlocked: Vec<String> = crate::library::locked_folders(&lib.folders)
            .into_iter()
            .filter(|f| f.is_unlocked)
            .map(|f| f.id)
            .collect();
        let mut n = 0u32;
        for id in unlocked {
            if self.session.relock_folder(&id).is_ok() {
                n += 1;
            }
        }
        self.status = SharedString::from(t_n(self.locale, "locks.lockedCount", n));
        if let Some(lib) = self.library.as_mut()
            && matches!(lib.place, Place::Folder { .. })
        {
            // May have been viewing an unlocked folder that is now locked.
            lib.place = Place::All;
        }
        self.reload_after_write(cx);
    }

    pub(crate) fn open_library_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let locale = self.locale;
        let name = lib.info.name.clone();
        let stats = self.session.library_stats().ok();
        let backups = self.session.backups().unwrap_or_default();
        let entity = cx.entity();
        let rename = cx.new(|cx| InputState::new(window, cx).default_value(name.clone()));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "librarySettings.title")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(480.))
                    .child(Label::new(t(locale, "librarySettings.name")).font_bold())
                    .child(Input::new(&rename))
                    .child({
                        let entity = entity.clone();
                        let rename = rename.clone();
                        Button::new("lib-rename")
                            .outline()
                            .label(t(locale, "librarySettings.rename"))
                            .on_click(move |_, _, cx| {
                                let name = rename.read(cx).value().to_string();
                                entity.update(cx, |this, cx| {
                                    match this.session.rename_library(name.trim()) {
                                        Ok(info) => {
                                            if let Some(lib) = this.library.as_mut() {
                                                lib.info = info;
                                            }
                                            this.status = SharedString::from(t(
                                                this.locale,
                                                "librarySettings.renamed",
                                            ));
                                            cx.notify();
                                        }
                                        Err(error) => {
                                            this.status = SharedString::from(format!("{error}"));
                                            cx.notify();
                                        }
                                    }
                                });
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        Button::new("lib-reveal")
                            .outline()
                            .label(t(locale, "librarySettings.reveal"))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    match this.session.library_folder() {
                                        Ok(path) => {
                                            if let Err(error) =
                                                platform::open_in_file_manager(&path)
                                            {
                                                this.status = SharedString::from(error);
                                            }
                                        }
                                        Err(error) => {
                                            this.status = SharedString::from(format!("{error}"));
                                        }
                                    }
                                    cx.notify();
                                });
                            })
                    })
                    .when_some(stats, |el, stats| {
                        el.child(
                            Label::new(format!(
                                "{}: originals {} · thumbs {} · db {} · total {}",
                                t(locale, "librarySettings.stats"),
                                stats.originals,
                                stats.thumbnails,
                                stats.database,
                                stats.total()
                            ))
                            .text_sm(),
                        )
                    })
                    .child(Label::new(t(locale, "librarySettings.backups")).font_bold())
                    .children(backups.iter().take(8).map(|b| {
                        let name = b.name.clone();
                        let entity = entity.clone();
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Label::new(name.clone()).text_sm().flex_1())
                            .child(
                                Button::new(SharedString::from(format!("restore-{name}")))
                                    .ghost()
                                    .label(t(locale, "librarySettings.restore"))
                                    .on_click(move |_, window, cx| {
                                        let name = name.clone();
                                        entity.update(cx, |this, cx| {
                                            match this.session.restore_backup(&name) {
                                                Ok(info) => {
                                                    this.adopt_library(info, window, cx);
                                                }
                                                Err(error) => {
                                                    this.status =
                                                        SharedString::from(format!("{error}"));
                                                    cx.notify();
                                                }
                                            }
                                        });
                                    }),
                            )
                            .into_any_element()
                    }))
                    .child({
                        let entity = entity.clone();
                        Button::new("lib-backup")
                            .primary()
                            .label(t(locale, "librarySettings.backupNow"))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    match this.session.back_up_now() {
                                        Ok(backup) => {
                                            this.status = SharedString::from(format!(
                                                "{}: {}",
                                                t(this.locale, "librarySettings.backedUp"),
                                                backup.name
                                            ));
                                        }
                                        Err(error) => {
                                            this.status = SharedString::from(format!("{error}"));
                                        }
                                    }
                                    cx.notify();
                                });
                            })
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child({
                                let entity = entity.clone();
                                Button::new("lib-check")
                                    .outline()
                                    .label(t(locale, "librarySettings.check"))
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| {
                                            match this.session.check_library() {
                                                Ok(report) => {
                                                    this.status = SharedString::from(format!(
                                                        "check: missing={}, orphan={}",
                                                        report.missing_count, report.orphan_count
                                                    ));
                                                }
                                                Err(error) => {
                                                    this.status =
                                                        SharedString::from(format!("{error}"));
                                                }
                                            }
                                            cx.notify();
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Button::new("lib-rebuild")
                                    .outline()
                                    .label(t(locale, "librarySettings.rebuildThumbs"))
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| {
                                            match this.session.rebuild_thumbnails() {
                                                Ok(n) => {
                                                    this.status = SharedString::from(t_n(
                                                        this.locale,
                                                        "librarySettings.rebuilt",
                                                        n,
                                                    ));
                                                }
                                                Err(error) => {
                                                    this.status =
                                                        SharedString::from(format!("{error}"));
                                                }
                                            }
                                            cx.notify();
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    /// File → Save as Smart Folder: the same as the Smart folders ⊕, in a dialog, since the
    /// left bar may be hidden (D-163).
    pub(crate) fn save_as_smart_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = match self.smart_draft() {
            Ok(draft) => draft,
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
                return;
            }
        };
        let locale = self.locale;
        let entity = cx.entity();
        let input =
            cx.new(|cx| InputState::new(window, cx).default_value(t(locale, "smart.defaultName")));
        window.open_dialog(cx, move |dialog, _, _| {
            let entity = entity.clone();
            let input_ok = input.clone();
            let draft = draft.clone();
            dialog.title(t(locale, "smart.saveTitle")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Input::new(&input))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("smart-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child(
                                Button::new("smart-ok")
                                    .primary()
                                    .label(t(locale, "common.save"))
                                    .on_click(move |_, window, cx| {
                                        let name = input_ok.read(cx).value().trim().to_string();
                                        window.close_dialog(cx);
                                        if name.is_empty() {
                                            return;
                                        }
                                        let draft = draft.clone();
                                        entity.update(cx, |this, cx| {
                                            this.save_smart_folder(name, draft, cx);
                                        });
                                    }),
                            ),
                    ),
            )
        });
    }

    pub(crate) fn reload_after_write(&mut self, cx: &mut Context<Self>) {
        self.refresh_view(cx);
        self.refresh_details(cx);
        self.tag_vocabulary = self.session.tag_vocabulary().unwrap_or_default();
        self.tag_groups = self.session.tag_groups().unwrap_or_default();
        self.extensions = self.session.extensions().unwrap_or_default();
    }
}

#[derive(Clone, Copy)]
pub(crate) enum EditField {
    Name,
    Note,
    Link,
}

fn find_folder<'a>(
    folders: &'a [library_core::Folder],
    id: &str,
) -> Option<&'a library_core::Folder> {
    for folder in folders {
        if folder.id == id {
            return Some(folder);
        }
        if let Some(found) = find_folder(&folder.children, id) {
            return Some(found);
        }
    }
    None
}

struct OsClipboard;

impl desktop_session::Clipboard for OsClipboard {
    fn files(&self) -> Option<Vec<PathBuf>> {
        // Windows file-list clipboard is not exposed via arboard; OS drop / Add Files cover
        // the common paths. Paste image still works.
        None
    }

    fn image(&self) -> Option<(u32, u32, Vec<u8>)> {
        let mut clipboard = arboard::Clipboard::new().ok()?;
        let image = clipboard.get_image().ok()?;
        Some((
            image.width as u32,
            image.height as u32,
            image.bytes.into_owned(),
        ))
    }
}
