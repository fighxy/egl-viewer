//! Library settings and maintenance: the name, size on disk, backups, Check and Repair, and
//! exports into a folder or a ZIP archive (docs/spec.md §7.4 items 6 and 12).

use std::os::raw::{c_char, c_int};
use std::path::Path;

use library_core::{ExportRequest, ExportSelection, RepairOptions};

use crate::{
    EglLibrary, EglStatus, arg, fail, fail_source, parse, reply, reply_unit, tri, with_lib,
    write_string,
};

/// Renames the library (the name inside it; the folder keeps its name). `*out_json` is the
/// `LibraryInfo` afterwards.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_rename_library(
    lib: *mut EglLibrary,
    name_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let name = tri!(arg(handle, name_utf8, "library name"));
            reply(
                handle,
                out_json,
                handle.service.rename_library(name),
                "library info",
            )
        })
    }
}

/// Size on disk by part: `LibraryStats`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_library_stats(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(
                handle,
                out_json,
                handle.service.library_stats(),
                "library stats",
            )
        })
    }
}

/// The backups of the database, newest first: `Backup[]`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_backups(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(handle, out_json, handle.service.backups(), "backups")
        })
    }
}

/// Backs the database up now: `*out_json` is the new `Backup`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_back_up_now(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(handle, out_json, handle.service.back_up_now(), "backup")
        })
    }
}

/// Restores backup `name` into the open library, after backing up the current state. Imports
/// stop; views, undo and unlocked folders start over. `*out_json` is the `LibraryInfo`.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_restore_backup(
    lib: *mut EglLibrary,
    name_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let name = tri!(arg(handle, name_utf8, "backup name"));
            reply(
                handle,
                out_json,
                handle.service.restore_backup(name),
                "library info",
            )
        })
    }
}

/// Check Library: the database's own check, items whose file is missing, files no item points
/// at: `LibraryCheck`. Reads only.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_check_library(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(
                handle,
                out_json,
                handle.service.check_library(),
                "library check",
            )
        })
    }
}

/// Repairs what a check found, as `options_json` asks (`{"orphans":bool,"missing":bool}`);
/// `*out_json` is the `LibraryCheck` afterwards.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_repair_library(
    lib: *mut EglLibrary,
    options_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let options: RepairOptions = tri!(parse(handle, options_json, "repair options JSON"));
            reply(
                handle,
                out_json,
                handle.service.repair_library(options),
                "library check",
            )
        })
    }
}

/// What an export of `selection_json` (`ExportSelection`) would write, from the database:
/// `ExportPreview`.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_export_preview(
    lib: *mut EglLibrary,
    selection_json: *const c_char,
    include_subfolders: c_int,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let selection: ExportSelection =
                tri!(parse(handle, selection_json, "export selection JSON"));
            let preview = handle
                .service
                .export_preview(selection, include_subfolders != 0);
            reply(handle, out_json, preview, "export preview")
        })
    }
}

/// Starts an export into a folder tree or a ZIP archive (`ExportRequest`; `eagle` must be
/// null). `*out_json` is its first `ExportProgress`; the rest come as `exportProgress`
/// events, or poll `egl_exports`. A failed or cancelled export removes what it wrote.
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_start_export(
    lib: *mut EglLibrary,
    request_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let request: ExportRequest = tri!(parse(handle, request_json, "export request JSON"));
            reply(
                handle,
                out_json,
                handle.service.start_export(request),
                "export progress",
            )
        })
    }
}

/// Running exports, newest first: `ExportProgress[]`.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_exports(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            reply(
                handle,
                out_json,
                Ok(handle.service.exports().list()),
                "exports",
            )
        })
    }
}

/// Asks export `id` to stop; it removes what it wrote.
///
/// # Safety
/// `lib` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_cancel_export(lib: *mut EglLibrary, id: u64) -> EglStatus {
    // SAFETY: the caller's handle.
    unsafe {
        with_lib(lib, |handle| {
            reply_unit(handle, handle.service.exports().cancel(id))
        })
    }
}

/// Decrypts a passphrase `.age` archive at `source_utf8` to `destination_utf8` (typically a
/// `.zip`). Blocks; call [`egl_cancel_decrypt_age`] from another thread to stop. On success
/// `*out_path` is the written path (free with `egl_string_free`); on cancel both status and
/// path are OK / NULL.
///
/// # Safety
/// `lib` is a live handle; the strings are NUL-terminated UTF-8; `out_path` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_decrypt_age_archive(
    lib: *mut EglLibrary,
    source_utf8: *const c_char,
    destination_utf8: *const c_char,
    password_utf8: *const c_char,
    out_path: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, strings and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let source = tri!(arg(handle, source_utf8, "source path"));
            let destination = tri!(arg(handle, destination_utf8, "destination path"));
            let password = tri!(arg(handle, password_utf8, "password"));
            match handle.service.decrypt_age_archive(
                Path::new(source),
                Path::new(destination),
                password.to_owned(),
                &|_, _| {},
            ) {
                Ok(Some(path)) => match write_string(out_path, path.to_string_lossy()) {
                    Ok(()) => EglStatus::Ok,
                    Err(status) => fail(handle, status, "failed to hand over the path"),
                },
                Ok(None) => {
                    if !out_path.is_null() {
                        *out_path = std::ptr::null_mut();
                    }
                    EglStatus::Ok
                }
                Err(error) => fail_source(handle, error),
            }
        })
    }
}

/// Stops a running [`egl_decrypt_age_archive`] between buffers.
///
/// # Safety
/// `lib` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_cancel_decrypt_age(lib: *mut EglLibrary) -> EglStatus {
    // SAFETY: the caller's handle.
    unsafe {
        with_lib(lib, |handle| {
            handle.service.cancel_decrypt_age();
            reply_unit(handle, Ok(()))
        })
    }
}

/// What a reviewed import of folder `path_utf8` would take: `ImportPreview`. Refused like the
/// import (read-only, or a folder inside this library).
///
/// # Safety
/// `lib` is a live handle; the string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_preview_import(
    lib: *mut EglLibrary,
    path_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle, string and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let path = tri!(arg(handle, path_utf8, "folder path"));
            let preview = handle.service.preview_import(Path::new(path));
            reply(handle, out_json, preview, "import preview")
        })
    }
}
