/**
 * Thin JNI forwarder to the library-ffi C ABI (D-083 / D-101 / D-102 / D-103).
 * A1: create / open / close, info, counts, folder tree, event callback.
 * A2: open_view / view_range / close_view, tags, extensions, smart folders, thumbs, edit_items.
 * A3: item, apply_item_edit, selection tags, undo, delete, empty trash, view ids, originals.
 * Package: app.eglviewer.core.EglNative
 */

#include <jni.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#if defined(EGL_HAS_RUST)
#include "egl_library.h"
#endif

static JavaVM *g_vm = nullptr;
static thread_local int g_last_status = 0;

static void set_status(int status) { g_last_status = status; }

static jstring empty_string(JNIEnv *env) { return env->NewStringUTF(""); }

static jstring take_c_string(JNIEnv *env, char *owned) {
  if (owned == nullptr) {
    return empty_string(env);
  }
  jstring out = env->NewStringUTF(owned);
#if defined(EGL_HAS_RUST)
  egl_string_free(owned);
#else
  free(owned);
#endif
  return out;
}

JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM *vm, void *) {
  g_vm = vm;
  return JNI_VERSION_1_6;
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_lastStatus(JNIEnv *, jclass) {
  return g_last_status;
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_createLibrary(JNIEnv *env, jclass,
                                                jstring parent, jstring name,
                                                jstring version) {
#if defined(EGL_HAS_RUST)
  const char *parent_utf = env->GetStringUTFChars(parent, nullptr);
  const char *name_utf = env->GetStringUTFChars(name, nullptr);
  const char *ver_utf = env->GetStringUTFChars(version, nullptr);
  char *out_path = nullptr;
  EglStatus status =
      egl_create_library(parent_utf, name_utf, ver_utf, &out_path);
  set_status(static_cast<int>(status));
  env->ReleaseStringUTFChars(parent, parent_utf);
  env->ReleaseStringUTFChars(name, name_utf);
  env->ReleaseStringUTFChars(version, ver_utf);
  if (status != EGL_OK || out_path == nullptr) {
    return nullptr;
  }
  return take_c_string(env, out_path);
#else
  (void)env;
  (void)parent;
  (void)name;
  (void)version;
  set_status(6); /* EGL_ERR_UNSUPPORTED */
  return nullptr;
#endif
}

extern "C" JNIEXPORT jlong JNICALL
Java_app_eglviewer_core_EglNative_open(JNIEnv *env, jclass, jstring path,
                                       jstring version, jboolean syncFolder) {
#if defined(EGL_HAS_RUST)
  const char *path_utf = env->GetStringUTFChars(path, nullptr);
  const char *ver_utf = env->GetStringUTFChars(version, nullptr);
  EglLibrary *lib = egl_open(path_utf, ver_utf, syncFolder ? 1 : 0);
  env->ReleaseStringUTFChars(path, path_utf);
  env->ReleaseStringUTFChars(version, ver_utf);
  set_status(lib != nullptr ? 0 : 7);
  return reinterpret_cast<jlong>(lib);
#else
  (void)env;
  (void)path;
  (void)version;
  (void)syncFolder;
  set_status(6);
  return 0;
#endif
}

extern "C" JNIEXPORT void JNICALL
Java_app_eglviewer_core_EglNative_close(JNIEnv *, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  egl_close(reinterpret_cast<EglLibrary *>(handle));
#else
  (void)handle;
#endif
}

#if defined(EGL_HAS_RUST)
static jstring call_json(JNIEnv *env, EglLibrary *lib,
                         EglStatus (*fn)(EglLibrary *, char **)) {
  char *json = nullptr;
  EglStatus status = fn(lib, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
}
#endif

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_libraryInfo(JNIEnv *env, jclass,
                                              jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_library_info);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_counts(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_counts);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_folderTree(JNIEnv *env, jclass,
                                             jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_folder_tree);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

#if defined(EGL_HAS_RUST)
static void event_trampoline(void *ctx, const char *event_json) {
  if (g_vm == nullptr || event_json == nullptr) {
    return;
  }
  JNIEnv *env = nullptr;
  bool detach = false;
  jint get_env = g_vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6);
  if (get_env == JNI_EDETACHED) {
    if (g_vm->AttachCurrentThread(&env, nullptr) != 0 || env == nullptr) {
      return;
    }
    detach = true;
  } else if (get_env != JNI_OK || env == nullptr) {
    return;
  }

  jlong handle = reinterpret_cast<jlong>(ctx);
  jclass cls = env->FindClass("app/eglviewer/core/Library");
  if (cls != nullptr) {
    jmethodID mid = env->GetStaticMethodID(cls, "dispatchEventFromJni",
                                           "(JLjava/lang/String;)V");
    if (mid != nullptr) {
      jstring json = env->NewStringUTF(event_json);
      if (json != nullptr) {
        env->CallStaticVoidMethod(cls, mid, handle, json);
        env->DeleteLocalRef(json);
      }
    }
    env->DeleteLocalRef(cls);
  }

  if (detach) {
    g_vm->DetachCurrentThread();
  }
}
#endif

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_setEventCallback(JNIEnv *, jclass,
                                                   jlong handle,
                                                   jboolean enabled) {
#if defined(EGL_HAS_RUST)
  EglLibrary *lib = reinterpret_cast<EglLibrary *>(handle);
  EglStatus status;
  if (enabled) {
    status = egl_set_event_callback(lib, event_trampoline,
                                    reinterpret_cast<void *>(handle));
  } else {
    status = egl_set_event_callback(lib, nullptr, nullptr);
  }
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)handle;
  (void)enabled;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_lastError(JNIEnv *env, jclass,
                                            jlong handle) {
#if defined(EGL_HAS_RUST)
  const char *msg = egl_last_error(reinterpret_cast<EglLibrary *>(handle));
  return env->NewStringUTF(msg != nullptr ? msg : "");
#else
  (void)handle;
  return empty_string(env);
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_lastErrorGlobal(JNIEnv *env, jclass) {
#if defined(EGL_HAS_RUST)
  const char *msg = egl_last_error_global();
  return env->NewStringUTF(msg != nullptr ? msg : "");
#else
  return empty_string(env);
#endif
}

/* ---- Views by id (Android A2 ≈ iOS D-094) -------------------------------- */

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_openView(JNIEnv *env, jclass, jlong handle,
                                           jstring queryJson) {
#if defined(EGL_HAS_RUST)
  const char *query = nullptr;
  if (queryJson != nullptr) {
    query = env->GetStringUTFChars(queryJson, nullptr);
  }
  char *json = nullptr;
  EglStatus status =
      egl_open_view(reinterpret_cast<EglLibrary *>(handle), query, &json);
  set_status(static_cast<int>(status));
  if (queryJson != nullptr) {
    env->ReleaseStringUTFChars(queryJson, query);
  }
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)queryJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_viewRange(JNIEnv *env, jclass, jlong handle,
                                            jlong viewId, jint start, jint len) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status = egl_view_range(
      reinterpret_cast<EglLibrary *>(handle), static_cast<uint64_t>(viewId),
      static_cast<uint32_t>(start), static_cast<uint32_t>(len), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)viewId;
  (void)start;
  (void)len;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT void JNICALL
Java_app_eglviewer_core_EglNative_closeView(JNIEnv *, jclass, jlong handle,
                                            jlong viewId) {
#if defined(EGL_HAS_RUST)
  egl_close_view(reinterpret_cast<EglLibrary *>(handle),
                 static_cast<uint64_t>(viewId));
#else
  (void)handle;
  (void)viewId;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_tags(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_tags);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_extensions(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_extensions);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_smartFolders(JNIEnv *env, jclass,
                                               jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_smart_folders);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jbyteArray JNICALL
Java_app_eglviewer_core_EglNative_thumbnailBytes(JNIEnv *env, jclass,
                                                 jlong handle, jstring id) {
#if defined(EGL_HAS_RUST)
  const char *id_utf = env->GetStringUTFChars(id, nullptr);
  uint8_t *bytes = nullptr;
  size_t len = 0;
  char *content_type = nullptr;
  EglStatus status = egl_thumbnail_bytes(
      reinterpret_cast<EglLibrary *>(handle), id_utf, &bytes, &len, &content_type);
  env->ReleaseStringUTFChars(id, id_utf);
  if (content_type != nullptr) {
    egl_string_free(content_type);
  }
  set_status(static_cast<int>(status));
  if (status != EGL_OK || bytes == nullptr || len == 0) {
    return nullptr;
  }
  jbyteArray out = env->NewByteArray(static_cast<jsize>(len));
  if (out != nullptr) {
    env->SetByteArrayRegion(out, 0, static_cast<jsize>(len),
                            reinterpret_cast<const jbyte *>(bytes));
  }
  egl_bytes_free(bytes, len);
  return out;
#else
  (void)env;
  (void)handle;
  (void)id;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_editItems(JNIEnv *env, jclass, jlong handle,
                                            jstring targetJson,
                                            jstring editJson) {
#if defined(EGL_HAS_RUST)
  const char *target = env->GetStringUTFChars(targetJson, nullptr);
  const char *edit = env->GetStringUTFChars(editJson, nullptr);
  uint32_t count = 0;
  EglStatus status = egl_edit_items(reinterpret_cast<EglLibrary *>(handle),
                                    target, edit, &count);
  env->ReleaseStringUTFChars(targetJson, target);
  env->ReleaseStringUTFChars(editJson, edit);
  set_status(static_cast<int>(status));
  if (status != EGL_OK) {
    return -1;
  }
  return static_cast<jint>(count);
#else
  (void)env;
  (void)handle;
  (void)targetJson;
  (void)editJson;
  set_status(6);
  return -1;
#endif
}

/* ---- Preview / details / actions (Android A3 ≈ iOS D-095) ---------------- */

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_item(JNIEnv *env, jclass, jlong handle,
                                       jstring id) {
#if defined(EGL_HAS_RUST)
  const char *id_utf = env->GetStringUTFChars(id, nullptr);
  char *json = nullptr;
  EglStatus status =
      egl_item(reinterpret_cast<EglLibrary *>(handle), id_utf, &json);
  env->ReleaseStringUTFChars(id, id_utf);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)id;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_applyItemEdit(JNIEnv *env, jclass, jlong handle,
                                                jstring id, jstring editJson) {
#if defined(EGL_HAS_RUST)
  const char *id_utf = env->GetStringUTFChars(id, nullptr);
  const char *edit = env->GetStringUTFChars(editJson, nullptr);
  char *json = nullptr;
  EglStatus status = egl_apply_item_edit(reinterpret_cast<EglLibrary *>(handle),
                                         id_utf, edit, &json);
  env->ReleaseStringUTFChars(id, id_utf);
  env->ReleaseStringUTFChars(editJson, edit);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)editJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_viewSelectionTags(JNIEnv *env, jclass,
                                                    jlong handle,
                                                    jstring targetJson) {
#if defined(EGL_HAS_RUST)
  const char *target = env->GetStringUTFChars(targetJson, nullptr);
  char *json = nullptr;
  EglStatus status = egl_view_selection_tags(
      reinterpret_cast<EglLibrary *>(handle), target, &json);
  env->ReleaseStringUTFChars(targetJson, target);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)targetJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_undo(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status = egl_undo(reinterpret_cast<EglLibrary *>(handle), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK) {
    return nullptr;
  }
  /* null JSON means nothing to undo — return the string "null". */
  if (json == nullptr) {
    return env->NewStringUTF("null");
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_undoStatus(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status =
      egl_undo_status(reinterpret_cast<EglLibrary *>(handle), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK) {
    return nullptr;
  }
  if (json == nullptr) {
    return env->NewStringUTF("null");
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_deleteItems(JNIEnv *env, jclass, jlong handle,
                                              jstring targetJson) {
#if defined(EGL_HAS_RUST)
  const char *target = env->GetStringUTFChars(targetJson, nullptr);
  uint32_t count = 0;
  EglStatus status = egl_delete_items(reinterpret_cast<EglLibrary *>(handle),
                                      target, &count);
  env->ReleaseStringUTFChars(targetJson, target);
  set_status(static_cast<int>(status));
  if (status != EGL_OK) {
    return -1;
  }
  return static_cast<jint>(count);
#else
  (void)env;
  (void)handle;
  (void)targetJson;
  set_status(6);
  return -1;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_emptyTrash(JNIEnv *, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  uint32_t count = 0;
  EglStatus status =
      egl_empty_trash(reinterpret_cast<EglLibrary *>(handle), &count);
  set_status(static_cast<int>(status));
  if (status != EGL_OK) {
    return -1;
  }
  return static_cast<jint>(count);
#else
  (void)handle;
  set_status(6);
  return -1;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_trashInfo(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_trash_info);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_viewIds(JNIEnv *env, jclass, jlong handle,
                                          jlong viewId, jint start, jint len) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status = egl_view_ids(
      reinterpret_cast<EglLibrary *>(handle), static_cast<uint64_t>(viewId),
      static_cast<uint32_t>(start), static_cast<uint32_t>(len), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)viewId;
  (void)start;
  (void)len;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_originalPath(JNIEnv *env, jclass, jlong handle,
                                               jstring id) {
#if defined(EGL_HAS_RUST)
  const char *id_utf = env->GetStringUTFChars(id, nullptr);
  char *path = nullptr;
  EglStatus status =
      egl_original_path(reinterpret_cast<EglLibrary *>(handle), id_utf, &path);
  env->ReleaseStringUTFChars(id, id_utf);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || path == nullptr) {
    return nullptr;
  }
  return take_c_string(env, path);
#else
  (void)env;
  (void)handle;
  (void)id;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jbyteArray JNICALL
Java_app_eglviewer_core_EglNative_originalBytes(JNIEnv *env, jclass, jlong handle,
                                                jstring id) {
#if defined(EGL_HAS_RUST)
  const char *id_utf = env->GetStringUTFChars(id, nullptr);
  uint8_t *bytes = nullptr;
  size_t len = 0;
  char *content_type = nullptr;
  EglStatus status = egl_original_bytes(
      reinterpret_cast<EglLibrary *>(handle), id_utf, &bytes, &len, &content_type);
  env->ReleaseStringUTFChars(id, id_utf);
  if (content_type != nullptr) {
    egl_string_free(content_type);
  }
  set_status(static_cast<int>(status));
  if (status != EGL_OK || bytes == nullptr || len == 0) {
    return nullptr;
  }
  jbyteArray out = env->NewByteArray(static_cast<jsize>(len));
  if (out != nullptr) {
    env->SetByteArrayRegion(out, 0, static_cast<jsize>(len),
                            reinterpret_cast<const jbyte *>(bytes));
  }
  egl_bytes_free(bytes, len);
  return out;
#else
  (void)env;
  (void)handle;
  (void)id;
  set_status(6);
  return nullptr;
#endif
}
