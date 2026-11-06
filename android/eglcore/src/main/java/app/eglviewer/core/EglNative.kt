package app.eglviewer.core

/**
 * JNI façades for `egl_*` (see `egl_jni.cpp`). Package-private to the core module.
 */
internal object EglNative {
    @Volatile
    var isAvailable: Boolean = false
        private set

    /** Why the bridge did not load, as the system's loader said it (D-115). */
    @Volatile
    var loadError: String? = null
        private set

    init {
        try {
            System.loadLibrary("egl_jni")
            isAvailable = true
        } catch (error: UnsatisfiedLinkError) {
            isAvailable = false
            loadError = error.message ?: error.toString()
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

    // Organizing (Android A5 ≈ D-097)
    external fun saveFolder(handle: Long, changeJson: String): String?
    external fun deleteFolder(handle: Long, id: String, trashItems: Boolean): String?
    external fun renameTag(handle: Long, from: String, to: String): Int
    external fun mergeTags(handle: Long, source: String, target: String): Int
    external fun deleteTag(handle: Long, name: String): Int
    external fun tagGroups(handle: Long): String?
    external fun saveTagGroup(handle: Long, changeJson: String): String?
    external fun deleteTagGroup(handle: Long, id: String): String?
    external fun saveSmartFolder(handle: Long, changeJson: String): String?
    external fun deleteSmartFolder(handle: Long, id: String): String?
    external fun smartRules(handle: Long, queryJson: String): String?

    // Locked folders (Android A6 ≈ D-098)
    external fun lockFolder(handle: Long, id: String, requestJson: String): String?
    external fun unlockFolder(handle: Long, id: String, password: String): Int
    /** [id] null relocks every open folder. */
    external fun relockFolder(handle: Long, id: String?): Int
    external fun recoverFolder(handle: Long, id: String, recovery: String, newPassword: String): Int
    external fun changeFolderPassword(handle: Long, id: String, current: String, newPassword: String): Int
    external fun renewRecoveryKey(handle: Long, id: String, password: String): String?
    external fun removeFolderLock(handle: Long, id: String, password: String): Int
    external fun setLockLabel(handle: Long, id: String, label: String?, color: String?): Int

    // Settings / maintenance / export (Android A7 ≈ D-099)
    external fun renameLibrary(handle: Long, name: String): String?
    external fun libraryStats(handle: Long): String?
    external fun backups(handle: Long): String?
    external fun backUpNow(handle: Long): String?
    external fun restoreBackup(handle: Long, name: String): String?
    external fun checkLibrary(handle: Long): String?
    external fun repairLibrary(handle: Long, optionsJson: String): String?
    external fun generatePreviews(handle: Long, rebuild: Boolean): String?
    external fun exportPreview(handle: Long, selectionJson: String, includeSubfolders: Boolean): String?
    external fun startExport(handle: Long, requestJson: String): String?
    external fun exports(handle: Long): String?
    external fun cancelExport(handle: Long, id: Long): Int

    // Decrypt Archive (D-110 / D-111): no library handle.
    external fun decryptArchive(source: String, destination: String, passphrase: String): String?
}
