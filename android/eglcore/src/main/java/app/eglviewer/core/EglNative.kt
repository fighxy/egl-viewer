package app.eglviewer.core

/**
 * JNI façades for `egl_*` (see `egl_jni.cpp`). Package-private to the core module.
 */
internal object EglNative {
    @Volatile
    var isAvailable: Boolean = false
        private set

    init {
        try {
            System.loadLibrary("egl_jni")
            isAvailable = true
        } catch (_: UnsatisfiedLinkError) {
            isAvailable = false
        }
    }

    external fun createLibrary(parent: String, name: String, appVersion: String): String?
    external fun open(path: String, appVersion: String, syncFolder: Boolean): Long
    external fun close(handle: Long)
    external fun libraryInfo(handle: Long): String?
    external fun counts(handle: Long): String?
    external fun folderTree(handle: Long): String?
    external fun setEventCallback(handle: Long, enabled: Boolean): Int
    external fun lastStatus(): Int
    external fun lastError(handle: Long): String
    external fun lastErrorGlobal(): String

    // Views by id (Android A2 ≈ D-094)
    external fun openView(handle: Long, queryJson: String?): String?
    external fun viewRange(handle: Long, viewId: Long, start: Int, len: Int): String?
    external fun closeView(handle: Long, viewId: Long)
    external fun tags(handle: Long): String?
    external fun extensions(handle: Long): String?
    external fun smartFolders(handle: Long): String?
    external fun thumbnailBytes(handle: Long, id: String): ByteArray?
    external fun editItems(handle: Long, targetJson: String, editJson: String): Int
}
