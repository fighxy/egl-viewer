/**
 * C ABI for the experimental egl-viewer iOS shell (docs/decisions.md D-082).
 *
 * Native `.eglib` only — no Eagle. Strings and JSON buffers are UTF-8,
 * heap-allocated by Rust; free them with egl_string_free / egl_bytes_free.
 *
 * Build the static library on a Mac:
 *   rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
 *   cargo build -p library-ffi --release --target aarch64-apple-ios
 * Then link libegl_library.a from the Xcode target (see ios/README.md).
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

/** Closes and frees the handle. Safe to call with NULL. */
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
