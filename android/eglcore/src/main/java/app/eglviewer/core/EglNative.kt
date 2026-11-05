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

    // Preview / details / actions (Android A3 ≈ D-095)
    external fun item(handle: Long, id: String): String?
    external fun applyItemEdit(handle: Long, id: String, editJson: String): String?
    external fun viewSelectionTags(handle: Long, targetJson: String): String?
    external fun undo(handle: Long): String?
    external fun undoStatus(handle: Long): String?
    external fun deleteItems(handle: Long, targetJson: String): Int
    external fun emptyTrash(handle: Long): Int
    external fun trashInfo(handle: Long): String?
    external fun viewIds(handle: Long, viewId: Long, start: Int, len: Int): String?
    external fun originalPath(handle: Long, id: String): String?
    external fun originalBytes(handle: Long, id: String): ByteArray?

    // Adding (Android A4 ≈ D-096)
    external fun addFiles(handle: Long, requestJson: String): String?
    external fun answerImport(handle: Long, id: Long, answer: String): Int
    external fun cancelImport(handle: Long, id: Long): Int
    external fun importStatus(handle: Long, id: Long): String?
    external fun setPlatformDecoder(enabled: Boolean): Int
    external fun decodeSinkPut(
        sinkPtr: Long,
        width: Int,
        height: Int,
        pixelWidth: Int,
        pixelHeight: Int,
        rgba: ByteArray,
    )
}
