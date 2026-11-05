package app.eglviewer.core

import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicReference

/**
 * Typed Kotlin face of the C ABI (`crates/library-ffi`), one [Library] per open `.eglib`.
 * Mirrors Swift `EglCore.Library` for A1–A7: create / open / close, views, thumbs, item edits,
 * undo, trash, originals, adding files, organizing (folders / tags / smart folders),
 * locked folders, and settings (stats / backups / check / repair / export). Calls may block —
 * run them off the main thread.
 */
class Library private constructor(private var handle: Long) : AutoCloseable {
    private val eventHandler = AtomicReference<((LibraryEvent) -> Unit)?>(null)

    /** Sets what hears the core's events (`null` clears). May run on a background thread. */
    fun setEventHandler(handler: ((LibraryEvent) -> Unit)?) {
        ensureOpen()
        eventHandler.set(handler)
        val status = EglNative.setEventCallback(handle, handler != null)
        if (status != 0) {
            eventHandler.set(null)
            throw error(status)
        }
        if (handler != null) {
            activeLibraries[handle] = this
        } else {
            activeLibraries.remove(handle)
        }
    }

    fun info(): LibraryInfo {
        ensureOpen()
        val json = EglNative.libraryInfo(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return LibraryInfo.fromJson(json)
    }

    fun counts(): LibraryCounts {
        ensureOpen()
        val json = EglNative.counts(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return LibraryCounts.fromJson(json)
    }

    /** Folder tree as the core's JSON array. */
    fun folderTreeJson(): String {
        ensureOpen()
        val json = EglNative.folderTree(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return json
    }

    fun folders(): List<Folder> = Folder.listFromJson(folderTreeJson())

    fun smartFolders(): List<SmartFolder> {
        ensureOpen()
        val json = EglNative.smartFolders(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return SmartFolder.listFromJson(json)
    }

    fun tags(): List<Tag> {
        ensureOpen()
        val json = EglNative.tags(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return Tag.listFromJson(json)
    }

    fun extensions(): List<ExtensionCount> {
        ensureOpen()
        val json = EglNative.extensions(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return ExtensionCount.listFromJson(json)
    }

    /** Opens a view of [query]; the item list stays in Rust (D-094). */
    fun openView(query: LibraryQuery = LibraryQuery()): ViewOpened {
        ensureOpen()
        val json = EglNative.openView(handle, query.toJson().toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return ViewOpened.fromJson(org.json.JSONObject(json))
    }

    fun range(viewId: Long, start: Int, count: Int): List<ItemCell> {
        ensureOpen()
        val json = EglNative.viewRange(handle, viewId, start, count)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        val arr = org.json.JSONArray(json)
        return (0 until arr.length()).map { ItemCell.fromJson(arr.getJSONObject(it)) }
    }

    fun closeView(viewId: Long) {
        if (handle == 0L) return
        EglNative.closeView(handle, viewId)
    }

    fun thumbnailBytes(id: String): ByteArray? {
        ensureOpen()
        return EglNative.thumbnailBytes(handle, id)
    }

    fun item(id: String): Item {
        ensureOpen()
        val json = EglNative.item(handle, id)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return Item.fromJson(json)
    }

    fun applyItemEdit(id: String, edit: ItemEdit): Item {
        ensureOpen()
        val json = EglNative.applyItemEdit(handle, id, edit.toJson().toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return Item.fromJson(json)
    }

    fun editItems(target: ItemsTarget, edit: ItemEdit): Int {
        ensureOpen()
        val count = EglNative.editItems(
            handle,
            target.toJson().toString(),
            edit.toJson().toString(),
        )
        if (count < 0) throw error(EglNative.lastStatus())
        return count
    }

    fun selectionTags(target: ItemsTarget): SelectionTags {
        ensureOpen()
        val json = EglNative.viewSelectionTags(handle, target.toJson().toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return SelectionTags.fromJson(json)
    }

    fun undo(): UndoStatus? {
        ensureOpen()
        val json = EglNative.undo(handle)
        val status = EglNative.lastStatus()
        if (status != 0) throw error(status)
        return UndoStatus.fromJson(json)
    }

    fun undoStatus(): UndoStatus? {
        ensureOpen()
        val json = EglNative.undoStatus(handle)
        val status = EglNative.lastStatus()
        if (status != 0) throw error(status)
        return UndoStatus.fromJson(json)
    }

    fun deleteItems(target: ItemsTarget): Int {
        ensureOpen()
        val count = EglNative.deleteItems(handle, target.toJson().toString())
        if (count < 0) throw error(EglNative.lastStatus())
        return count
    }

    fun emptyTrash(): Int {
        ensureOpen()
        val count = EglNative.emptyTrash(handle)
        if (count < 0) throw error(EglNative.lastStatus())
        return count
    }

    fun trashInfo(): TrashInfo {
        ensureOpen()
        val json = EglNative.trashInfo(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return TrashInfo.fromJson(json)
    }

    fun viewIds(viewId: Long, start: Int, count: Int): List<String> {
        ensureOpen()
        val json = EglNative.viewIds(handle, viewId, start, count)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        val arr = org.json.JSONArray(json)
        return (0 until arr.length()).map { arr.getString(it) }
    }

    fun originalPath(id: String): String? {
        ensureOpen()
        val path = EglNative.originalPath(handle, id)
        val status = EglNative.lastStatus()
        if (path.isNullOrEmpty()) {
            if (status == 0 || status == EglError.Code.NOT_FOUND.raw) return null
            throw error(status)
        }
        return path
    }

    fun originalBytes(id: String): ByteArray? {
        ensureOpen()
        return EglNative.originalBytes(handle, id)
    }

    /** Starts adding files and folders in the background; progress comes as events. */
    fun addFiles(paths: List<String>, folder: String? = null): ImportTicket {
        ensureOpen()
        require(paths.isNotEmpty()) { "paths must not be empty" }
        val request = org.json.JSONObject()
            .put("paths", org.json.JSONArray(paths))
            .put("folder", folder ?: org.json.JSONObject.NULL)
        val json = EglNative.addFiles(handle, request.toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return ImportTicket.fromJson(json)
    }

    fun importStatus(id: Long): ImportProgress? {
        ensureOpen()
        val json = EglNative.importStatus(handle, id)
        val status = EglNative.lastStatus()
        if (status != 0) throw error(status)
        if (json.isNullOrEmpty() || json == "null") return null
        return ImportProgress.fromJson(json)
    }

    fun answerImport(id: Long, answer: DuplicateAnswer) {
        ensureOpen()
        val status = EglNative.answerImport(handle, id, answer.wire)
        if (status != 0) throw error(status)
    }

    fun cancelImport(id: Long) {
        ensureOpen()
        val status = EglNative.cancelImport(handle, id)
        if (status != 0) throw error(status)
    }

    // --- Folders and tags (Android A5 ≈ D-097) --------------------------------

    fun saveFolder(change: FolderChange): List<Folder> {
        ensureOpen()
        val json = EglNative.saveFolder(handle, change.toJson().toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return Folder.listFromJson(json)
    }

    fun deleteFolder(id: String, trashItems: Boolean): List<Folder> {
        ensureOpen()
        val json = EglNative.deleteFolder(handle, id, trashItems)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return Folder.listFromJson(json)
    }

    fun renameTag(from: String, to: String) {
        ensureOpen()
        val status = EglNative.renameTag(handle, from, to)
        if (status != 0) throw error(status)
    }

    fun mergeTags(source: String, target: String) {
        ensureOpen()
        val status = EglNative.mergeTags(handle, source, target)
        if (status != 0) throw error(status)
    }

    fun deleteTag(name: String) {
        ensureOpen()
        val status = EglNative.deleteTag(handle, name)
        if (status != 0) throw error(status)
    }

    fun tagGroups(): List<TagGroup> {
        ensureOpen()
        val json = EglNative.tagGroups(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return TagGroup.listFromJson(json)
    }

    fun saveTagGroup(change: TagGroupChange): List<TagGroup> {
        ensureOpen()
        val json = EglNative.saveTagGroup(handle, change.toJson().toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return TagGroup.listFromJson(json)
    }

    fun deleteTagGroup(id: String): List<TagGroup> {
        ensureOpen()
        val json = EglNative.deleteTagGroup(handle, id)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return TagGroup.listFromJson(json)
    }

    fun saveSmartFolder(change: SmartFolderChange): List<SmartFolder> {
        ensureOpen()
        val json = EglNative.saveSmartFolder(handle, change.toJson().toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return SmartFolder.listFromJson(json)
    }

    fun deleteSmartFolder(id: String): List<SmartFolder> {
        ensureOpen()
        val json = EglNative.deleteSmartFolder(handle, id)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return SmartFolder.listFromJson(json)
    }

    fun smartRules(query: LibraryQuery): SmartRulesDraft {
        ensureOpen()
        val json = EglNative.smartRules(handle, query.toJson().toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return SmartRulesDraft.fromJson(json)
    }

    // --- Locked folders (Android A6 ≈ D-098) ---------------------------------

    /**
     * Locks a folder (or sets the first password of a lock imported without one) and returns
     * its recovery key, to be shown once. The label and its colour stay readable while locked.
     */
    fun lockFolder(
        id: String,
        password: String,
        label: String? = null,
        labelColor: String? = null,
    ): RecoveryKey {
        ensureOpen()
        val request = org.json.JSONObject()
            .put("password", password)
            .put("label", label ?: org.json.JSONObject.NULL)
            .put("labelColor", labelColor ?: org.json.JSONObject.NULL)
        val json = EglNative.lockFolder(handle, id, request.toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return RecoveryKey.fromJson(json)
    }

    fun unlockFolder(id: String, password: String) {
        ensureOpen()
        val status = EglNative.unlockFolder(handle, id, password)
        if (status != 0) throw error(status)
    }

    /** Locks an open folder again; [id] null locks every open folder. */
    fun relock(id: String? = null) {
        ensureOpen()
        val status = EglNative.relockFolder(handle, id)
        if (status != 0) throw error(status)
    }

    /** Opens a folder with its recovery key or phrase and sets a new password. */
    fun recoverFolder(id: String, recovery: String, newPassword: String) {
        ensureOpen()
        val status = EglNative.recoverFolder(handle, id, recovery, newPassword)
        if (status != 0) throw error(status)
    }

    fun changeFolderPassword(id: String, current: String, newPassword: String) {
        ensureOpen()
        val status = EglNative.changeFolderPassword(handle, id, current, newPassword)
        if (status != 0) throw error(status)
    }

    /** A new recovery key; the old one stops working. */
    fun renewRecoveryKey(id: String, password: String): RecoveryKey {
        ensureOpen()
        val json = EglNative.renewRecoveryKey(handle, id, password)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return RecoveryKey.fromJson(json)
    }

    /** Decrypts the folder and takes the lock off ("" for a lock imported without password). */
    fun removeFolderLock(id: String, password: String) {
        ensureOpen()
        val status = EglNative.removeFolderLock(handle, id, password)
        if (status != 0) throw error(status)
    }

    /** The plaintext label of a locked folder; null clears a part. */
    fun setLockLabel(id: String, label: String?, color: String?) {
        ensureOpen()
        val status = EglNative.setLockLabel(handle, id, label, color)
        if (status != 0) throw error(status)
    }

    // --- Settings / maintenance / export (Android A7 ≈ D-099) ---------------

    /** Renames the library; its folder keeps its name. */
    fun rename(to: String): LibraryInfo {
        ensureOpen()
        val json = EglNative.renameLibrary(handle, to)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return LibraryInfo.fromJson(json)
    }

    fun stats(): LibraryStats {
        ensureOpen()
        val json = EglNative.libraryStats(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return LibraryStats.fromJson(json)
    }

    /** Newest first. */
    fun backups(): List<Backup> {
        ensureOpen()
        val json = EglNative.backups(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return Backup.listFromJson(json)
    }

    fun backUpNow(): Backup {
        ensureOpen()
        val json = EglNative.backUpNow(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return Backup.fromJson(org.json.JSONObject(json))
    }

    /**
     * Puts a backup's database back (the current one is kept as a backup first). Views and
     * undo start over.
     */
    fun restoreBackup(name: String): LibraryInfo {
        ensureOpen()
        val json = EglNative.restoreBackup(handle, name)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return LibraryInfo.fromJson(json)
    }

    /** Reads only. */
    fun check(): LibraryCheck {
        ensureOpen()
        val json = EglNative.checkLibrary(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return LibraryCheck.fromJson(json)
    }

    fun repair(options: RepairOptions): LibraryCheck {
        ensureOpen()
        val json = EglNative.repairLibrary(handle, options.toJson().toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return LibraryCheck.fromJson(json)
    }

    /** Makes missing thumbnails in the background; [rebuild] makes all of them again. */
    fun generatePreviews(rebuild: Boolean): PreviewsTicket {
        ensureOpen()
        val json = EglNative.generatePreviews(handle, rebuild)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return PreviewsTicket.fromJson(json)
    }

    fun exportPreview(selection: ExportSelection, includeSubfolders: Boolean): ExportPreview {
        ensureOpen()
        val json = EglNative.exportPreview(
            handle,
            selection.toJson().toString(),
            includeSubfolders,
        )
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return ExportPreview.fromJson(json)
    }

    /** Starts an export in the background; progress comes as events. */
    fun startExport(request: ExportRequest): ExportProgress {
        ensureOpen()
        val json = EglNative.startExport(handle, request.toJson().toString())
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return ExportProgress.fromJson(json)
    }

    fun exports(): List<ExportProgress> {
        ensureOpen()
        val json = EglNative.exports(handle)
        val status = EglNative.lastStatus()
        if (json.isNullOrEmpty()) throw error(status)
        return ExportProgress.listFromJson(json)
    }

    fun cancelExport(id: Long) {
        ensureOpen()
        val status = EglNative.cancelExport(handle, id)
        if (status != 0) throw error(status)
    }

    override fun close() {
        val closing = handle
        if (closing == 0L) return
        handle = 0L
        activeLibraries.remove(closing)
        eventHandler.set(null)
        runCatching { EglNative.setEventCallback(closing, false) }
        EglNative.close(closing)
    }

    private fun ensureOpen() {
        if (handle == 0L) {
            throw EglError(EglError.Code.OTHER, "library is closed")
        }
        if (!EglNative.isAvailable) {
            throw EglError(EglError.Code.UNSUPPORTED, "native core is not linked")
        }
    }

    private fun error(status: Int): EglError {
        val message = if (handle != 0L) {
            EglNative.lastError(handle)
        } else {
            EglNative.lastErrorGlobal()
        }
        return EglError.of(if (status == 0) EglError.Code.OTHER.raw else status, message)
    }

    /** Called from JNI on the thread that produced the event. */
    internal fun dispatchEvent(json: String) {
        val handler = eventHandler.get() ?: return
        val event = LibraryEvent.fromJson(json) ?: return
        handler(event)
    }

    companion object {
        const val APP_VERSION_DEFAULT = "0.2.3-android"

        /** handle → live Library, for the JNI event trampoline. */
        private val activeLibraries = ConcurrentHashMap<Long, Library>()

        val isNativeAvailable: Boolean
            get() = EglNative.isAvailable

        /** Creates `<parent>/<name>.eglib` and returns its absolute path. */
        @JvmStatic
        fun create(parent: File, name: String, appVersion: String = APP_VERSION_DEFAULT): File {
            requireNative()
            val path = EglNative.createLibrary(parent.absolutePath, name, appVersion)
            val status = EglNative.lastStatus()
            if (path.isNullOrEmpty()) {
                throw EglError.of(
                    if (status == 0) EglError.Code.OTHER.raw else status,
                    EglNative.lastErrorGlobal(),
                )
            }
            return File(path)
        }

        /**
         * Opens an age archive (an encrypted export, `name.zip.age`) with its passphrase into
         * the existing folder [into], as `name.zip` (D-110). Blocking: call it off the main
         * thread. A wrong passphrase or a damaged file throws and leaves nothing behind.
         */
        @JvmStatic
        fun decryptArchive(source: File, into: File, passphrase: String): DecryptedArchive {
            requireNative()
            val json = EglNative.decryptArchive(source.absolutePath, into.absolutePath, passphrase)
            val status = EglNative.lastStatus()
            if (json.isNullOrEmpty()) {
                throw EglError.of(
                    if (status == 0) EglError.Code.OTHER.raw else status,
                    EglNative.lastErrorGlobal(),
                )
            }
            return DecryptedArchive.fromJson(json)
        }

        /** Opens a library folder; [syncFolder] opens it read-only (cloud / synced places). */
        @JvmStatic
        fun open(
            path: File,
            appVersion: String = APP_VERSION_DEFAULT,
            syncFolder: Boolean = false,
        ): Library {
            requireNative()
            val handle = EglNative.open(path.absolutePath, appVersion, syncFolder)
            if (handle == 0L) {
                throw EglError(
                    EglError.Code.OTHER,
                    EglNative.lastErrorGlobal().ifEmpty { "open failed" },
                )
            }
            return Library(handle)
        }

        @JvmStatic
        fun requireNative() {
            if (!EglNative.isAvailable) {
                throw EglError(
                    EglError.Code.UNSUPPORTED,
                    "native core is not linked (build libegl_library.so with cargo-ndk)",
                )
            }
        }

        @JvmStatic
        fun dispatchEventFromJni(handle: Long, json: String) {
            activeLibraries[handle]?.dispatchEvent(json)
        }
    }
}
