/**
 * Thin JNI forwarder to the library-ffi C ABI (D-083 / D-101).
 * A1 surface: create / open / close, library info, counts, folder tree, event callback.
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
