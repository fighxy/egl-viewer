/**
 * Thin JNI forwarder to the library-ffi C ABI (D-083 / D-101 / D-102 / D-103 / D-104).
 * Strings are standard UTF-8 to the core and back (Utf8, new_string); callbacks from the
 * core's threads use the classes cached in JNI_OnLoad (D-109).
 * A1: create / open / close, info, counts, folder tree, event callback.
 * A2: open_view / view_range / close_view, tags, extensions, smart folders, thumbs, edit_items.
 * A3: item, apply_item_edit, selection tags, undo, delete, empty trash, view ids, originals.
 * A4: add_files, answer/cancel import, import_status, platform decoder (HEIC via ImageDecoder).
 * A5: save/delete folder, tags/groups, save/delete smart folder, smart_rules.
 * A6: lock/unlock/relock/recover folder, change password, renew recovery key,
 *     remove lock, set lock label.
 * A7: rename library, stats, backups, check/repair, generate previews, export.
 * Package: app.eglviewer.core.EglNative
 */

#include <jni.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include <string>

#if defined(EGL_HAS_RUST)
#include "egl_library.h"
#endif

static JavaVM *g_vm = nullptr;
static thread_local int g_last_status = 0;

static void set_status(int status) { g_last_status = status; }

/*
 * Strings cross as standard UTF-8 both ways (D-109). GetStringUTFChars and NewStringUTF
 * speak Java's "modified UTF-8": a character outside the BMP (an emoji) becomes two
 * 3-byte surrogates, which the core rejects as invalid UTF-8, and NewStringUTF is not
 * guaranteed to take the core's 4-byte sequences. So the bridge converts from and to
 * UTF-16 itself; an unpaired surrogate or a broken sequence becomes U+FFFD.
 */
static void append_utf8(std::string &out, uint32_t cp) {
  if (cp < 0x80) {
    out.push_back(static_cast<char>(cp));
  } else if (cp < 0x800) {
    out.push_back(static_cast<char>(0xC0 | (cp >> 6)));
    out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
  } else if (cp < 0x10000) {
    out.push_back(static_cast<char>(0xE0 | (cp >> 12)));
    out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
    out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
  } else {
    out.push_back(static_cast<char>(0xF0 | (cp >> 18)));
    out.push_back(static_cast<char>(0x80 | ((cp >> 12) & 0x3F)));
    out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
    out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
  }
}

/** A Java string as standard UTF-8 for the core; get() is null for a null string. */
class Utf8 {
 public:
  Utf8(JNIEnv *env, jstring value) {
    if (value == nullptr) {
      return;
    }
    jsize len = env->GetStringLength(value);
    const jchar *chars = env->GetStringChars(value, nullptr);
    if (chars == nullptr) {
      return; /* out of memory: an exception is pending */
    }
    present_ = true;
    text_.reserve(static_cast<size_t>(len));
    for (jsize i = 0; i < len; ++i) {
      uint32_t unit = chars[i];
      if (unit >= 0xD800 && unit <= 0xDBFF && i + 1 < len && chars[i + 1] >= 0xDC00 &&
          chars[i + 1] <= 0xDFFF) {
        append_utf8(text_, 0x10000 + ((unit - 0xD800) << 10) + (chars[i + 1] - 0xDC00));
        ++i;
      } else if (unit >= 0xD800 && unit <= 0xDFFF) {
        append_utf8(text_, 0xFFFD);
      } else {
        append_utf8(text_, unit);
      }
    }
    env->ReleaseStringChars(value, chars);
  }
  Utf8(const Utf8 &) = delete;
  Utf8 &operator=(const Utf8 &) = delete;

  const char *get() const { return present_ ? text_.c_str() : nullptr; }

 private:
  bool present_ = false;
  std::string text_;
};

