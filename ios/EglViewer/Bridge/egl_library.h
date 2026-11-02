/**
 * C ABI for the experimental egl-viewer iOS / Android shells (docs/decisions.md D-082).
 *
 * Native `.eglib` only — no Eagle. Strings and JSON buffers are UTF-8,
 * heap-allocated by Rust; free them with egl_string_free / egl_bytes_free.
 *
 * Build:
 *   iOS (Mac):  cargo build -p library-ffi --release --target aarch64-apple-ios
 *               → libegl_library.a (see ios/README.md)
 *   Android:    cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs \
 *                 build -p library-ffi --release
 *               → libegl_library.so (see android/README.md)
 */

#ifndef EGL_LIBRARY_H
#define EGL_LIBRARY_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct EglLibrary EglLibrary;

/** Success and error codes returned by every call. */
typedef enum EglStatus {
  EGL_OK = 0,
  EGL_ERR_INVALID = 1,
  EGL_ERR_NOT_FOUND = 2,
  EGL_ERR_READ_ONLY = 3,
  EGL_ERR_NOT_A_LIBRARY = 4,
  EGL_ERR_STORAGE = 5,
  EGL_ERR_UNSUPPORTED = 6,
  EGL_ERR_OTHER = 7
} EglStatus;

/**
 * Creates a new library folder `<parent>/<name>.eglib`.
 * On success, *out_path is the absolute path (free with egl_string_free).
 */
EglStatus egl_create_library(
    const char *parent_utf8,
    const char *name_utf8,
    const char *app_version_utf8,
    char **out_path);

/**
 * Opens a library folder. Pass sync_folder != 0 when the path is inside a
 * cloud-synced location (opens read-only, D-021).
 * Returns NULL on failure; call egl_last_error(NULL) is not available — use
 * the status from a follow-up only after a successful open. Failures that
 * happen before a handle exists leave a process-wide last error via
 * egl_last_error_global().
 */
EglLibrary *egl_open(
    const char *path_utf8,
    const char *app_version_utf8,
    int sync_folder);

/** Closes and frees the handle; cancels and joins running imports first. Safe with NULL. */
void egl_close(EglLibrary *lib);

/** LibraryInfo as camelCase JSON. */
EglStatus egl_library_info(EglLibrary *lib, char **out_json);

/**
 * Window of ItemCell JSON for a Scope.
 * scope_json examples: {"kind":"all"}, {"kind":"unfiled"}, {"kind":"trash"},
 * {"kind":"folder","id":"…","includeDescendants":true}.
 * Result: {"total":N,"start":S,"items":[…]} with at most 200 items (MAX_WINDOW).
 */
EglStatus egl_view_window(
    EglLibrary *lib,
    const char *scope_json,
    uint32_t start,
    uint32_t len,
    char **out_json);

/** Full Item as camelCase JSON, or EGL_ERR_NOT_FOUND. */
EglStatus egl_item(EglLibrary *lib, const char *id_utf8, char **out_json);

/** Folder tree JSON array. */
EglStatus egl_folder_tree(EglLibrary *lib, char **out_json);

/** Tag vocabulary JSON array. */
EglStatus egl_tags(EglLibrary *lib, char **out_json);

/** Absolute path of the WebP thumbnail file inside the library (may not exist yet). */
EglStatus egl_thumbnail_path(EglLibrary *lib, const char *id_utf8, char **out_path);

/**
 * Thumbnail file bytes (WebP) and content type. out_bytes length is *out_len.
 * Free with egl_bytes_free. Returns EGL_ERR_NOT_FOUND when missing.
 */
EglStatus egl_thumbnail_bytes(
    EglLibrary *lib,
    const char *id_utf8,
    uint8_t **out_bytes,
    size_t *out_len,
    char **out_content_type);

/**
 * Applies an ItemEdit JSON intent, e.g. {"type":"addTags","value":["Ref"]}.
 * Returns the updated Item JSON.
 */
EglStatus egl_apply_item_edit(
    EglLibrary *lib,
    const char *id_utf8,
    const char *edit_json,
    char **out_item_json);

