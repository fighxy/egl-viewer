//! Writes: adding files, the trash, folders, tags, edits with undo, smart folders and locked
//! folders. Every write goes through the service, so the open views follow and the app hears
//! `viewsInvalidated` (D-092).

use std::os::raw::{c_char, c_int};
use std::path::PathBuf;

use app_service::ItemsTarget;
use library_core::{FolderChange, ImportKind, ItemEdit, ItemId, SmartFolderChange, TagGroupChange};
use library_native::{DuplicateChoice, ImportFilter, make_pending_previews};
use serde::{Deserialize, Serialize};

use crate::read::{QueryTarget, query_target_ids};
use crate::{
    EglLibrary, EglStatus, arg, fail, fail_source, optional, parse, reply, reply_count, reply_unit,
    tri, with_lib,
};

// ---------------------------------------------------------------------------------------------
// Adding files

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct AddRequest {
    paths: Vec<String>,
    folder: Option<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    reviewed: bool,
}

/// Starts adding files and folders (walked recursively) in a background task; thumbnails are
/// made by the same task. `request_json`:
/// `{"paths":["/abs/a.png","/abs/dir"],"folder":null,"exclude":[],"include":[],"reviewed":false}`.
/// `*out_json` is `{"id":N}`; progress comes as `importProgress` events, or poll
/// `egl_import_status`.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_add_files(
    lib: *mut EglLibrary,
    request_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let request: AddRequest = tri!(parse(handle, request_json, "add request JSON"));
            if request.paths.is_empty() {
                return fail(handle, EglStatus::Invalid, "paths must not be empty");
            }
            let filter = ImportFilter {
                exclude: request.exclude.into_iter().collect(),
                include: request.include.into_iter().collect(),
                reviewed: request.reviewed,
            };
            let paths = request.paths.into_iter().map(PathBuf::from).collect();
            let ticket = handle.service.add_files(paths, request.folder, filter);
            reply(handle, out_json, ticket, "import ticket")
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EagleImportRequest {
    path: String,
    #[serde(default)]
    import_trash: bool,
    #[serde(default)]
    under_root_folder: bool,
}

/// Imports an Eagle library folder into this library in a background task (docs/spec.md §8.2,
/// D-116). `request_json`: `{"path":"/abs/Name.library","importTrash":false,
/// "underRootFolder":false}`; `underRootFolder` puts the Eagle folders under a new root folder
/// named after the Eagle library (for a library already in use). `*out_json` is `{"id":N}`;
/// progress comes as `importProgress` events (kind `eagle`), the last one with the report.
/// The Eagle folder is only read. A folder that is not an Eagle library is
/// `EGL_ERR_NOT_A_LIBRARY`.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_import_eagle(
    lib: *mut EglLibrary,
    request_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let request: EagleImportRequest =
                tri!(parse(handle, request_json, "Eagle import request JSON"));
            let ticket = handle.service.import_eagle(
                std::path::Path::new(&request.path),
                request.import_trash,
                request.under_root_folder,
            );
            reply(handle, out_json, ticket, "import ticket")
        })
    }
}

/// Answers the duplicate question of import `id` (state `waitingForDecision`). `answer_utf8`:
/// `skip` | `addAnyway` | `addToFolder` | `cancel`. The first answer holds for the batch.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_answer_import(
    lib: *mut EglLibrary,
    id: u64,
    answer_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and string.
    unsafe {
        with_lib(lib, |handle| {
            let choice = match tri!(arg(handle, answer_utf8, "answer")) {
                "skip" => DuplicateChoice::Skip,
                "addAnyway" => DuplicateChoice::AddAnyway,
                "addToFolder" => DuplicateChoice::AddToFolder,
                "cancel" => DuplicateChoice::Cancel,
                other => {
                    return fail(
                        handle,
                        EglStatus::Invalid,
                        format!("unknown duplicate answer: {other}"),
                    );
                }
            };
            reply_unit(handle, handle.service.answer_import(id, choice))
        })
    }
}

