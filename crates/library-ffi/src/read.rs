//! Reads: views by id (D-092) and the stateless windows they replace, items, the folder tree,
//! tags, smart folders, thumbnails and originals.

use std::os::raw::c_char;

use app_service::{ItemsTarget, LibraryService};
use library_core::{ItemCell, ItemId, LibraryQuery, MAX_WINDOW, Scope, SelectionTags, SourceError};
use serde::{Deserialize, Serialize};

use crate::{
    EglLibrary, EglStatus, arg, fail, fail_source, parse, parse_or_default, reply, reply_unit, tri,
    with_lib, write_string,
};

/// A selection as the stateless calls name it: listed `ids`, or with `all` every item of
/// `query` except `ids`. The view-based calls take an `ItemsTarget` instead.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct QueryTarget {
    pub ids: Vec<ItemId>,
    pub all: bool,
    pub query: Option<LibraryQuery>,
}

/// Runs `read` on a view of `query` that lives only for the call.
pub(crate) fn with_query_view<T>(
    service: &LibraryService,
    query: LibraryQuery,
    read: impl FnOnce(u64) -> Result<T, SourceError>,
) -> Result<T, SourceError> {
    let opened = service.open_view(query, &|_, _| {})?;
    let result = read(opened.view_id);
    service.close_view(opened.view_id);
    result
}

/// The ids a `QueryTarget` names.
pub(crate) fn query_target_ids(
    service: &LibraryService,
    target: QueryTarget,
) -> Result<Vec<ItemId>, SourceError> {
    if !target.all {
        return Ok(target.ids);
    }
    let query = target
        .query
        .ok_or_else(|| SourceError::Invalid("Select All needs its query.".into()))?;
    with_query_view(service, query, |view_id| {
        service.target_ids(ItemsTarget {
            view_id: Some(view_id),
            all: true,
            ids: target.ids,
        })
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Window {
    total: usize,
    start: u32,
    items: Vec<ItemCell>,
}

fn window(
    service: &LibraryService,
    query: LibraryQuery,
    start: u32,
    len: u32,
) -> Result<Window, SourceError> {
    with_query_view(service, query, |view_id| {
        let total = service.with_view(view_id, |view| view.total())?;
        let items = service.view_range(view_id, start, len.min(MAX_WINDOW as u32))?;
        Ok(Window {
            total,
            start,
            items,
        })
    })
}

// ---------------------------------------------------------------------------------------------
// Views by id

/// Opens a view of `query_json` (a `LibraryQuery`; null or empty is All, newest first):
/// `*out_json` is `{"viewId":N,"total":N}`. The view follows writes under the same id: a
/// `viewsInvalidated` event says when its total or order changed. Close it with
/// `egl_close_view`; the oldest views go when too many are open.
///
/// # Safety
/// `lib` is a live handle; the string is null or NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_open_view(
    lib: *mut EglLibrary,
    query_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let query: LibraryQuery = tri!(parse_or_default(handle, query_json, "query JSON"));
            let opened = handle.service.open_view(query, &|_, _| {});
            reply(handle, out_json, opened, "view")
        })
    }
}

/// At most 200 cells of view `view_id` from `start`: `ItemCell[]`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_view_range(
    lib: *mut EglLibrary,
    view_id: u64,
    start: u32,
    len: u32,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let cells = handle.service.view_range(view_id, start, len);
            reply(handle, out_json, cells, "cells")
        })
    }
}

/// The ids of positions `start..start + len` of a view, for selections: `string[]`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_view_ids(
    lib: *mut EglLibrary,
    view_id: u64,
    start: u32,
    len: u32,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let ids = handle.service.view_ids(view_id, start, len);
            reply(handle, out_json, ids, "ids")
        })
    }
}

/// Width and height of every position of a view, for justified rows: `[[w,h],…]`; `[0,0]`
/// when unknown.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_view_dimensions(
    lib: *mut EglLibrary,
    view_id: u64,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let dimensions = handle.service.view_dimensions(view_id);
            reply(handle, out_json, dimensions, "dimensions")
        })
    }
}

/// Closes a view. An unknown id is fine.
///
/// # Safety
/// `lib` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_close_view(lib: *mut EglLibrary, view_id: u64) -> EglStatus {
    // SAFETY: the caller's handle.
    unsafe {
        with_lib(lib, |handle| {
            handle.service.close_view(view_id);
            EglStatus::Ok
        })
    }
}