/* ==========================================================================
 * Management (mobile parity with the desktop commands; native .eglib only).
 *
 * Shared JSON shapes (camelCase, same as the desktop IPC):
 *   Target (a selection):
 *     {"ids":["01…"]}                                  listed items
 *     {"all":true,"ids":[excluded…],"query":Query}     Select All of a query
 *   Query (LibraryQuery; every field optional, {} = All, newest first):
 *     {"scope":{"kind":"all"|"unfiled"|"untagged"|"trash"}
 *            | {"kind":"folder","id":"…","includeDescendants":true}
 *            | {"kind":"smart","id":"…"},
 *      "tags":["A"],"tagMode":"any"|"all"|"none", "extraFolderIds":[…],
 *      "folderMode":"any"|"all"|"none", "ext":"png", "rating":0..5,
 *      "shape":"…", "text":"search", "sort":"added"|"btime"|"mtime"|"name"|"ext"
 *      |"size"|"resolution"|"star"|"duration"|"tag"|"random"|"manual",
 *      "ascending":false}
 *   ImportProgress: {"id","kind":"files"|"previews","state":"scanning"|"adding"
 *      |"waitingForDecision"|"previews"|"done"|"cancelled","total","processed",
 *      "added","duplicates","failed","previewsTotal","previewsDone","current",
 *      "duplicate":{…}|null,"failures":[…],"skipped":[…],…}
 *   UndoStatus: {"kind":"…","items":N}
 *
 * Counts (u32 *out_count) may be NULL when the caller does not need them.
 * Every call returns EglStatus; on failure read egl_last_error(lib).
 * Blocking calls (egl_empty_trash, egl_query_window on big libraries) should
 * run off the UI thread; the handle is safe to use from several threads.
 * ========================================================================== */

/* --- 1. Adding files ---------------------------------------------------- */

/**
 * Starts a background import of files/folders (recursive) with thumbnails.
 * request_json: {"paths":["/abs/a.png","/abs/dir"],"folder":null|"id",
 *                "exclude":[],"include":[],"reviewed":false}
 * *out_json: {"id":N}. Poll with egl_import_status.
 */
EglStatus egl_add_files(EglLibrary *lib, const char *request_json, char **out_json);

/** Answers the duplicate question (state "waitingForDecision").
 *  answer_utf8: "skip" | "addAnyway" | "addToFolder" | "cancel". */
EglStatus egl_answer_import(EglLibrary *lib, uint64_t id, const char *answer_utf8);

/** Stops import id after the current file; EGL_ERR_NOT_FOUND once it ended. */
EglStatus egl_cancel_import(EglLibrary *lib, uint64_t id);

/** ImportProgress[]: running (newest first), then the last 20 finished. */
EglStatus egl_imports(EglLibrary *lib, char **out_json);

/** ImportProgress of one import, or JSON null if it has not reported yet. */
EglStatus egl_import_status(EglLibrary *lib, uint64_t id, char **out_json);

/**
 * Makes missing thumbnails in the background (rebuild != 0: all of them again).
 * *out_json: {"id":N|null,"waiting":N}; id is null when a preview task already runs.
 */
EglStatus egl_generate_previews(EglLibrary *lib, int rebuild, char **out_json);

/* --- 2. Originals -------------------------------------------------------- */

/** Absolute path of the stored original (full-size view). Refused for items of
 *  locked folders. Item detail JSON is egl_item. */
EglStatus egl_original_path(EglLibrary *lib, const char *id_utf8, char **out_path);

/* --- 3. Trash ------------------------------------------------------------ */

/** Moves a Target to the trash (undoable). */
EglStatus egl_trash_items(EglLibrary *lib, const char *target_json, uint32_t *out_count);
/** Restore All of the trash (undoable). */
EglStatus egl_restore_trash(EglLibrary *lib, uint32_t *out_count);
/** Removes the trashed items of a Target with their files. No undo. */
EglStatus egl_delete_permanently(EglLibrary *lib, const char *target_json, uint32_t *out_count);
/** Empties the trash (blocking). No undo. */
EglStatus egl_empty_trash(EglLibrary *lib, uint32_t *out_count);
/** Stops a running egl_empty_trash between items (call from another thread). */
EglStatus egl_cancel_empty_trash(EglLibrary *lib);
/** {"items":N,"bytes":N} */
EglStatus egl_trash_info(EglLibrary *lib, char **out_json);

/* --- 4. Folders ---------------------------------------------------------- */

/**
 * FolderChange JSON; *out_json is the new folder tree.
 *  {"type":"create","name":"…","parent":null,"description":"","iconColor":null}
 *  {"type":"update","id":"…","name":"…","description":"","tags":[],
 *   "iconColor":null,"orderBy":null,"sortAscending":null}
 *  {"type":"move","id":"…","parent":null,"index":0}
 */
