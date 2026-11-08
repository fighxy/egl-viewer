//! Phase 3.B–3.E organize surfaces: folder props, filters, tags, locks manage, import review.

use std::collections::HashSet;
use std::path::PathBuf;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::{StyledExt as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, IntoElement, ParentElement, PathPromptOptions, SharedString, Styled,
    Window, px,
};
use library_core::{
    Folder, FolderChange, PivPolicy, PivPrompt, SortKey, TagGroupChange, TagMode,
};
use library_native::ImportFilter;

use crate::app::RootApp;
use crate::i18n::{t, t_n};
use crate::library::Place;

impl RootApp {
    pub(crate) fn set_ext_filter(&mut self, ext: Option<String>, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.filters.ext = ext;
            lib.selection.clear();
        }
        self.refresh_view(cx);
    }

    pub(crate) fn set_shape_filter(&mut self, shape: Option<String>, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.filters.shape = shape;
            lib.selection.clear();
        }
        self.refresh_view(cx);
    }

    pub(crate) fn set_tag_mode(&mut self, mode: TagMode, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.filters.tag_mode = mode;
            lib.selection.clear();
        }
        self.refresh_view(cx);
    }

    pub(crate) fn set_folder_mode(&mut self, mode: TagMode, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.filters.folder_mode = mode;
            lib.selection.clear();
        }
        self.refresh_view(cx);
    }

    pub(crate) fn toggle_folder_filter(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            if let Some(pos) = lib.filters.folders.iter().position(|f| f == &id) {
                lib.filters.folders.remove(pos);
            } else {
                lib.filters.folders.push(id);
            }
            lib.selection.clear();
        }
        self.refresh_view(cx);
    }

    pub(crate) fn open_filter_type_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let extensions = self.extensions.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "filters.type")).child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .w(px(320.))
                    .max_h(px(420.))
                    .child({
                        let entity = entity.clone();
                        Button::new("type-any")
                            .ghost()
                            .label(t(locale, "filters.anyType"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| this.set_ext_filter(None, cx));
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        Button::new("type-image")
                            .ghost()
                            .label(t(locale, "filters.images"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.set_ext_filter(Some("image".into()), cx);
                                });
                            })
                    })
                    .children(extensions.clone().into_iter().map(|ext| {
                        let entity = entity.clone();
                        let name = ext.ext.clone();
                        Button::new(SharedString::from(format!("type-{name}")))
                            .ghost()
                            .label(format!(
                                "{} ({})",
                                name.to_uppercase(),
                                ext.count
                            ))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.set_ext_filter(Some(name.clone()), cx);
                                });
                            })
                    })),
            )
        });
    }

    pub(crate) fn open_filter_shape_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        const SHAPES: &[(&str, &str)] = &[
            ("square", "filters.square"),
            ("landscape", "filters.landscape"),
            ("portrait", "filters.portrait"),
            ("panoramic-landscape", "filters.panoramicLandscape"),
            ("panoramic-portrait", "filters.panoramicPortrait"),
        ];
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "filters.shape")).child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .w(px(320.))
                    .child({
                        let entity = entity.clone();
                        Button::new("shape-any")
                            .ghost()
                            .label(t(locale, "filters.anyShape"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| this.set_shape_filter(None, cx));
                            })
                    })
                    .children(SHAPES.iter().map(|(value, key)| {
                        let entity = entity.clone();
                        let value = (*value).to_owned();
                        Button::new(SharedString::from(format!("shape-{value}")))
                            .ghost()
                            .label(t(locale, key))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.set_shape_filter(Some(value.clone()), cx);
                                });
                            })
                    })),
            )
        });
    }

    pub(crate) fn open_tag_mode_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let modes = [
            (TagMode::Any, "filters.modeAny"),
            (TagMode::All, "filters.modeAll"),
            (TagMode::Exact, "filters.modeExact"),
            (TagMode::None, "filters.modeNone"),
        ];
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "filters.tagMode")).child(
                v_flex().gap_2().p_2().w(px(280.)).children(modes.into_iter().map(
                    |(mode, key)| {
                        let entity = entity.clone();
                        Button::new(SharedString::from(key))
                            .ghost()
                            .label(t(locale, key))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| this.set_tag_mode(mode, cx));
                            })
                    },
                )),
            )
        });
    }

    pub(crate) fn open_folder_filter_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let folders = self
            .library
            .as_ref()
            .map(|lib| flatten_folders(&lib.folders))
            .unwrap_or_default();
        let selected: HashSet<String> = self
            .library
            .as_ref()
            .map(|lib| lib.filters.folders.iter().cloned().collect())
            .unwrap_or_default();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "filters.folders")).child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .w(px(360.))
                    .max_h(px(420.))
                    .child({
                        let entity = entity.clone();
                        Button::new("folder-mode-any")
                            .ghost()
                            .label(t(locale, "filters.modeAny"))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.set_folder_mode(TagMode::Any, cx);
                                });
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        Button::new("folder-mode-all")
                            .ghost()
                            .label(t(locale, "filters.modeAll"))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.set_folder_mode(TagMode::All, cx);
                                });
                            })
                    })
                    .children(folders.clone().into_iter().map(|(id, name, depth)| {
                        let entity = entity.clone();
                        let on = selected.contains(&id);
                        let label = format!(
                            "{}{}{}",
                            "  ".repeat(depth as usize),
                            if on { "✓ " } else { "" },
                            name
                        );
                        Button::new(SharedString::from(format!("ff-{id}")))
                            .ghost()
                            .label(label)
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.toggle_folder_filter(id.clone(), cx);
                                });
                            })
                    })),
            )
        });
    }

    pub(crate) fn open_tag_groups_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let groups = self.tag_groups.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "tags.groups")).child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .w(px(420.))
                    .max_h(px(480.))
                    .child(if groups.is_empty() {
                        Label::new(t(locale, "tags.noGroups")).into_any_element()
                    } else {
                        v_flex()
                            .gap_2()
                            .children(groups.clone().into_iter().map(|group| {
                                let entity_del = entity.clone();
                                let entity_apply = entity.clone();
                                let id = group.id.clone();
                                let tags = group.tags.clone();
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Label::new(format!(
                                            "{} ({})",
                                            group.name,
                                            group.tags.len()
                                        ))
                                        .text_sm()
                                        .flex_1(),
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!("tg-apply-{id}")))
                                            .ghost()
                                            .label(t(locale, "tags.applyGroup"))
                                            .on_click(move |_, window, cx| {
                                                window.close_dialog(cx);
                                                entity_apply.update(cx, |this, cx| {
                                                    if let Some(lib) = this.library.as_mut() {
                                                        lib.filters.tags = tags.clone();
                                                        lib.selection.clear();
                                                    }
                                                    this.refresh_view(cx);
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!("tg-del-{id}")))
                                            .ghost()
                                            .label(t(locale, "common.delete"))
                                            .on_click(move |_, window, cx| {
                                                window.close_dialog(cx);
                                                entity_del.update(cx, |this, cx| {
                                                    match this.session.delete_tag_group(&id) {
                                                        Ok(groups) => {
                                                            this.tag_groups = groups;
                                                            this.status = SharedString::from(t(
                                                                this.locale,
                                                                "tags.groupDeleted",
                                                            ));
                                                            cx.notify();
                                                        }
                                                        Err(error) => {
                                                            this.status = SharedString::from(
                                                                format!("{error}"),
                                                            );
                                                            cx.notify();
                                                        }
                                                    }
                                                });
                                            }),
                                    )
                                    .into_any_element()
                            }))
                            .into_any_element()
                    })
                    .child({
                        let entity = entity.clone();
                        Button::new("tg-new")
                            .primary()
                            .label(t(locale, "tags.newGroup"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.create_tag_group_dialog(window, cx);
                                });
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        Button::new("tg-rename-tag")
                            .ghost()
                            .label(t(locale, "tags.rename"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.rename_tag_dialog(window, cx);
                                });
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        Button::new("tg-merge-tag")
                            .ghost()
                            .label(t(locale, "tags.merge"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.merge_tag_dialog(window, cx);
                                });
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        Button::new("tg-delete-tag")
                            .ghost()
                            .label(t(locale, "tags.delete"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.delete_tag_dialog(window, cx);
                                });
                            })
                    }),
            )
        });
    }

    fn create_tag_group_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let name = cx.new(|cx| InputState::new(window, cx).default_value("New group"));
        let tags = cx.new(|cx| InputState::new(window, cx).placeholder("tag1, tag2"));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "tags.newGroup")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "tags.groupName")))
                    .child(Input::new(&name))
                    .child(Label::new(t(locale, "tags.groupTags")))
                    .child(Input::new(&tags))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("tg-create-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let name = name.clone();
                                let tags = tags.clone();
                                Button::new("tg-create-ok")
                                    .primary()
                                    .label(t(locale, "common.save"))
                                    .on_click(move |_, window, cx| {
                                        let group_name = name.read(cx).value().to_string();
                                        let tag_list = tags
                                            .read(cx)
                                            .value()
                                            .split(',')
                                            .map(str::trim)
                                            .filter(|s| !s.is_empty())
                                            .map(str::to_owned)
                                            .collect::<Vec<_>>();
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            match this.session.save_tag_group(TagGroupChange::Create {
                                                name: group_name,
                                                color: None,
                                                tags: tag_list,
                                            }) {
                                                Ok(groups) => {
                                                    this.tag_groups = groups;
                                                    this.status = SharedString::from(t(
                                                        this.locale,
                                                        "tags.groupSaved",
                                                    ));
                                                    cx.notify();
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

    fn rename_tag_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let from = cx.new(|cx| InputState::new(window, cx));
        let to = cx.new(|cx| InputState::new(window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "tags.rename")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "tags.from")))
                    .child(Input::new(&from))
                    .child(Label::new(t(locale, "tags.to")))
                    .child(Input::new(&to))
                    .child(dialog_ok_cancel(
                        locale,
                        "tag-rename",
                        entity.clone(),
                        {
                            let from = from.clone();
                            let to = to.clone();
                            move |this, cx| {
                            let from = from.read(cx).value().to_string();
                            let to = to.read(cx).value().to_string();
                            match this.session.rename_tag(&from, &to) {
                                Ok(()) => {
                                    this.status =
                                        SharedString::from(t(this.locale, "tags.renamed"));
                                    this.reload_after_write(cx);
                                }
                                Err(error) => {
                                    this.status = SharedString::from(format!("{error}"));
                                    cx.notify();
                                }
                            }
                        }
                        },
                    )),
            )
        });
    }

    fn merge_tag_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let from = cx.new(|cx| InputState::new(window, cx));
        let into = cx.new(|cx| InputState::new(window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "tags.merge")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "tags.from")))
                    .child(Input::new(&from))
                    .child(Label::new(t(locale, "tags.into")))
                    .child(Input::new(&into))
                    .child(dialog_ok_cancel(
                        locale,
                        "tag-merge",
                        entity.clone(),
                        {
                            let from = from.clone();
                            let into = into.clone();
                            move |this, cx| {
                            let from = from.read(cx).value().to_string();
                            let into = into.read(cx).value().to_string();
                            match this.session.merge_tags(&from, &into) {
                                Ok(()) => {
                                    this.status =
                                        SharedString::from(t(this.locale, "tags.merged"));
                                    this.reload_after_write(cx);
                                }
                                Err(error) => {
                                    this.status = SharedString::from(format!("{error}"));
                                    cx.notify();
                                }
                            }
                        }
                        },
                    )),
            )
        });
    }

    fn delete_tag_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let name = cx.new(|cx| InputState::new(window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "tags.delete")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "tags.from")))
                    .child(Input::new(&name))
                    .child(dialog_ok_cancel(
                        locale,
                        "tag-delete",
                        entity.clone(),
                        {
                            let name = name.clone();
                            move |this, cx| {
                            let name = name.read(cx).value().to_string();
                            match this.session.delete_tag(&name) {
                                Ok(()) => {
                                    this.status =
                                        SharedString::from(t(this.locale, "tags.deleted"));
                                    this.reload_after_write(cx);
                                }
                                Err(error) => {
                                    this.status = SharedString::from(format!("{error}"));
                                    cx.notify();
                                }
                            }
                        }
                        },
                    )),
            )
        });
    }

    pub(crate) fn update_open_folder(
        &mut self,
        change: FolderChange,
        cx: &mut Context<Self>,
    ) {
        match self.session.save_folder(change) {
            Ok(tree) => {
                if let Some(lib) = self.library.as_mut() {
                    lib.folders = tree;
                    // Apply folder sort when set.
                    if let Place::Folder { id } = &lib.place {
                        if let Some(folder) = find_folder_ref(&lib.folders, id) {
                            if let Some(sort) = folder.order_by {
                                lib.sort = sort;
                            }
                            if let Some(asc) = folder.sort_ascending {
                                lib.ascending = asc;
                            }
                        }
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

    pub(crate) fn set_folder_sort(
        &mut self,
        folder_id: String,
        order_by: Option<SortKey>,
        sort_ascending: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        let Some(folder) = self
            .library
            .as_ref()
            .and_then(|lib| find_folder_ref(&lib.folders, &folder_id).cloned())
        else {
            return;
        };
        self.update_open_folder(
            FolderChange::Update {
                id: folder_id,
                name: folder.name,
                description: folder.description,
                tags: folder.tags,
                icon_color: folder.icon_color,
                order_by,
                sort_ascending,
            },
            cx,
        );
    }

    pub(crate) fn set_folder_color(
        &mut self,
        folder_id: String,
        icon_color: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(folder) = self
            .library
            .as_ref()
            .and_then(|lib| find_folder_ref(&lib.folders, &folder_id).cloned())
        else {
            return;
        };
        self.update_open_folder(
            FolderChange::Update {
                id: folder_id,
                name: folder.name,
                description: folder.description,
                tags: folder.tags,
                icon_color,
                order_by: folder.order_by,
                sort_ascending: folder.sort_ascending,
            },
            cx,
        );
    }

    pub(crate) fn edit_folder_description(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let Some(folder) = self
            .library
            .as_ref()
            .and_then(|lib| find_folder_ref(&lib.folders, &folder_id).cloned())
        else {
            return;
        };
        let locale = self.locale;
        let entity = cx.entity();
        let input = cx.new(|cx| {
            InputState::new(window, cx).default_value(folder.description.clone())
        });
        let folder_snap = folder.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "folder.description")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Input::new(&input))
                    .child(dialog_ok_cancel(
                        locale,
                        "folder-desc",
                        entity.clone(),
                        {
                            let input = input.clone();
                            let folder_id = folder_id.clone();
                            let folder_snap = folder_snap.clone();
                            move |this, cx| {
                            let description = input.read(cx).value().to_string();
                            this.update_open_folder(
                                FolderChange::Update {
                                    id: folder_id.clone(),
                                    name: folder_snap.name.clone(),
                                    description,
                                    tags: folder_snap.tags.clone(),
                                    icon_color: folder_snap.icon_color.clone(),
                                    order_by: folder_snap.order_by,
                                    sort_ascending: folder_snap.sort_ascending,
                                },
                                cx,
                            );
                        }
                        },
                    )),
            )
        });
    }

    pub(crate) fn open_lock_manage_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let yubi = desktop_session::yubikey::present();
        let wraps = self.session.piv_wraps(&folder_id).unwrap_or_default();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.manageTitle")).child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .w(px(400.))
                    .child({
                        let entity = entity.clone();
                        let id = folder_id.clone();
                        Button::new("lock-change-pw")
                            .ghost()
                            .label(t(locale, "locks.changePassword"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.change_password_dialog(window, cx, id.clone());
                                });
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        let id = folder_id.clone();
                        Button::new("lock-recover")
                            .ghost()
                            .label(t(locale, "locks.recover"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.recover_folder_dialog(window, cx, id.clone());
                                });
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        let id = folder_id.clone();
                        Button::new("lock-renew")
                            .ghost()
                            .label(t(locale, "locks.renewRecovery"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.renew_recovery_dialog(window, cx, id.clone());
                                });
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        let id = folder_id.clone();
                        Button::new("lock-remove")
                            .ghost()
                            .label(t(locale, "locks.remove"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.remove_lock_dialog(window, cx, id.clone());
                                });
                            })
                    })
                    .child({
                        let entity = entity.clone();
                        let id = folder_id.clone();
                        Button::new("lock-label")
                            .ghost()
                            .label(t(locale, "locks.setLabel"))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.set_lock_label_dialog(window, cx, id.clone());
                                });
                            })
                    })
                    .when(yubi, |el| {
                        let unlock_entity = entity.clone();
                        let unlock_id = folder_id.clone();
                        let bind_entity = entity.clone();
                        let bind_id = folder_id.clone();
                        el.child(
                            Button::new("lock-yubi-unlock")
                                .ghost()
                                .label(t(locale, "locks.unlockYubi"))
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    unlock_entity.update(cx, |this, cx| {
                                        this.unlock_yubi_dialog(window, cx, unlock_id.clone());
                                    });
                                }),
                        )
                        .child(
                            Button::new("lock-yubi-bind")
                                .ghost()
                                .label(t(locale, "locks.bindYubi"))
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    bind_entity.update(cx, |this, cx| {
                                        this.bind_yubi_dialog(window, cx, bind_id.clone());
                                    });
                                }),
                        )
                    })
                    .children(wraps.clone().into_iter().map(|wrap| {
                        let entity = entity.clone();
                        let id = folder_id.clone();
                        let serial = wrap.serial.clone();
                        Button::new(SharedString::from(format!("lock-yubi-unbind-{serial}")))
                            .ghost()
                            .label(format!("{} {serial}", t(locale, "locks.unbindYubi")))
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                entity.update(cx, |this, cx| {
                                    this.unbind_yubi_dialog(window, cx, id.clone(), serial.clone());
                                });
                            })
                    })),
            )
        });
    }

    fn change_password_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let current = cx.new(|cx| InputState::new(window, cx).masked(true));
        let next = cx.new(|cx| InputState::new(window, cx).masked(true));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.changePassword")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "locks.currentPassword")))
                    .child(Input::new(&current))
                    .child(Label::new(t(locale, "locks.newPassword")))
                    .child(Input::new(&next))
                    .child(dialog_ok_cancel(
                        locale,
                        "change-pw",
                        entity.clone(),
                        {
                            let current = current.clone();
                            let next = next.clone();
                            let folder_id = folder_id.clone();
                            move |this, cx| {
                            let current = current.read(cx).value().to_string();
                            let next = next.read(cx).value().to_string();
                            match this
                                .session
                                .change_folder_password(&folder_id, current, next)
                            {
                                Ok(()) => {
                                    this.status =
                                        SharedString::from(t(this.locale, "locks.passwordChanged"));
                                    cx.notify();
                                }
                                Err(error) => {
                                    this.status = SharedString::from(format!("{error}"));
                                    cx.notify();
                                }
                            }
                        }
                        },
                    )),
            )
        });
    }

    fn recover_folder_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let recovery = cx.new(|cx| InputState::new(window, cx));
        let next = cx.new(|cx| InputState::new(window, cx).masked(true));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.recover")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "locks.recoveryKey")))
                    .child(Input::new(&recovery))
                    .child(Label::new(t(locale, "locks.newPassword")))
                    .child(Input::new(&next))
                    .child(dialog_ok_cancel(
                        locale,
                        "recover",
                        entity.clone(),
                        {
                            let recovery = recovery.clone();
                            let next = next.clone();
                            let folder_id = folder_id.clone();
                            move |this, cx| {
                            let recovery = recovery.read(cx).value().to_string();
                            let next = next.read(cx).value().to_string();
                            match this.session.recover_folder(&folder_id, &recovery, next) {
                                Ok(()) => {
                                    this.status =
                                        SharedString::from(t(this.locale, "locks.recovered"));
                                    this.reload_after_write(cx);
                                }
                                Err(error) => {
                                    this.status = SharedString::from(format!("{error}"));
                                    cx.notify();
                                }
                            }
                        }
                        },
                    )),
            )
        });
    }

    fn renew_recovery_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let current = cx.new(|cx| InputState::new(window, cx).masked(true));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.renewRecovery")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "locks.currentPassword")))
                    .child(Input::new(&current))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("renew-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let current = current.clone();
                                let folder_id = folder_id.clone();
                                Button::new("renew-ok")
                                    .primary()
                                    .label(t(locale, "common.save"))
                                    .on_click(move |_, window, cx| {
                                        let current = current.read(cx).value().to_string();
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            match this
                                                .session
                                                .renew_recovery_key(&folder_id, current)
                                            {
                                                Ok(key) => {
                                                    this.status = SharedString::from(format!(
                                                        "{} — {}",
                                                        t(this.locale, "locks.renewed"),
                                                        key.key
                                                    ));
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

    fn remove_lock_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let password = cx.new(|cx| InputState::new(window, cx).masked(true));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.remove")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "locks.password")))
                    .child(Input::new(&password))
                    .child(dialog_ok_cancel(
                        locale,
                        "remove-lock",
                        entity.clone(),
                        {
                            let password = password.clone();
                            let folder_id = folder_id.clone();
                            move |this, cx| {
                            let password = password.read(cx).value().to_string();
                            match this
                                .session
                                .remove_folder_lock(&folder_id, password, &|_, _| {})
                            {
                                Ok(()) => {
                                    this.status =
                                        SharedString::from(t(this.locale, "locks.removed"));
                                    this.reload_after_write(cx);
                                }
                                Err(error) => {
                                    this.status = SharedString::from(format!("{error}"));
                                    cx.notify();
                                }
                            }
                        }
                        },
                    )),
            )
        });
    }

    fn set_lock_label_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let input = cx.new(|cx| InputState::new(window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.setLabel")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Input::new(&input))
                    .child(dialog_ok_cancel(
                        locale,
                        "lock-label",
                        entity.clone(),
                        {
                            let input = input.clone();
                            let folder_id = folder_id.clone();
                            move |this, cx| {
                                let label = input.read(cx).value().to_string();
                                let label = if label.trim().is_empty() {
                                    None
                                } else {
                                    Some(label)
                                };
                                match this.session.set_lock_label(&folder_id, label, None) {
                                    Ok(()) => {
                                        this.reload_after_write(cx);
                                    }
                                    Err(error) => {
                                        this.status = SharedString::from(format!("{error}"));
                                        cx.notify();
                                    }
                                }
                            }
                        },
                    )),
            )
        });
    }

    fn unlock_yubi_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let wraps = self.session.piv_wraps(&folder_id).unwrap_or_default();
        if wraps.is_empty() {
            self.status = SharedString::from(t(locale, "locks.noYubi"));
            cx.notify();
            return;
        }
        let serial = wraps[0].serial.clone();
        let policy = wraps[0].policy;
        let pin_input = cx.new(|cx| InputState::new(window, cx).masked(true));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.unlockYubi")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(format!("YubiKey {serial}")))
                    .when(matches!(policy, PivPolicy::Pin), |el| {
                        el.child(Label::new(t(locale, "locks.yubiPin")))
                            .child(Input::new(&pin_input))
                    })
                    .child(dialog_ok_cancel(
                        locale,
                        "yubi-unlock",
                        entity.clone(),
                        {
                            let pin_input = pin_input.clone();
                            let folder_id = folder_id.clone();
                            let serial = serial.clone();
                            move |this, cx| {
                            let prompt = match policy {
                                PivPolicy::Touch => PivPrompt::Touch,
                                PivPolicy::Pin => PivPrompt::Pin {
                                    pin: pin_input.read(cx).value().to_string(),
                                },
                            };
                            match this
                                .session
                                .unlock_with_yubikey(&folder_id, serial.clone(), prompt)
                            {
                                Ok(()) => {
                                    this.set_place(
                                        Place::Folder {
                                            id: folder_id.clone(),
                                        },
                                        cx,
                                    );
                                }
                                Err(error) => {
                                    this.status = SharedString::from(format!("{error}"));
                                    cx.notify();
                                }
                            }
                        }
                        },
                    )),
            )
        });
    }

    fn bind_yubi_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let password = cx.new(|cx| InputState::new(window, cx).masked(true));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.bindYubi")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "locks.password")))
                    .child(Input::new(&password))
                    .child(dialog_ok_cancel(
                        locale,
                        "yubi-bind",
                        entity.clone(),
                        {
                            let password = password.clone();
                            let folder_id = folder_id.clone();
                            move |this, cx| {
                            let password = password.read(cx).value().to_string();
                            match this.session.bind_yubikey(
                                &folder_id,
                                password,
                                PivPolicy::Touch,
                                None,
                            ) {
                                Ok(()) => {
                                    this.status =
                                        SharedString::from(t(this.locale, "locks.yubiBound"));
                                    cx.notify();
                                }
                                Err(error) => {
                                    this.status = SharedString::from(format!("{error}"));
                                    cx.notify();
                                }
                            }
                        }
                        },
                    )),
            )
        });
    }

    fn unbind_yubi_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        folder_id: String,
        serial: String,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let password = cx.new(|cx| InputState::new(window, cx).masked(true));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.unbindYubi")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(360.))
                    .child(Label::new(t(locale, "locks.password")))
                    .child(Input::new(&password))
                    .child(dialog_ok_cancel(
                        locale,
                        "yubi-unbind",
                        entity.clone(),
                        {
                            let password = password.clone();
                            let folder_id = folder_id.clone();
                            let serial = serial.clone();
                            move |this, cx| {
                            let password = password.read(cx).value().to_string();
                            match this
                                .session
                                .unbind_yubikey(&folder_id, password, serial.clone())
                            {
                                Ok(()) => {
                                    this.status =
                                        SharedString::from(t(this.locale, "locks.yubiUnbound"));
                                    cx.notify();
                                }
                                Err(error) => {
                                    this.status = SharedString::from(format!("{error}"));
                                    cx.notify();
                                }
                            }
                        }
                        },
                    )),
            )
        });
    }

    /// After lock / renew: offer saving the recovery key outside the library.
    pub(crate) fn offer_save_recovery(
        &mut self,
        window: &mut Window,
        key: &library_core::RecoveryKey,
        cx: &mut Context<Self>,
    ) {
        let locale = self.locale;
        let entity = cx.entity();
        let key = key.clone();
        let folder_name = self
            .library
            .as_ref()
            .map(|lib| lib.info.name.clone())
            .unwrap_or_else(|| "folder".into());
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "locks.saveRecovery")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(420.))
                    .child(Label::new(t(locale, "locks.saveRecoveryBody")))
                    .child(Label::new(key.key.clone()).text_sm())
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("recovery-skip")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let key = key.clone();
                                let folder_name = folder_name.clone();
                                Button::new("recovery-save")
                                    .primary()
                                    .label(t(locale, "locks.saveToFile"))
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.pick_save_recovery(
                                                window,
                                                folder_name.clone(),
                                                key.clone(),
                                                cx,
                                            );
                                        });
                                    })
                            }),
                    ),
            )
        });
    }

    fn pick_save_recovery(
        &mut self,
        window: &mut Window,
        folder: String,
        key: library_core::RecoveryKey,
        cx: &mut Context<Self>,
    ) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t(self.locale, "locks.saveToFile"))),
        });
        cx.spawn_in(window, async move |this, cx| {
            let path = match rx.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            if let Some(path) = path {
                let _ = this.update(cx, |this, cx| {
                    match this.session.save_recovery_file(
                        &path.to_string_lossy(),
                        &folder,
                        &key,
                    ) {
                        Ok(()) => {
                            this.status =
                                SharedString::from(t(this.locale, "locks.recoverySaved"));
                            cx.notify();
                        }
                        Err(error) => {
                            this.status = SharedString::from(format!("{error}"));
                            cx.notify();
                        }
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn pick_import_folder_reviewed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from(t(
                self.locale,
                "commands.file.importFolder",
            ))),
        });
        cx.spawn_in(window, async move |this, cx| {
            let path = match rx.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let Some(path) = path else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.show_import_preview(window, path, cx);
            });
        })
        .detach();
    }

    fn show_import_preview(
        &mut self,
        window: &mut Window,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let path_str = path.to_string_lossy().into_owned();
        let preview = match self.session.preview_import(&path_str) {
            Ok(preview) => preview,
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
                return;
            }
        };
        let locale = self.locale;
        let entity = cx.entity();
        let files = preview.files;
        let bytes = preview.bytes;
        let unlisted = preview.unlisted;
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "import.reviewTitle")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(420.))
                    .child(Label::new(path_str.clone()).text_sm())
                    .child(Label::new(format!(
                        "{} · {} · {}",
                        t_n(locale, "import.reviewFiles", files),
                        format_bytes(bytes),
                        if unlisted > 0 {
                            t_n(locale, "import.reviewUnlisted", unlisted)
                        } else {
                            String::new()
                        }
                    )))
                    .child(Label::new(t(locale, "import.reviewHint")).text_xs())
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("import-review-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(|_, window, cx| window.close_dialog(cx)),
                            )
                            .child({
                                let entity = entity.clone();
                                let path = path_str.clone();
                                Button::new("import-review-all")
                                    .ghost()
                                    .label(t(locale, "import.importAll"))
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.start_import_filtered(
                                                vec![PathBuf::from(&path)],
                                                ImportFilter {
                                                    reviewed: false,
                                                    ..ImportFilter::default()
                                                },
                                                cx,
                                            );
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                let path_str = path_str.clone();
                                Button::new("import-review-images")
                                    .primary()
                                    .label(t(locale, "import.importImages"))
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        entity.update(cx, |this, cx| {
                                            this.start_import_filtered(
                                                vec![PathBuf::from(&path_str)],
                                                ImportFilter {
                                                    reviewed: true,
                                                    exclude: HashSet::new(),
                                                    include: HashSet::new(),
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

    pub(crate) fn start_import_filtered(
        &mut self,
        paths: Vec<PathBuf>,
        filter: ImportFilter,
        cx: &mut Context<Self>,
    ) {
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
        match self.session.add_files_filtered(paths, folder, filter) {
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
}

fn dialog_ok_cancel<F>(
    locale: app_store::Locale,
    id: &'static str,
    entity: gpui_kit::Entity<RootApp>,
    on_ok: F,
) -> impl IntoElement
where
    F: Fn(&mut RootApp, &mut Context<RootApp>) + Clone + 'static,
{
    let on_ok = on_ok.clone();
    h_flex()
        .gap_2()
        .justify_end()
        .child(
            Button::new(SharedString::from(format!("{id}-cancel")))
                .label(t(locale, "common.cancel"))
                .on_click(|_, window, cx| window.close_dialog(cx)),
        )
        .child(
            Button::new(SharedString::from(format!("{id}-ok")))
                .primary()
                .label(t(locale, "common.save"))
                .on_click(move |_, window, cx| {
                    window.close_dialog(cx);
                    entity.update(cx, |this, cx| on_ok(this, cx));
                }),
        )
}

fn flatten_folders(folders: &[Folder]) -> Vec<(String, String, u32)> {
    let mut out = Vec::new();
    fn walk(folders: &[Folder], depth: u32, out: &mut Vec<(String, String, u32)>) {
        for folder in folders {
            out.push((folder.id.clone(), folder.name.clone(), depth));
            walk(&folder.children, depth + 1, out);
        }
    }
    walk(folders, 0, &mut out);
    out
}

fn find_folder_ref<'a>(folders: &'a [Folder], id: &str) -> Option<&'a Folder> {
    for folder in folders {
        if folder.id == id {
            return Some(folder);
        }
        if let Some(found) = find_folder_ref(&folder.children, id) {
            return Some(found);
        }
    }
    None
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}