/// The tags of a selection of a view (`ItemsTarget`: `{"viewId":N,"all":bool,"ids":[…]}`),
/// each with how many selected items carry it: `SelectionTags`.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_view_selection_tags(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let target: ItemsTarget = tri!(parse(handle, target_json, "target JSON"));
            let tags = handle.service.selection_tags(target);
            reply(handle, out_json, tags, "selection tags")
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Stateless windows (kept for the shells that call them)

/// A window of All or of one `Scope`: `{"total":N,"start":S,"items":[ItemCell…]}`, at most
/// 200 items, newest first. Every call evaluates the scope again; `egl_open_view` does not.
///
/// # Safety
/// `lib` is a live handle; the string is null or NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_view_window(
    lib: *mut EglLibrary,
    scope_json: *const c_char,
    start: u32,
    len: u32,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let text = crate::c_str(scope_json).unwrap_or("");
            let scope: Scope = if text.trim().is_empty() || text.trim() == "{}" {
                Scope::All
            } else {
                tri!(parse(handle, scope_json, "scope JSON"))
            };
            let query = LibraryQuery {
                scope,
                ..LibraryQuery::default()
            };
            reply(
                handle,
                out_json,
                window(&handle.service, query, start, len),
                "view window",
            )
        })
    }
}

/// A window of a full `LibraryQuery` (scope, chips, type, rating, shape, search text, sort):
/// `{"total":N,"start":S,"items":[ItemCell…]}`, at most 200 items. Empty / `{}` is All, newest
/// first. Every call evaluates the query again; `egl_open_view` does not.
///
/// # Safety
/// `lib` is a live handle; the string is null or NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_query_window(
    lib: *mut EglLibrary,
    query_json: *const c_char,
    start: u32,
    len: u32,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let query: LibraryQuery = tri!(parse_or_default(handle, query_json, "query JSON"));
            reply(
                handle,
                out_json,
                window(&handle.service, query, start, len),
                "query window",
            )
        })
    }
}

/// The tags of a selection with per-tag counts: `SelectionTags`. `target_json` is
/// `{"ids":[…],"all":bool,"query":LibraryQuery|null}`; counts come from `query` (default
/// All), so a trash selection needs `{"scope":{"kind":"trash"}}`.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_selection_tags(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let target: QueryTarget = tri!(parse(handle, target_json, "target JSON"));
            let service = &handle.service;
            let tags: Result<SelectionTags, SourceError> =
                with_query_view(service, target.query.unwrap_or_default(), |view_id| {
                    service.selection_tags(ItemsTarget {
                        view_id: Some(view_id),
                        all: target.all,
                        ids: target.ids,
                    })
                });
            reply(handle, out_json, tags, "selection tags")
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Items, folders, tags

/// The full item: `Item`. `EGL_ERR_NOT_FOUND` when it is gone or hidden by a lock.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_item(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "item id"));
            match handle.service.item(id) {
                Ok(Some(item)) => reply(handle, out_json, Ok(item), "item"),
                Ok(None) => fail(handle, EglStatus::NotFound, "item not found"),
                Err(err) => fail_source(handle, err),
            }
        })
    }
}

/// `LibraryCounts` for the sidebar: All, Uncategorized, Untagged, Trash.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_counts(lib: *mut EglLibrary, out_json: *mut *mut c_char) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(handle, out_json, handle.service.counts(), "counts")
        })
    }
}

/// The folder tree with counts: `Folder[]`. Passwords never appear.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_folder_tree(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(
                handle,
                out_json,
                handle.service.folder_tree(),
                "folder tree",
            )
        })
    }
}

/// The tag vocabulary with counts: `Tag[]`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_tags(lib: *mut EglLibrary, out_json: *mut *mut c_char) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(handle, out_json, handle.service.tags(), "tags")
        })
    }
}

/// `TagGroup[]`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_tag_groups(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(handle, out_json, handle.service.tag_groups(), "tag groups")
        })
    }
}

/// `ExtensionCount[]` for the Type filter.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_extensions(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(handle, out_json, handle.service.extensions(), "extensions")
        })
    }
}

/// The smart folder tree: `SmartFolder[]`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_smart_folders(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(
                handle,
                out_json,
                handle.service.smart_folders(),
                "smart folders",
            )
        })
    }
}

