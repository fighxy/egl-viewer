/**
 * Thin JNI forwarder to the library-ffi C ABI (D-083 / D-085).
 * Mirrors ios NativeFfiSession / egl_library.h — no Eagle.
 */

#include <jni.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#if defined(EGL_HAS_RUST)
#include "egl_library.h"
#endif

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

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_bridge_EglNative_createLibrary(JNIEnv *env, jclass,
                                                  jstring parent, jstring name,
                                                  jstring version) {
#if defined(EGL_HAS_RUST)
  const char *parent_utf = env->GetStringUTFChars(parent, nullptr);
  const char *name_utf = env->GetStringUTFChars(name, nullptr);
  const char *ver_utf = env->GetStringUTFChars(version, nullptr);
  char *out_path = nullptr;
  EglStatus status =
      egl_create_library(parent_utf, name_utf, ver_utf, &out_path);
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
  return nullptr;
#endif
}

extern "C" JNIEXPORT jlong JNICALL
Java_app_eglviewer_bridge_EglNative_open(JNIEnv *env, jclass, jstring path,
                                         jstring version, jboolean syncFolder) {
#if defined(EGL_HAS_RUST)
  const char *path_utf = env->GetStringUTFChars(path, nullptr);
  const char *ver_utf = env->GetStringUTFChars(version, nullptr);
  EglLibrary *lib = egl_open(path_utf, ver_utf, syncFolder ? 1 : 0);
  env->ReleaseStringUTFChars(path, path_utf);
  env->ReleaseStringUTFChars(version, ver_utf);
  return reinterpret_cast<jlong>(lib);
#else
  (void)env;
  (void)path;
  (void)version;
  (void)syncFolder;
  return 0;
#endif
}

extern "C" JNIEXPORT void JNICALL
Java_app_eglviewer_bridge_EglNative_close(JNIEnv *, jclass, jlong handle) {
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
  if (fn(lib, &json) != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
}
#endif

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_bridge_EglNative_libraryInfo(JNIEnv *env, jclass,
                                                jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_library_info);
#else
  (void)env;
  (void)handle;
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_bridge_EglNative_viewWindow(JNIEnv *env, jclass, jlong handle,
                                               jstring scopeJson, jint start,
                                               jint len) {
#if defined(EGL_HAS_RUST)
  const char *scope = env->GetStringUTFChars(scopeJson, nullptr);
  char *json = nullptr;
  EglStatus status = egl_view_window(reinterpret_cast<EglLibrary *>(handle), scope,
                                     static_cast<uint32_t>(start),
                                     static_cast<uint32_t>(len), &json);
  env->ReleaseStringUTFChars(scopeJson, scope);
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)scopeJson;
  (void)start;
  (void)len;
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_bridge_EglNative_item(JNIEnv *env, jclass, jlong handle,
                                         jstring id) {
#if defined(EGL_HAS_RUST)
  const char *id_utf = env->GetStringUTFChars(id, nullptr);
  char *json = nullptr;
  EglStatus status =
      egl_item(reinterpret_cast<EglLibrary *>(handle), id_utf, &json);
  env->ReleaseStringUTFChars(id, id_utf);
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)id;
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_bridge_EglNative_folderTree(JNIEnv *env, jclass,
                                               jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_folder_tree);
#else
  (void)env;
  (void)handle;
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_bridge_EglNative_tags(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_tags);
#else
  (void)env;
  (void)handle;
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_bridge_EglNative_applyItemEdit(JNIEnv *env, jclass,
                                                  jlong handle, jstring id,
                                                  jstring editJson) {
#if defined(EGL_HAS_RUST)
  const char *id_utf = env->GetStringUTFChars(id, nullptr);
  const char *edit_utf = env->GetStringUTFChars(editJson, nullptr);
  char *json = nullptr;
  EglStatus status = egl_apply_item_edit(reinterpret_cast<EglLibrary *>(handle),
                                         id_utf, edit_utf, &json);
  env->ReleaseStringUTFChars(id, id_utf);
  env->ReleaseStringUTFChars(editJson, edit_utf);
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)editJson;
  return nullptr;
#endif
}

extern "C" JNIEXPORT jbyteArray JNICALL
Java_app_eglviewer_bridge_EglNative_thumbnailBytes(JNIEnv *env, jclass,
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
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_bridge_EglNative_lastError(JNIEnv *env, jclass,
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
Java_app_eglviewer_bridge_EglNative_lastErrorGlobal(JNIEnv *env, jclass) {
#if defined(EGL_HAS_RUST)
  const char *msg = egl_last_error_global();
  return env->NewStringUTF(msg != nullptr ? msg : "");
#else
  return empty_string(env);
#endif
}