/// Stops import `id` after the current file; added files stay.
///
/// # Safety
/// `lib` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_cancel_import(lib: *mut EglLibrary, id: u64) -> EglStatus {
    // SAFETY: the caller's handle.
    unsafe {
        with_lib(lib, |handle| {
            reply_unit(handle, handle.service.cancel_import(id))
        })
    }
}

/// Every import: running first (newest first), then the last finished ones:
/// `ImportProgress[]`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_imports(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(
                handle,
                out_json,
                Ok(handle.service.imports().list()),
                "imports",
            )
        })
    }
}

/// The progress of import `id`: an `ImportProgress`, or `null` while it started but has not
/// reported yet. `EGL_ERR_NOT_FOUND` for an unknown id.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_import_status(
    lib: *mut EglLibrary,
    id: u64,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| match handle.service.imports().status(id) {
            Some(progress) => reply(handle, out_json, Ok(progress), "import progress"),
            None => fail(handle, EglStatus::NotFound, "no import with this id"),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewTicket {
    /// The preview task, or `null` when one already runs.
    id: Option<u64>,
    /// Items waiting for a thumbnail after a rebuild (0 without `rebuild`).
    waiting: u32,
}

/// Makes the thumbnails a crash or a cancelled import left undone, in the background. With
/// `rebuild != 0` every thumbnail is made again (Rebuild Thumbnails).
/// `*out_json`: `{"id":N|null,"waiting":N}`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_generate_previews(
    lib: *mut EglLibrary,
    rebuild: c_int,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let service = &handle.service;
            let library = tri!(
                service
                    .writable_library()
                    .map_err(|err| fail_source(handle, err))
            );
            let waiting = if rebuild != 0 {
                tri!(
                    library
                        .rebuild_previews()
                        .map_err(|err| fail_source(handle, err))
                )
            } else {
                0
            };
            if rebuild != 0 {
                service.refresh_views();
            }
            let id = if service.imports().is_running(ImportKind::Previews) {
                None
            } else {
                Some(tri!(
                    service
                        .start(library, ImportKind::Previews, make_pending_previews)
                        .map_err(|err| fail_source(handle, err))
                ))
            };
            reply(
                handle,
                out_json,
                Ok(PreviewTicket { id, waiting }),
                "preview ticket",
            )
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Edits and undo

/// The ids of a stateless selection (`{"ids":[…],"all":bool,"query":LibraryQuery|null}`).
///
/// # Safety
/// As `parse`.
unsafe fn query_target(handle: &EglLibrary, raw: *const c_char) -> Result<ItemsTarget, EglStatus> {
    // SAFETY: forwarded from the caller.
    let target: QueryTarget = unsafe { parse(handle, raw, "target JSON") }?;
    query_target_ids(&handle.service, target)
        .map(ItemsTarget::ids)
        .map_err(|err| fail_source(handle, err))
}

/// Validates `edit` before it reaches the library.
fn checked(handle: &EglLibrary, edit: ItemEdit) -> Result<ItemEdit, EglStatus> {
    edit.validate()
        .map(|()| edit)
        .map_err(|err| fail_source(handle, err))
}

/// Edits one item (undoable, like every edit through this library). `*out_item_json` is the
/// item afterwards.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8; `out_item_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_apply_item_edit(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    edit_json: *const c_char,
    out_item_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, strings and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "item id"));
            let edit = tri!(checked(handle, tri!(parse(handle, edit_json, "edit JSON"))));
            reply(
                handle,
                out_item_json,
                handle.service.apply_item_edit(id, edit),
                "item",
            )
        })
    }
}

/// `egl_apply_item_edit` under its older name.
///
/// # Safety
/// As `egl_apply_item_edit`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_apply_item_edit_undoable(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    edit_json: *const c_char,
    out_item_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: forwarded from the caller.
    unsafe { egl_apply_item_edit(lib, id_utf8, edit_json, out_item_json) }
}