/// "Save as smart folder": the rules that keep what `query_json` shows, and the smart folder
/// the new one goes under: `{"parent":id|null,"conditions":[…]}`. Refused with the reason
/// when rules cannot hold the query (search text, the trash, nothing to keep).
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_smart_rules(
    lib: *mut EglLibrary,
    query_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let query: LibraryQuery = tri!(parse(handle, query_json, "query JSON"));
            reply(
                handle,
                out_json,
                handle.service.smart_rules(&query),
                "rules",
            )
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Thumbnails and originals

/// Absolute path of the stored WebP thumbnail.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_path` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_thumbnail_path(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_path: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "item id"));
            let library = tri!(
                handle
                    .service
                    .current_library()
                    .ok_or_else(app_service::no_library)
                    .map_err(|err| fail_source(handle, err))
            );
            match library.thumbnail_path(id) {
                Ok(path) => match write_string(out_path, path.to_string_lossy()) {
                    Ok(()) => EglStatus::Ok,
                    Err(status) => fail(handle, status, "failed to hand over the path"),
                },
                Err(err) => fail_source(handle, err),
            }
        })
    }
}

/// Hands `bytes` to the caller as a buffer freed with `egl_bytes_free`.
///
/// # Safety
/// `out_bytes` and `out_len` are writable.
unsafe fn hand_over_bytes(bytes: Vec<u8>, out_bytes: *mut *mut u8, out_len: *mut usize) {
    let len = bytes.len();
    let mut boxed = bytes.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    // SAFETY: the caller's out pointers.
    unsafe {
        *out_bytes = ptr;
        *out_len = len;
    }
}

/// The thumbnail bytes (WebP; decrypted for an unlocked folder) and their content type.
/// `EGL_ERR_NOT_FOUND` when there is none yet.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_bytes` and `out_len` are
/// writable; `out_content_type` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_thumbnail_bytes(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
    out_content_type: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointers.
    unsafe {
        with_lib(lib, |handle| {
            if out_bytes.is_null() || out_len.is_null() {
                return fail(
                    handle,
                    EglStatus::Invalid,
                    "out_bytes and out_len are required",
                );
            }
            let id = tri!(arg(handle, id_utf8, "item id"));
            let thumb = match handle.service.image(&format!("/{id}")) {
                Some(thumb) => thumb,
                None => return fail(handle, EglStatus::NotFound, "thumbnail not found"),
            };
            if !out_content_type.is_null()
                && let Err(status) = write_string(out_content_type, thumb.content_type)
            {
                return fail(handle, status, "failed to hand over the content type");
            }
            hand_over_bytes(thumb.bytes, out_bytes, out_len);
            EglStatus::Ok
        })
    }
}

/// Absolute path of the stored original, for a full-size view or sharing. Refused for an
/// item of a locked folder: its file is encrypted (use `egl_original_bytes`).
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_path` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_original_path(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_path: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let id = tri!(arg(handle, id_utf8, "item id"));
            match handle.service.original_paths(&[ItemId(id.to_owned())]) {
                Ok(paths) => match write_string(out_path, paths[0].to_string_lossy()) {
                    Ok(()) => EglStatus::Ok,
                    Err(status) => fail(handle, status, "failed to hand over the path"),
                },
                Err(err) => fail_source(handle, err),
            }
        })
    }
}

/// The original's bytes and content type, decrypted for an item of an unlocked folder: for a
/// full-size view of a locked folder's item, which has no plain file to open.
///
/// # Safety
/// As `egl_thumbnail_bytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_original_bytes(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
    out_content_type: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointers.
    unsafe {
        with_lib(lib, |handle| {
            if out_bytes.is_null() || out_len.is_null() {
                return fail(
                    handle,
                    EglStatus::Invalid,
                    "out_bytes and out_len are required",
                );
            }
            let id = tri!(arg(handle, id_utf8, "item id"));
            let original = match handle.service.image(&format!("/{id}/original")) {
                Some(original) => original,
                None => return fail(handle, EglStatus::NotFound, "original not found"),
            };
            if !out_content_type.is_null()
                && let Err(status) = write_string(out_content_type, original.content_type)
            {
                return fail(handle, status, "failed to hand over the content type");
            }
            hand_over_bytes(original.bytes, out_bytes, out_len);
            EglStatus::Ok
        })
    }
}

/// Recomputes the open views now (after the app wrote files itself, say) and sends
/// `viewsInvalidated` for those that changed.
///
/// # Safety
/// `lib` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_refresh_views(lib: *mut EglLibrary) -> EglStatus {
    // SAFETY: the caller's handle.
    unsafe {
        with_lib(lib, |handle| {
            handle.service.refresh_views();
            reply_unit(handle, Ok(()))
        })
    }
}