/** A Java string from the core's UTF-8. */
static jstring new_string(JNIEnv *env, const char *utf8) {
  std::u16string units;
  const auto *bytes = reinterpret_cast<const unsigned char *>(utf8 != nullptr ? utf8 : "");
  size_t len = strlen(reinterpret_cast<const char *>(bytes));
  units.reserve(len);
  for (size_t i = 0; i < len;) {
    unsigned char lead = bytes[i];
    uint32_t cp = 0xFFFD;
    size_t extra = 0;
    uint32_t min = 0;
    if (lead < 0x80) {
      cp = lead;
    } else if ((lead & 0xE0) == 0xC0) {
      cp = lead & 0x1F;
      extra = 1;
      min = 0x80;
    } else if ((lead & 0xF0) == 0xE0) {
      cp = lead & 0x0F;
      extra = 2;
      min = 0x800;
    } else if ((lead & 0xF8) == 0xF0) {
      cp = lead & 0x07;
      extra = 3;
      min = 0x10000;
    } else {
      extra = 0;
    }
    size_t used = 1;
    if (extra > 0) {
      bool ok = true;
      for (size_t k = 1; ok && k <= extra; ++k) {
        if (i + k >= len || (bytes[i + k] & 0xC0) != 0x80) {
          ok = false;
        } else {
          cp = (cp << 6) | (bytes[i + k] & 0x3F);
        }
      }
      if (ok && cp >= min && cp <= 0x10FFFF && !(cp >= 0xD800 && cp <= 0xDFFF)) {
        used = extra + 1;
      } else {
        cp = 0xFFFD;
      }
    } else if (lead >= 0x80) {
      cp = 0xFFFD;
    }
    if (cp >= 0x10000) {
      cp -= 0x10000;
      units.push_back(static_cast<char16_t>(0xD800 + (cp >> 10)));
      units.push_back(static_cast<char16_t>(0xDC00 + (cp & 0x3FF)));
    } else {
      units.push_back(static_cast<char16_t>(cp));
    }
    i += used;
  }
  return env->NewString(reinterpret_cast<const jchar *>(units.data()),
                        static_cast<jsize>(units.size()));
}

static jstring empty_string(JNIEnv *env) { return new_string(env, ""); }

static jstring take_c_string(JNIEnv *env, char *owned) {
  if (owned == nullptr) {
    return empty_string(env);
  }
  jstring out = new_string(env, owned);
#if defined(EGL_HAS_RUST)
  egl_string_free(owned);
#else
  free(owned);
#endif
  return out;
}

/*
 * The core calls back on threads it started (imports, exports, previews). A FindClass on
 * such a thread searches the system class loader, which cannot see the app's classes
 * (NoClassDefFoundError, D-109), so the classes and methods the callbacks need are found
 * once here, where the app's class loader is in reach, and kept as global references.
 */
static jclass g_library_class = nullptr;
static jmethodID g_dispatch_event = nullptr;
static jclass g_decoder_class = nullptr;
static jmethodID g_decode_from_jni = nullptr;

static jclass global_class(JNIEnv *env, const char *name) {
  jclass local = env->FindClass(name);
  if (local == nullptr) {
    env->ExceptionClear();
    return nullptr;
  }
  auto global = static_cast<jclass>(env->NewGlobalRef(local));
  env->DeleteLocalRef(local);
  return global;
}

static jmethodID static_method(JNIEnv *env, jclass cls, const char *name, const char *sig) {
  if (cls == nullptr) {
    return nullptr;
  }
  jmethodID id = env->GetStaticMethodID(cls, name, sig);
  if (id == nullptr) {
    env->ExceptionClear();
  }
  return id;
}

JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM *vm, void *) {
  g_vm = vm;
  JNIEnv *env = nullptr;
  if (vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) != JNI_OK || env == nullptr) {
    return JNI_ERR;
  }
  g_library_class = global_class(env, "app/eglviewer/core/Library");
  g_dispatch_event =
      static_method(env, g_library_class, "dispatchEventFromJni", "(JLjava/lang/String;)V");
  g_decoder_class = global_class(env, "app/eglviewer/core/PlatformDecoder");
  g_decode_from_jni =
      static_method(env, g_decoder_class, "decodeFromJni", "([BLjava/lang/String;J)I");
  return JNI_VERSION_1_6;
}