/// One `ItemEdit` for a stateless selection (undoable); `*out_count` is how many items
/// changed.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8; `out_count` is null or
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_apply_batch_edit(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    edit_json: *const c_char,
    out_count: *mut u32,
) -> EglStatus {
    // SAFETY: the caller's handle, strings and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let target = tri!(query_target(handle, target_json));
            let edit = tri!(checked(handle, tri!(parse(handle, edit_json, "edit JSON"))));
            reply_count(
                handle,
                out_count,
                handle.service.apply_batch_edit(target, edit),
            )
        })
    }
}

/// One `ItemEdit` for a selection of a view (`ItemsTarget`:
/// `{"viewId":N,"all":bool,"ids":[…]}`; with `all`, every item of the view except `ids`).
/// Undoable; `*out_count` is how many items changed.
///
/// # Safety
/// As `egl_apply_batch_edit`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_edit_items(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    edit_json: *const c_char,
    out_count: *mut u32,
) -> EglStatus {
    // SAFETY: the caller's handle, strings and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let target: ItemsTarget = tri!(parse(handle, target_json, "target JSON"));
            let edit = tri!(checked(handle, tri!(parse(handle, edit_json, "edit JSON"))));
            reply_count(
                handle,
                out_count,
                handle.service.apply_batch_edit(target, edit),
            )
        })
    }
}

/// Takes back the last edit (item edits, batch edits, trash, restore). `*out_json` is the
/// `UndoStatus` taken back, or `null` when there was nothing to undo. A failed undo stays.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_undo(lib: *mut EglLibrary, out_json: *mut *mut c_char) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(handle, out_json, handle.service.undo(), "undo status")
        })
    }
}

/// What `egl_undo` would take back: `{"kind":"…","items":N}` or `null`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_undo_status(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(
                handle,
                out_json,
                Ok(handle.service.undo_status()),
                "undo status",
            )
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Trash

/// Moves a stateless selection to the trash (undoable).
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_count` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_trash_items(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    out_count: *mut u32,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let target = tri!(query_target(handle, target_json));
            reply_count(
                handle,
                out_count,
                handle.service.apply_batch_edit(target, ItemEdit::Trash),
            )
        })
    }
}

/// Restore All: every visible trash item comes back with its folders and tags (undoable).
///
/// # Safety
/// `lib` is a live handle; `out_count` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_restore_trash(lib: *mut EglLibrary, out_count: *mut u32) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply_count(handle, out_count, handle.service.restore_trash())
        })
    }
}

/// Removes the trashed items of a stateless selection for good, with their files. No undo.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_count` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_permanently(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    out_count: *mut u32,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let target = tri!(query_target(handle, target_json));
            reply_count(handle, out_count, handle.service.delete_permanently(target))
        })
    }
}

/// Removes the trashed items of a view selection (`ItemsTarget`) for good. No undo.
///
/// # Safety
/// As `egl_delete_permanently`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_items(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    out_count: *mut u32,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let target: ItemsTarget = tri!(parse(handle, target_json, "target JSON"));
            reply_count(handle, out_count, handle.service.delete_permanently(target))
        })
    }
}

/// Empties the trash with the files; blocks until done. No undo. Another thread may call
/// `egl_cancel_empty_trash` to stop between items.
///
/// # Safety
/// `lib` is a live handle; `out_count` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_empty_trash(lib: *mut EglLibrary, out_count: *mut u32) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply_count(handle, out_count, handle.service.empty_trash(&|_, _| {}))
        })
    }
}

/// Stops a running `egl_empty_trash` between items; remaining files go on the next open.
///
/// # Safety
/// `lib` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_cancel_empty_trash(lib: *mut EglLibrary) -> EglStatus {
    // SAFETY: the caller's handle.
    unsafe {
        with_lib(lib, |handle| {
            handle.service.cancel_empty_trash();
            EglStatus::Ok
        })
    }
}

/// `{"items":N,"bytes":N}` of the visible trash.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_trash_info(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(handle, out_json, handle.service.trash_info(), "trash info")
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Folders

