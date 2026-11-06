//! Library settings and maintenance: the name, size on disk, backups, Check and Repair, and
//! exports into a folder or a ZIP archive (docs/spec.md §7.4 items 6 and 12).

use std::os::raw::{c_char, c_int};
use std::path::Path;

use library_core::{ExportRequest, ExportSelection, RepairOptions};

use std::sync::atomic::AtomicBool;

use crate::{
    EglLibrary, EglStatus, arg, c_str, guarded, map_error, parse, reply, reply_unit,
    set_global_error, tri, with_lib, write_json,
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

/// Opens an age archive (an encrypted export, `name.zip.age`) with its passphrase into the
/// existing folder `destination_utf8`, as `name.zip` (D-110). Needs no library handle.
/// `*out_json` is `DecryptedArchive` {"path","bytes"}. A wrong passphrase, a damaged file or
/// a file that is not an age archive is `EGL_ERR_INVALID` with the reason in
/// `egl_last_error_global()`, and leaves nothing behind. Runs on the caller's thread.
///
/// # Safety
/// The strings are NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_decrypt_archive(
    source_utf8: *const c_char,
    destination_utf8: *const c_char,
    passphrase_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    guarded(set_global_error, || {
        // SAFETY: the caller's strings.
        let (source, destination, passphrase) = unsafe {
            (
                c_str(source_utf8),
                c_str(destination_utf8),
                c_str(passphrase_utf8),
            )
        };
        let (Ok(source), Ok(destination), Ok(passphrase)) = (source, destination, passphrase)
        else {
            set_global_error("the archive, the folder and the passphrase are required");
            return EglStatus::Invalid;
        };
        let cancel = AtomicBool::new(false);
        let result = library_native::decrypt_archive(
            Path::new(source),
            Path::new(destination),
            passphrase,
            &cancel,
            &mut |_, _| {},
        );
        match result {
            Ok(Some(decrypted)) => match unsafe { write_json(out_json, &decrypted) } {
                // SAFETY (above): the caller's out pointer.
                Ok(()) => EglStatus::Ok,
                Err(status) => {
                    set_global_error("failed to hand over the decrypted file");
                    status
                }
            },
            Ok(None) => {
                set_global_error("the decryption stopped");
                EglStatus::Other
            }
            Err(error) => {
                set_global_error(error.to_string());
                map_error(&error)
            }
        }
    })
}

/// What the import wizard shows for the Eagle library folder at `path_utf8` (docs/spec.md §8.2
/// step 1, D-116): `EagleLibrarySummary` {"path","name","itemCount","bytes"}. Needs no library
/// handle and writes nothing. `EGL_ERR_NOT_A_LIBRARY` when the folder is not an Eagle library,
/// with the reason in `egl_last_error_global()`. Walks `images/` for the size: run it off the
/// UI thread.
///
/// # Safety
/// The string is NUL-terminated UTF-8; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_eagle_library_info(
    path_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    guarded(set_global_error, || {
        // SAFETY: the caller's string.
        let Ok(path) = (unsafe { c_str(path_utf8) }) else {
            set_global_error("the Eagle library folder is required");
            return EglStatus::Invalid;
        };
        match app_service::eagle_library_info(Path::new(path)) {
            // SAFETY: the caller's out pointer.
            Ok(summary) => match unsafe { write_json(out_json, &summary) } {
                Ok(()) => EglStatus::Ok,
                Err(status) => {
                    set_global_error("failed to hand over the Eagle library summary");
                    status
                }
            },
            Err(error) => {
                set_global_error(match &error {
                    library_core::SourceError::NotALibrary { .. } => {
                        "This folder is not an Eagle library (Name.library).".to_owned()
                    }
                    other => other.to_string(),
                });
                map_error(&error)
            }
        }
    })
}