EglStatus egl_save_folder(EglLibrary *lib, const char *change_json, char **out_json);
/** Deletes a folder and subfolders; trash_items != 0 trashes their items. */
EglStatus egl_delete_folder(EglLibrary *lib, const char *id_utf8, int trash_items, char **out_json);
/** Manual order: ids_json ["id",…] before before_utf8 (NULL = end) in folder. */
EglStatus egl_reorder_items(EglLibrary *lib, const char *folder_utf8, const char *ids_json,
                            const char *before_utf8);

/* --- 5. Tags (vocabulary with counts is egl_tags) ------------------------ */

/** Renames a tag everywhere; onto an existing name the two merge. */
EglStatus egl_rename_tag(EglLibrary *lib, const char *from_utf8, const char *to_utf8);
EglStatus egl_merge_tags(EglLibrary *lib, const char *source_utf8, const char *target_utf8);
EglStatus egl_delete_tag(EglLibrary *lib, const char *name_utf8);
/** TagGroup[] */
EglStatus egl_tag_groups(EglLibrary *lib, char **out_json);
/** {"type":"create","name","color":null,"tags":[]} |
 *  {"type":"update","id","name","color","tags"}; returns TagGroup[]. */
EglStatus egl_save_tag_group(EglLibrary *lib, const char *change_json, char **out_json);
EglStatus egl_delete_tag_group(EglLibrary *lib, const char *id_utf8, char **out_json);

/* --- 6. Multi-select and undo -------------------------------------------- */

/** One ItemEdit for a Target (undoable). */
EglStatus egl_apply_batch_edit(EglLibrary *lib, const char *target_json, const char *edit_json,
                               uint32_t *out_count);
/** {"items":N,"tags":[{"name","imageCount"}],"size":N,"stack":[ItemCell…]}.
 *  Counted within target.query (default All). */
EglStatus egl_selection_tags(EglLibrary *lib, const char *target_json, char **out_json);
/** egl_apply_item_edit that egl_undo can take back. */
EglStatus egl_apply_item_edit_undoable(EglLibrary *lib, const char *id_utf8,
                                       const char *edit_json, char **out_item_json);
/** Takes back the last undoable edit of this handle; UndoStatus or null. */
EglStatus egl_undo(EglLibrary *lib, char **out_json);
/** What egl_undo would take back; UndoStatus or null. */
EglStatus egl_undo_status(EglLibrary *lib, char **out_json);

/* --- 7. Search / filter / sort and smart folders ------------------------- */

/** Window of a full Query: {"total":N,"start":S,"items":[ItemCell…]} (≤ 200). */
EglStatus egl_query_window(EglLibrary *lib, const char *query_json, uint32_t start,
                           uint32_t len, char **out_json);
/** ExtensionCount[] for the Type filter. */
EglStatus egl_extensions(EglLibrary *lib, char **out_json);
/** SmartFolder[] tree. */
EglStatus egl_smart_folders(EglLibrary *lib, char **out_json);
/** {"type":"create","name","parent":null,"conditions":[…]} |
 *  {"type":"update","id","name","conditions":[…]}; returns SmartFolder[]. */
EglStatus egl_save_smart_folder(EglLibrary *lib, const char *change_json, char **out_json);
EglStatus egl_delete_smart_folder(EglLibrary *lib, const char *id_utf8, char **out_json);

/*
 * JNI notes (android/app/src/main/cpp/egl_jni.cpp forwards these 1:1):
 *  - Pass the EglLibrary* as jlong; uint64_t import ids as jlong.
 *  - JSON in: GetStringUTFChars / ReleaseStringUTFChars; JSON out: take_c_string
 *    (NewStringUTF + egl_string_free). Return null on status != EGL_OK and let
 *    Kotlin read egl_last_error via the existing lastError forwarder.
 *  - u32 counts: return jint (or -1 on error).
 *  - egl_reorder_items before_utf8: pass nullptr for a null jstring.
 *  - Never call blocking exports (egl_empty_trash, egl_query_window) on the
 *    main thread; egl_cancel_empty_trash / egl_cancel_import are safe from any.
 */

void egl_string_free(char *s);
void egl_bytes_free(uint8_t *bytes, size_t len);

/** Last error message for this handle, or empty string. Never NULL. */
const char *egl_last_error(EglLibrary *lib);

/** Last error when open/create failed before a handle existed. Never NULL. */
const char *egl_last_error_global(void);

#ifdef __cplusplus
}
#endif

#endif /* EGL_LIBRARY_H */