/// Creates, updates or moves a folder; `*out_json` is the new folder tree.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_save_folder(
    lib: *mut EglLibrary,
    change_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let change: FolderChange = tri!(parse(handle, change_json, "folder change JSON"));
            let tree = change
                .validate()
                .and_then(|()| handle.service.save_folder(change));
            reply(handle, out_json, tree, "folder tree")
        })
    }
}

/// Deletes a folder and its subfolders; with `trash_items != 0` their items go to the trash.
/// `*out_json` is the new folder tree.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_folder(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    trash_items: c_int,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let tree = handle.service.delete_folder(id, trash_items != 0);
            reply(handle, out_json, tree, "folder tree")
        })
    }
}

/// Manual order: `ids_json` (`["id",…]`) go before `before_utf8` (null: to the end) in
/// `folder_utf8`.
///
/// # Safety
/// `lib` is a live handle; the strings are null (`before_utf8` only) or NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_reorder_items(
    lib: *mut EglLibrary,
    folder_utf8: *const c_char,
    ids_json: *const c_char,
    before_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let folder = tri!(arg(handle, folder_utf8, "folder id"));
            let ids: Vec<ItemId> = tri!(parse(handle, ids_json, "ids JSON"));
            let before =
                tri!(optional(handle, before_utf8, "before id")).map(|id| ItemId(id.to_owned()));
            reply_unit(handle, handle.service.reorder_items(folder, ids, before))
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Tags

/// Renames a tag on every item; onto an existing tag the two merge.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_rename_tag(
    lib: *mut EglLibrary,
    from_utf8: *const c_char,
    to_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let from = tri!(arg(handle, from_utf8, "from tag"));
            let to = tri!(arg(handle, to_utf8, "to tag"));
            reply_unit(handle, handle.service.rename_tag(from, to))
        })
    }
}

/// Merges tag `source` into `target` on every item.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_merge_tags(
    lib: *mut EglLibrary,
    source_utf8: *const c_char,
    target_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let source = tri!(arg(handle, source_utf8, "source tag"));
            let target = tri!(arg(handle, target_utf8, "target tag"));
            reply_unit(handle, handle.service.merge_tags(source, target))
        })
    }
}

/// Removes a tag from every item.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_tag(
    lib: *mut EglLibrary,
    name_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and string.
    unsafe {
        with_lib(lib, |handle| {
            let name = tri!(arg(handle, name_utf8, "tag name"));
            reply_unit(handle, handle.service.delete_tag(name))
        })
    }
}

/// Creates or updates a tag group; `*out_json` is every group.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_save_tag_group(
    lib: *mut EglLibrary,
    change_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let change: TagGroupChange = tri!(parse(handle, change_json, "tag group change JSON"));
            let groups = change
                .validate()
                .and_then(|()| handle.service.save_tag_group(change));
            reply(handle, out_json, groups, "tag groups")
        })
    }
}

/// Deletes a tag group (item tags stay); `*out_json` is every group left.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_tag_group(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "tag group id"));
            reply(
                handle,
                out_json,
                handle.service.delete_tag_group(id),
                "tag groups",
            )
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Smart folders

/// Creates or updates a smart folder (Eagle-style `conditions`); `*out_json` is the tree.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_save_smart_folder(
    lib: *mut EglLibrary,
    change_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let change: SmartFolderChange = tri!(parse(handle, change_json, "smart folder JSON"));
            reply(
                handle,
                out_json,
                handle.service.save_smart_folder(change),
                "smart folders",
            )
        })
    }
}

/// Deletes a smart folder (and its children); `*out_json` is the tree left.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_smart_folder(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "smart folder id"));
            reply(
                handle,
                out_json,
                handle.service.delete_smart_folder(id),
                "smart folders",
            )
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Locked folders (docs/age-locked-folders.md)

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LockRequest {
    password: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    label_color: Option<String>,
}