/** The JNIEnv of this thread, attaching a thread the core started (then *attached is set). */
static JNIEnv *thread_env(bool *attached) {
  *attached = false;
  if (g_vm == nullptr) {
    return nullptr;
  }
  JNIEnv *env = nullptr;
  jint got = g_vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6);
  if (got == JNI_OK) {
    return env;
  }
  if (got != JNI_EDETACHED) {
    return nullptr;
  }
#if defined(__ANDROID__)
  jint attach = g_vm->AttachCurrentThread(&env, nullptr);
#else
  jint attach = g_vm->AttachCurrentThread(reinterpret_cast<void **>(&env), nullptr);
#endif
  if (attach != JNI_OK || env == nullptr) {
    return nullptr;
  }
  *attached = true;
  return env;
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
  Utf8 parent_utf_arg(env, parent);
  const char *parent_utf = parent_utf_arg.get();
  Utf8 name_utf_arg(env, name);
  const char *name_utf = name_utf_arg.get();
  Utf8 ver_utf_arg(env, version);
  const char *ver_utf = ver_utf_arg.get();
  char *out_path = nullptr;
  EglStatus status =
      egl_create_library(parent_utf, name_utf, ver_utf, &out_path);
  set_status(static_cast<int>(status));
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

/* Decrypt Archive (D-110 / D-111): no library handle; DecryptedArchive JSON or null. */
extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_decryptArchive(JNIEnv *env, jclass,
                                                 jstring source, jstring destination,
                                                 jstring passphrase) {
#if defined(EGL_HAS_RUST)
  Utf8 source_arg(env, source);
  Utf8 destination_arg(env, destination);
  Utf8 passphrase_arg(env, passphrase);
  char *json = nullptr;
  EglStatus status = egl_decrypt_archive(source_arg.get(), destination_arg.get(),
                                         passphrase_arg.get(), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)source;
  (void)destination;
  (void)passphrase;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jlong JNICALL
Java_app_eglviewer_core_EglNative_open(JNIEnv *env, jclass, jstring path,
                                       jstring version, jboolean syncFolder) {
#if defined(EGL_HAS_RUST)
  Utf8 path_utf_arg(env, path);
  const char *path_utf = path_utf_arg.get();
  Utf8 ver_utf_arg(env, version);
  const char *ver_utf = ver_utf_arg.get();
  EglLibrary *lib = egl_open(path_utf, ver_utf, syncFolder ? 1 : 0);
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
  if (event_json == nullptr || g_library_class == nullptr || g_dispatch_event == nullptr) {
    return;
  }
  bool attached = false;
  JNIEnv *env = thread_env(&attached);
  if (env == nullptr) {
    return;
  }
  jlong handle = reinterpret_cast<jlong>(ctx);
  jstring json = new_string(env, event_json);
  if (json != nullptr) {
    env->CallStaticVoidMethod(g_library_class, g_dispatch_event, handle, json);
    env->DeleteLocalRef(json);
  }
  /* A throwing handler must not leave an exception on the core's thread. */
  if (env->ExceptionCheck()) {
    env->ExceptionClear();
  }
  if (attached) {
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
  return new_string(env, msg != nullptr ? msg : "");
#else
  (void)handle;
  return empty_string(env);
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_lastErrorGlobal(JNIEnv *env, jclass) {
#if defined(EGL_HAS_RUST)
  const char *msg = egl_last_error_global();
  return new_string(env, msg != nullptr ? msg : "");
#else
  return empty_string(env);
#endif
}

/* ---- Views by id (Android A2 ≈ iOS D-094) -------------------------------- */

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_openView(JNIEnv *env, jclass, jlong handle,
                                           jstring queryJson) {
#if defined(EGL_HAS_RUST)
  Utf8 query_arg(env, queryJson);
  const char *query = query_arg.get();
  char *json = nullptr;
  EglStatus status =
      egl_open_view(reinterpret_cast<EglLibrary *>(handle), query, &json);
  set_status(static_cast<int>(status));
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
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  uint8_t *bytes = nullptr;
  size_t len = 0;
  char *content_type = nullptr;
  EglStatus status = egl_thumbnail_bytes(
      reinterpret_cast<EglLibrary *>(handle), id_utf, &bytes, &len, &content_type);
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
  Utf8 target_arg(env, targetJson);
  const char *target = target_arg.get();
  Utf8 edit_arg(env, editJson);
  const char *edit = edit_arg.get();
  uint32_t count = 0;
  EglStatus status = egl_edit_items(reinterpret_cast<EglLibrary *>(handle),
                                    target, edit, &count);
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
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  char *json = nullptr;
  EglStatus status =
      egl_item(reinterpret_cast<EglLibrary *>(handle), id_utf, &json);
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
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  Utf8 edit_arg(env, editJson);
  const char *edit = edit_arg.get();
  char *json = nullptr;
  EglStatus status = egl_apply_item_edit(reinterpret_cast<EglLibrary *>(handle),
                                         id_utf, edit, &json);
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
  Utf8 target_arg(env, targetJson);
  const char *target = target_arg.get();
  char *json = nullptr;
  EglStatus status = egl_view_selection_tags(
      reinterpret_cast<EglLibrary *>(handle), target, &json);
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
    return new_string(env, "null");
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
    return new_string(env, "null");
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
  Utf8 target_arg(env, targetJson);
  const char *target = target_arg.get();
  uint32_t count = 0;
  EglStatus status = egl_delete_items(reinterpret_cast<EglLibrary *>(handle),
                                      target, &count);
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
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  char *path = nullptr;
  EglStatus status =
      egl_original_path(reinterpret_cast<EglLibrary *>(handle), id_utf, &path);
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
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  uint8_t *bytes = nullptr;
  size_t len = 0;
  char *content_type = nullptr;
  EglStatus status = egl_original_bytes(
      reinterpret_cast<EglLibrary *>(handle), id_utf, &bytes, &len, &content_type);
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

/* ---- Adding files (Android A4 ≈ iOS D-096) -------------------------------- */

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_addFiles(JNIEnv *env, jclass, jlong handle,
                                           jstring requestJson) {
#if defined(EGL_HAS_RUST)
  Utf8 req_arg(env, requestJson);
  const char *req = req_arg.get();
  char *out = nullptr;
  EglStatus status =
      egl_add_files(reinterpret_cast<EglLibrary *>(handle), req, &out);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || out == nullptr) {
    return nullptr;
  }
  return take_c_string(env, out);
#else
  (void)env;
  (void)handle;
  (void)requestJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_answerImport(JNIEnv *env, jclass, jlong handle,
                                               jlong id, jstring answer) {
#if defined(EGL_HAS_RUST)
  Utf8 ans_arg(env, answer);
  const char *ans = ans_arg.get();
  EglStatus status = egl_answer_import(reinterpret_cast<EglLibrary *>(handle),
                                       static_cast<uint64_t>(id), ans);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)answer;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_cancelImport(JNIEnv *, jclass, jlong handle,
                                               jlong id) {
#if defined(EGL_HAS_RUST)
  EglStatus status = egl_cancel_import(reinterpret_cast<EglLibrary *>(handle),
                                       static_cast<uint64_t>(id));
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)handle;
  (void)id;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_importStatus(JNIEnv *env, jclass, jlong handle,
                                               jlong id) {
#if defined(EGL_HAS_RUST)
  char *out = nullptr;
  EglStatus status = egl_import_status(reinterpret_cast<EglLibrary *>(handle),
                                       static_cast<uint64_t>(id), &out);
  set_status(static_cast<int>(status));
  if (status != EGL_OK) {
    return nullptr;
  }
  // JSON null when the import has not reported yet.
  if (out == nullptr) {
    return new_string(env, "null");
  }
  return take_c_string(env, out);
#else
  (void)env;
  (void)handle;
  (void)id;
  set_status(6);
  return nullptr;
#endif
}

#if defined(EGL_HAS_RUST)
static int platform_decode_trampoline(void * /*ctx*/, const uint8_t *bytes,
                                      size_t len, const char *ext_utf8,
                                      EglDecodeSink *sink) {
  if (bytes == nullptr || sink == nullptr || len == 0 || g_decoder_class == nullptr ||
      g_decode_from_jni == nullptr || len > static_cast<size_t>(INT32_MAX)) {
    return 0;
  }
  bool attached = false;
  JNIEnv *env = thread_env(&attached);
  if (env == nullptr) {
    return 0;
  }
  int ok = 0;
  jbyteArray arr = env->NewByteArray(static_cast<jsize>(len));
  if (arr != nullptr) {
    env->SetByteArrayRegion(arr, 0, static_cast<jsize>(len),
                            reinterpret_cast<const jbyte *>(bytes));
    jstring ext = new_string(env, ext_utf8 != nullptr ? ext_utf8 : "");
    if (ext != nullptr) {
      ok = env->CallStaticIntMethod(g_decoder_class, g_decode_from_jni, arr, ext,
                                    reinterpret_cast<jlong>(sink));
      env->DeleteLocalRef(ext);
    }
    env->DeleteLocalRef(arr);
  }
  if (env->ExceptionCheck()) {
    env->ExceptionClear();
    ok = 0;
  }
  if (attached) {
    g_vm->DetachCurrentThread();
  }
  return ok != 0 ? 1 : 0;
}
#endif

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_setPlatformDecoder(JNIEnv *, jclass,
                                                     jboolean enabled) {
#if defined(EGL_HAS_RUST)
  EglStatus status;
  if (enabled) {
    status = egl_set_platform_decoder(platform_decode_trampoline, nullptr);
  } else {
    status = egl_set_platform_decoder(nullptr, nullptr);
  }
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)enabled;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT void JNICALL
Java_app_eglviewer_core_EglNative_decodeSinkPut(JNIEnv *env, jclass,
                                                jlong sinkPtr, jint width,
                                                jint height, jint pixelWidth,
                                                jint pixelHeight,
                                                jbyteArray rgba) {
#if defined(EGL_HAS_RUST)
  if (sinkPtr == 0 || rgba == nullptr) {
    return;
  }
  jsize len = env->GetArrayLength(rgba);
  jbyte *bytes = env->GetByteArrayElements(rgba, nullptr);
  if (bytes == nullptr) {
    return;
  }
  egl_decode_sink_put(reinterpret_cast<EglDecodeSink *>(sinkPtr),
                      static_cast<uint32_t>(width),
                      static_cast<uint32_t>(height),
                      static_cast<uint32_t>(pixelWidth),
                      static_cast<uint32_t>(pixelHeight),
                      reinterpret_cast<const uint8_t *>(bytes),
                      static_cast<size_t>(len));
  env->ReleaseByteArrayElements(rgba, bytes, JNI_ABORT);
#else
  (void)env;
  (void)sinkPtr;
  (void)width;
  (void)height;
  (void)pixelWidth;
  (void)pixelHeight;
  (void)rgba;
#endif
}

/* ---- Organizing (Android A5 ≈ iOS D-097) --------------------------------- */

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_saveFolder(JNIEnv *env, jclass, jlong handle,
                                             jstring changeJson) {
#if defined(EGL_HAS_RUST)
  Utf8 change_arg(env, changeJson);
  const char *change = change_arg.get();
  char *json = nullptr;
  EglStatus status = egl_save_folder(reinterpret_cast<EglLibrary *>(handle),
                                     change, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)changeJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_deleteFolder(JNIEnv *env, jclass, jlong handle,
                                               jstring id, jboolean trashItems) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  char *json = nullptr;
  EglStatus status = egl_delete_folder(reinterpret_cast<EglLibrary *>(handle),
                                       id_utf, trashItems ? 1 : 0, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)trashItems;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_renameTag(JNIEnv *env, jclass, jlong handle,
                                            jstring from, jstring to) {
#if defined(EGL_HAS_RUST)
  Utf8 from_utf_arg(env, from);
  const char *from_utf = from_utf_arg.get();
  Utf8 to_utf_arg(env, to);
  const char *to_utf = to_utf_arg.get();
  EglStatus status = egl_rename_tag(reinterpret_cast<EglLibrary *>(handle),
                                    from_utf, to_utf);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)from;
  (void)to;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_mergeTags(JNIEnv *env, jclass, jlong handle,
                                            jstring source, jstring target) {
#if defined(EGL_HAS_RUST)
  Utf8 source_utf_arg(env, source);
  const char *source_utf = source_utf_arg.get();
  Utf8 target_utf_arg(env, target);
  const char *target_utf = target_utf_arg.get();
  EglStatus status = egl_merge_tags(reinterpret_cast<EglLibrary *>(handle),
                                    source_utf, target_utf);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)source;
  (void)target;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_deleteTag(JNIEnv *env, jclass, jlong handle,
                                            jstring name) {
#if defined(EGL_HAS_RUST)
  Utf8 name_utf_arg(env, name);
  const char *name_utf = name_utf_arg.get();
  EglStatus status =
      egl_delete_tag(reinterpret_cast<EglLibrary *>(handle), name_utf);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)name;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_tagGroups(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  return call_json(env, reinterpret_cast<EglLibrary *>(handle), egl_tag_groups);
#else
  (void)env;
  (void)handle;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_saveTagGroup(JNIEnv *env, jclass, jlong handle,
                                               jstring changeJson) {
#if defined(EGL_HAS_RUST)
  Utf8 change_arg(env, changeJson);
  const char *change = change_arg.get();
  char *json = nullptr;
  EglStatus status = egl_save_tag_group(reinterpret_cast<EglLibrary *>(handle),
                                        change, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)changeJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_deleteTagGroup(JNIEnv *env, jclass,
                                                 jlong handle, jstring id) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  char *json = nullptr;
  EglStatus status = egl_delete_tag_group(reinterpret_cast<EglLibrary *>(handle),
                                          id_utf, &json);
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
Java_app_eglviewer_core_EglNative_saveSmartFolder(JNIEnv *env, jclass,
                                                  jlong handle,
                                                  jstring changeJson) {
#if defined(EGL_HAS_RUST)
  Utf8 change_arg(env, changeJson);
  const char *change = change_arg.get();
  char *json = nullptr;
  EglStatus status = egl_save_smart_folder(
      reinterpret_cast<EglLibrary *>(handle), change, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)changeJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_deleteSmartFolder(JNIEnv *env, jclass,
                                                    jlong handle, jstring id) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  char *json = nullptr;
  EglStatus status = egl_delete_smart_folder(
      reinterpret_cast<EglLibrary *>(handle), id_utf, &json);
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
Java_app_eglviewer_core_EglNative_smartRules(JNIEnv *env, jclass, jlong handle,
                                             jstring queryJson) {
#if defined(EGL_HAS_RUST)
  Utf8 query_arg(env, queryJson);
  const char *query = query_arg.get();
  char *json = nullptr;
  EglStatus status = egl_smart_rules(reinterpret_cast<EglLibrary *>(handle),
                                     query, &json);
  set_status(static_cast<int>(status));
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

/* ---- Locked folders (Android A6 ≈ iOS D-098) ----------------------------- */

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_lockFolder(JNIEnv *env, jclass, jlong handle,
                                             jstring id, jstring requestJson) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  Utf8 request_arg(env, requestJson);
  const char *request = request_arg.get();
  char *json = nullptr;
  EglStatus status = egl_lock_folder(reinterpret_cast<EglLibrary *>(handle),
                                     id_utf, request, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)requestJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_unlockFolder(JNIEnv *env, jclass, jlong handle,
                                               jstring id, jstring password) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  Utf8 password_utf_arg(env, password);
  const char *password_utf = password_utf_arg.get();
  EglStatus status = egl_unlock_folder(reinterpret_cast<EglLibrary *>(handle),
                                       id_utf, password_utf);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)password;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_relockFolder(JNIEnv *env, jclass, jlong handle,
                                               jstring id) {
#if defined(EGL_HAS_RUST)
  Utf8 id_arg(env, id);
  const char *id_utf = id_arg.get();
  EglStatus status =
      egl_relock_folder(reinterpret_cast<EglLibrary *>(handle), id_utf);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)id;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_recoverFolder(JNIEnv *env, jclass, jlong handle,
                                                jstring id, jstring recovery,
                                                jstring newPassword) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  Utf8 recovery_utf_arg(env, recovery);
  const char *recovery_utf = recovery_utf_arg.get();
  Utf8 password_utf_arg(env, newPassword);
  const char *password_utf = password_utf_arg.get();
  EglStatus status = egl_recover_folder(reinterpret_cast<EglLibrary *>(handle),
                                        id_utf, recovery_utf, password_utf);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)recovery;
  (void)newPassword;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_changeFolderPassword(JNIEnv *env, jclass,
                                                       jlong handle, jstring id,
                                                       jstring current,
                                                       jstring newPassword) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  Utf8 current_utf_arg(env, current);
  const char *current_utf = current_utf_arg.get();
  Utf8 password_utf_arg(env, newPassword);
  const char *password_utf = password_utf_arg.get();
  EglStatus status = egl_change_folder_password(
      reinterpret_cast<EglLibrary *>(handle), id_utf, current_utf, password_utf);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)current;
  (void)newPassword;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_renewRecoveryKey(JNIEnv *env, jclass,
                                                   jlong handle, jstring id,
                                                   jstring password) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  Utf8 password_utf_arg(env, password);
  const char *password_utf = password_utf_arg.get();
  char *json = nullptr;
  EglStatus status = egl_renew_recovery_key(
      reinterpret_cast<EglLibrary *>(handle), id_utf, password_utf, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)password;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_removeFolderLock(JNIEnv *env, jclass,
                                                   jlong handle, jstring id,
                                                   jstring password) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  Utf8 password_utf_arg(env, password);
  const char *password_utf = password_utf_arg.get();
  EglStatus status = egl_remove_folder_lock(
      reinterpret_cast<EglLibrary *>(handle), id_utf, password_utf);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)password;
  set_status(6);
  return 6;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_app_eglviewer_core_EglNative_setLockLabel(JNIEnv *env, jclass, jlong handle,
                                               jstring id, jstring label,
                                               jstring color) {
#if defined(EGL_HAS_RUST)
  Utf8 id_utf_arg(env, id);
  const char *id_utf = id_utf_arg.get();
  Utf8 label_arg(env, label);
  const char *label_utf = label_arg.get();
  Utf8 color_arg(env, color);
  const char *color_utf = color_arg.get();
  EglStatus status = egl_set_lock_label(reinterpret_cast<EglLibrary *>(handle),
                                        id_utf, label_utf, color_utf);
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)env;
  (void)handle;
  (void)id;
  (void)label;
  (void)color;
  set_status(6);
  return 6;
#endif
}

/* ---- Settings / maintenance / export (Android A7 ≈ iOS D-099) ------------ */

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_renameLibrary(JNIEnv *env, jclass, jlong handle,
                                                jstring name) {
#if defined(EGL_HAS_RUST)
  Utf8 name_utf_arg(env, name);
  const char *name_utf = name_utf_arg.get();
  char *json = nullptr;
  EglStatus status = egl_rename_library(reinterpret_cast<EglLibrary *>(handle),
                                        name_utf, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)name;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_libraryStats(JNIEnv *env, jclass,
                                               jlong handle) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status =
      egl_library_stats(reinterpret_cast<EglLibrary *>(handle), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
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
Java_app_eglviewer_core_EglNative_backups(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status = egl_backups(reinterpret_cast<EglLibrary *>(handle), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
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
Java_app_eglviewer_core_EglNative_backUpNow(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status =
      egl_back_up_now(reinterpret_cast<EglLibrary *>(handle), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
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
Java_app_eglviewer_core_EglNative_restoreBackup(JNIEnv *env, jclass,
                                                jlong handle, jstring name) {
#if defined(EGL_HAS_RUST)
  Utf8 name_utf_arg(env, name);
  const char *name_utf = name_utf_arg.get();
  char *json = nullptr;
  EglStatus status = egl_restore_backup(reinterpret_cast<EglLibrary *>(handle),
                                        name_utf, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)name;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_checkLibrary(JNIEnv *env, jclass,
                                               jlong handle) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status =
      egl_check_library(reinterpret_cast<EglLibrary *>(handle), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
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
Java_app_eglviewer_core_EglNative_repairLibrary(JNIEnv *env, jclass,
                                                jlong handle,
                                                jstring optionsJson) {
#if defined(EGL_HAS_RUST)
  Utf8 options_arg(env, optionsJson);
  const char *options = options_arg.get();
  char *json = nullptr;
  EglStatus status = egl_repair_library(reinterpret_cast<EglLibrary *>(handle),
                                        options, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)optionsJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_generatePreviews(JNIEnv *env, jclass,
                                                   jlong handle,
                                                   jboolean rebuild) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status = egl_generate_previews(
      reinterpret_cast<EglLibrary *>(handle), rebuild ? 1 : 0, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)rebuild;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_exportPreview(JNIEnv *env, jclass,
                                                jlong handle,
                                                jstring selectionJson,
                                                jboolean includeSubfolders) {
#if defined(EGL_HAS_RUST)
  Utf8 selection_arg(env, selectionJson);
  const char *selection = selection_arg.get();
  char *json = nullptr;
  EglStatus status = egl_export_preview(
      reinterpret_cast<EglLibrary *>(handle), selection,
      includeSubfolders ? 1 : 0, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)selectionJson;
  (void)includeSubfolders;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_startExport(JNIEnv *env, jclass, jlong handle,
                                              jstring requestJson) {
#if defined(EGL_HAS_RUST)
  Utf8 request_arg(env, requestJson);
  const char *request = request_arg.get();
  char *json = nullptr;
  EglStatus status = egl_start_export(reinterpret_cast<EglLibrary *>(handle),
                                      request, &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
  }
  return take_c_string(env, json);
#else
  (void)env;
  (void)handle;
  (void)requestJson;
  set_status(6);
  return nullptr;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_app_eglviewer_core_EglNative_exports(JNIEnv *env, jclass, jlong handle) {
#if defined(EGL_HAS_RUST)
  char *json = nullptr;
  EglStatus status = egl_exports(reinterpret_cast<EglLibrary *>(handle), &json);
  set_status(static_cast<int>(status));
  if (status != EGL_OK || json == nullptr) {
    return nullptr;
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
Java_app_eglviewer_core_EglNative_cancelExport(JNIEnv *, jclass, jlong handle,
                                               jlong id) {
#if defined(EGL_HAS_RUST)
  EglStatus status = egl_cancel_export(reinterpret_cast<EglLibrary *>(handle),
                                       static_cast<uint64_t>(id));
  set_status(static_cast<int>(status));
  return static_cast<jint>(status);
#else
  (void)handle;
  (void)id;
  set_status(6);
  return 6;
#endif
}