/// Locks folder `id`: its items are encrypted, then it is hidden. `request_json`:
/// `{"password":"…","label":null,"labelColor":null}`. `*out_json` is the `RecoveryKey`
/// (`{"key":"…","phrase":"…"}`), shown to the user once and never stored. Blocks while it
/// encrypts.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_lock_folder(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    request_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, strings and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let request: LockRequest = tri!(parse(handle, request_json, "lock request JSON"));
            let key = handle.service.lock_folder(
                id,
                request.password,
                request.label,
                request.label_color,
                &|_, _| {},
            );
            reply(handle, out_json, key, "recovery key")
        })
    }
}

/// Opens locked folder `id` for this session. `EGL_ERR_WRONG_PASSWORD` for a wrong password.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_unlock_folder(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    password_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let password = tri!(arg(handle, password_utf8, "password"));
            reply_unit(
                handle,
                handle.service.unlock_folder(id, password.to_owned()),
            )
        })
    }
}

/// Locks an open folder again: its key leaves memory and its items leave the views. With a
/// null id, every open folder (the app going to the background, say).
///
/// # Safety
/// `lib` is a live handle; the string is null or NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_relock_folder(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and string.
    unsafe {
        with_lib(lib, |handle| {
            let service = &handle.service;
            let result = match tri!(optional(handle, id_utf8, "folder id")) {
                Some(id) => service.relock_folder(id),
                None => service.folder_tree().and_then(|tree| {
                    unlocked_ids(&tree)
                        .iter()
                        .try_for_each(|id| service.relock_folder(id))
                }),
            };
            reply_unit(handle, result)
        })
    }
}

fn unlocked_ids(tree: &[library_core::Folder]) -> Vec<String> {
    tree.iter()
        .flat_map(|folder| {
            let own = (folder.is_locked && folder.is_unlocked).then(|| folder.id.clone());
            own.into_iter().chain(unlocked_ids(&folder.children))
        })
        .collect()
}

/// The recovery key or phrase opens folder `id` and sets a new password.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_recover_folder(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    recovery_utf8: *const c_char,
    new_password_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let recovery = tri!(arg(handle, recovery_utf8, "recovery key"));
            let password = tri!(arg(handle, new_password_utf8, "new password"));
            reply_unit(
                handle,
                handle
                    .service
                    .recover_folder(id, recovery, password.to_owned()),
            )
        })
    }
}

/// A new password for folder `id`; the recovery key stays.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_change_folder_password(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    current_utf8: *const c_char,
    new_password_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let current = tri!(arg(handle, current_utf8, "current password"));
            let password = tri!(arg(handle, new_password_utf8, "new password"));
            reply_unit(
                handle,
                handle
                    .service
                    .change_folder_password(id, current.to_owned(), password.to_owned()),
            )
        })
    }
}

/// A new recovery key for folder `id`; the old one stops working. `*out_json` is the
/// `RecoveryKey`, shown once.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_renew_recovery_key(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    current_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, strings and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let current = tri!(arg(handle, current_utf8, "current password"));
            let key = handle.service.renew_recovery_key(id, current.to_owned());
            reply(handle, out_json, key, "recovery key")
        })
    }
}

/// Takes the lock off folder `id` and decrypts it. A folder imported locked from Eagle has no
/// password: pass an empty one. Blocks while it decrypts.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_remove_folder_lock(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    password_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let password = tri!(arg(handle, password_utf8, "password"));
            reply_unit(
                handle,
                handle
                    .service
                    .remove_folder_lock(id, password.to_owned(), &|_, _| {}),
            )
        })
    }
}

/// The plaintext tag of a locked folder: a label and a colour, each null to clear.
///
/// # Safety
/// `lib` is a live handle; `id_utf8` is NUL-terminated UTF-8; the others are null or
/// NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_set_lock_label(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    label_utf8: *const c_char,
    color_utf8: *const c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and strings.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "folder id"));
            let label = tri!(optional(handle, label_utf8, "label")).map(str::to_owned);
            let color = tri!(optional(handle, color_utf8, "label colour")).map(str::to_owned);
            reply_unit(handle, handle.service.set_lock_label(id, label, color))
        })
    }
}
