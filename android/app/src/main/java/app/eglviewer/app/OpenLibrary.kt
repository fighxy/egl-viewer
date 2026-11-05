package app.eglviewer.app

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import app.eglviewer.adding.PickedFile
import app.eglviewer.adding.importSummary
import app.eglviewer.browse.ActionToast
import app.eglviewer.browse.BrowseFilters
import app.eglviewer.browse.BrowseHost
import app.eglviewer.browse.BrowseModel
import app.eglviewer.browse.LiveViewSession
import app.eglviewer.browse.OriginalCache
import app.eglviewer.browse.PickableFolder
import app.eglviewer.browse.PickableSmart
import app.eglviewer.browse.Place
import app.eglviewer.browse.SavedView
import app.eglviewer.browse.SavedViews
import app.eglviewer.browse.ThumbnailCache
import app.eglviewer.browse.ViewSession
import app.eglviewer.R
import app.eglviewer.core.DuplicateAnswer
import app.eglviewer.core.EglError
import app.eglviewer.core.ExportProgress
import app.eglviewer.core.ExportRequest
import app.eglviewer.core.Folder
import app.eglviewer.core.FolderChange
import app.eglviewer.core.ImportProgress
import app.eglviewer.core.ImportState
import app.eglviewer.core.Library
import app.eglviewer.core.LibraryCounts
import app.eglviewer.core.LibraryEvent
import app.eglviewer.core.LibraryInfo
import app.eglviewer.core.LibraryKind
import app.eglviewer.core.LibraryQuery
import app.eglviewer.core.SmartFolder
import app.eglviewer.core.SmartFolderChange
import app.eglviewer.core.SmartRulesDraft
import app.eglviewer.core.TagGroup
import app.eglviewer.core.TagGroupChange
import app.eglviewer.core.ViewOpened
import java.io.File
import java.lang.ref.WeakReference
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * One open library: core handle, places/counts/folders, saved view, toast / undo,
 * browse-model registration, adding, organizing, locks, and settings
 * (A1–A7, D-101…D-107).
 */
class OpenLibrary(
    val library: Library,
    val path: File,
    private val savedViews: SavedViews,
    sessionOverride: ViewSession? = null,
    private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
) : BrowseHost {
    override val libraryPath: String = path.absolutePath
    override val session: ViewSession = sessionOverride ?: LiveViewSession(library)
    override val thumbnails: ThumbnailCache = ThumbnailCache(session)
    val originals: OriginalCache = OriginalCache(session)

    var info: LibraryInfo by mutableStateOf(
        LibraryInfo(
            kind = LibraryKind.NATIVE,
            id = null,
            name = path.name.removeSuffix(".eglib"),
            path = path.absolutePath,
            readOnly = false,
        ),
    )
        private set

    var counts: LibraryCounts? by mutableStateOf(null)
        private set

    var folders: List<Folder> by mutableStateOf(emptyList())
        private set

    var smartFolders: List<SmartFolder> by mutableStateOf(emptyList())
        private set

    var tagGroups: List<TagGroup> by mutableStateOf(emptyList())
        private set

    /** Kept for A1 callers that only need a folders count. */
    val folderCount: Int
        get() = folders.size

    override var savedView: SavedView by mutableStateOf(savedViews.load(forLibrary = libraryPath))
        private set

    override val readOnly: Boolean get() = info.readOnly

    var lastError: String? by mutableStateOf(null)
        private set

    override var toast: ActionToast? by mutableStateOf(null)
        private set

    /** Running imports by id (banner). */
    var imports: Map<Long, ImportProgress> by mutableStateOf(emptyMap())
        private set

    /** The duplicate an import waits on, shown as a question. */
    var duplicateQuestion: ImportProgress? by mutableStateOf(null)
        private set

    /** Running and finished exports by id (Settings export screen). */
    var exports: Map<Long, ExportProgress> by mutableStateOf(emptyMap())
        private set

    private var hidingToast: Job? = null

    private val browsers = CopyOnWriteArrayList<WeakReference<BrowseModel>>()
    private val afterImport = ConcurrentHashMap<Long, () -> Unit>()
    private val answered = ConcurrentHashMap.newKeySet<Long>()
    private var appContext: Context? = null

    override val pickableFolders: List<PickableFolder>
        get() {
            val list = mutableListOf<PickableFolder>()
            fun walk(nodes: List<Folder>, depth: Int) {
                for (folder in nodes) {
                    if (!folder.isLocked || folder.isUnlocked) {
                        list += PickableFolder(folder = folder, depth = depth)
                        walk(folder.children, depth + 1)
                    }
                }
            }
            walk(folders, 0)
            return list
        }

    fun attachEvents(context: Context? = null) {
        if (context != null) appContext = context.applicationContext
        // Events arrive on the core's threads (imports, exports) or on the thread of the call
        // that wrote (D-109): the browse models change on the main thread.
        library.setEventHandler { event ->
            when (event) {
                is LibraryEvent.ViewsInvalidated -> {
                    scope.launch(Dispatchers.Main.immediate) {
                        notifyViewsInvalidated(event.views)
                        withContext(Dispatchers.IO) { runCatching { refresh() } }
                    }
                }
                is LibraryEvent.ImportProgress -> {
                    scope.launch(Dispatchers.Main.immediate) {
                        progressed(event.progress)
                    }
                    if (event.progress.isFinished ||
                        event.progress.state == ImportState.ADDING ||
                        event.progress.state == ImportState.PREVIEWS
                    ) {
                        runCatching { refresh() }
                    }
                }
                is LibraryEvent.ExportProgress -> {
                    scope.launch(Dispatchers.Main.immediate) {
                        exports = exports + (event.progress.id to event.progress)
                    }
                }
                else -> Unit
            }
        }
    }

    // --- Settings / maintenance / export (A7 ≈ D-099) -----------------------

    /** Renames the library: its folder keeps its name, the library's own name changes. */
    suspend fun rename(to: String): Boolean {
        val name = to.trim()
        if (name.isEmpty() || readOnly) return false
        val next = withContext(Dispatchers.IO) {
            runCatching { library.rename(to = name) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        } ?: return false
        info = next
        return true
    }

    /**
     * A restored backup is another database: views and undo start over, so every open
     * screen asks for its view again and the tree is read anew.
     */
    suspend fun restoreBackup(name: String): Boolean {
        val next = withContext(Dispatchers.IO) {
            runCatching { library.restoreBackup(name) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        } ?: return false
        info = next
        browsers.mapNotNull { it.get() }.forEach { browser ->
            scope.launch { browser.reload() }
        }
        thumbnails.removeAll()
        originals.removeAll()
        withContext(Dispatchers.IO) { runCatching { refresh() } }
        val ctx = appContext
        show(ctx?.getString(R.string.toast_restored) ?: "Backup restored", undoable = false)
        return true
    }

    fun forgetExport(id: Long) {
        exports = exports - id
    }

    /** Starts an export; its progress arrives as events. */
    suspend fun startExport(request: ExportRequest): ExportProgress? {
        val started = withContext(Dispatchers.IO) {
            runCatching { library.startExport(request) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        } ?: return null
        if (exports[started.id] == null) {
            exports = exports + (started.id to started)
        }
        return started
    }

    fun cancelExport(id: Long) {
        scope.launch {
            withContext(Dispatchers.IO) {
                runCatching { library.cancelExport(id) }.onFailure {
                    report(it.message ?: it.toString())
                }
            }
        }
    }

    /**
     * Adds filesystem paths into [folder] (or none). [cleanup] runs when the import ends,
     * or at once when it cannot start.
     */
    fun addPaths(paths: List<String>, into: String?, cleanup: () -> Unit = {}) {
        if (paths.isEmpty() || readOnly) {
            cleanup()
            return
        }
        scope.launch {
            val ticket = try {
                withContext(Dispatchers.IO) { library.addFiles(paths, into) }
            } catch (error: Exception) {
                report(error.message ?: error.toString())
                cleanup()
                return@launch
            }
            afterImport[ticket.id] = cleanup
            val progress = withContext(Dispatchers.IO) {
                runCatching { library.importStatus(ticket.id) }.getOrNull()
            }
            if (progress != null && progress.isFinished) {
                finished(progress)
            } else if (progress != null) {
                progressed(progress)
            }
        }
    }

    /** Adds temporary copies from gallery / paste / drops; they are removed afterwards. */
    fun addPicked(files: List<PickedFile>, into: String?) {
        addPaths(files.map { it.file.absolutePath }, into) {
            files.forEach { it.remove() }
        }
    }

    fun answer(answer: DuplicateAnswer) {
        val question = duplicateQuestion ?: return
        duplicateQuestion = null
        answered.add(question.id)
        scope.launch {
            withContext(Dispatchers.IO) {
                runCatching { library.answerImport(question.id, answer) }
                    .onFailure { report(it.message ?: it.toString()) }
            }
        }
    }

    fun showUnreadable(count: Int) {
        val ctx = appContext
        val message = if (ctx != null) {
            ctx.resources.getQuantityString(R.plurals.add_unreadable, count, count)
        } else {
            "$count item(s) could not be read from Gallery"
        }
        show(message, undoable = false)
    }

    private fun progressed(progress: ImportProgress) {
        if (progress.isFinished) {
            finished(progress)
            return
        }
        imports = imports + (progress.id to progress)
        if (progress.state == ImportState.WAITING_FOR_DECISION &&
            progress.duplicate != null &&
            progress.id !in answered &&
            duplicateQuestion?.id != progress.id
        ) {
            duplicateQuestion = progress
        }
    }

    private fun finished(progress: ImportProgress) {
        if (duplicateQuestion?.id == progress.id) {
            duplicateQuestion = null
        }
        answered.remove(progress.id)
        imports = imports - progress.id
        val cleanup = afterImport.remove(progress.id) ?: return
        cleanup()
        show(summaryOf(progress), undoable = false)
        runCatching { refresh() }
    }

    private fun summaryOf(progress: ImportProgress): String {
        val ctx = appContext
        return if (ctx != null) {
            importSummary(
                progress = progress,
                addedLabel = { n -> ctx.resources.getQuantityString(R.plurals.add_done, n, n) },
                duplicatesLabel = { n -> ctx.resources.getQuantityString(R.plurals.add_duplicates, n, n) },
                failedLabel = { n -> ctx.resources.getQuantityString(R.plurals.add_failed, n, n) },
                cancelledLabel = ctx.getString(R.string.add_cancelled),
            )
        } else {
            importSummary(
                progress = progress,
                addedLabel = { n -> "Added $n items" },
                duplicatesLabel = { n -> "$n duplicates skipped" },
                failedLabel = { n -> "$n failed" },
                cancelledLabel = "stopped",
            )
        }
    }

    fun refresh() {
        info = library.info()
        counts = library.counts()
        folders = runCatching { library.folders() }.getOrDefault(emptyList())
        smartFolders = runCatching { library.smartFolders() }.getOrDefault(emptyList())
        tagGroups = runCatching { library.tagGroups() }.getOrDefault(emptyList())
    }

    /** One edit of the folder tree; false when the core refused it. */
    suspend fun saveFolder(change: FolderChange): Boolean {
        val tree = withContext(Dispatchers.IO) {
            runCatching { library.saveFolder(change) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        } ?: return false
        folders = tree
        return true
    }

    suspend fun deleteFolder(id: String, trashItems: Boolean) {
        val tree = withContext(Dispatchers.IO) {
            runCatching { library.deleteFolder(id, trashItems) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        } ?: return
        folders = tree
        forgetIfSaved(Place.Folder(id = id, name = ""))
        withContext(Dispatchers.IO) { runCatching { refresh() } }
        val ctx = appContext
        show(ctx?.getString(R.string.toast_folder_deleted) ?: "Folder deleted", undoable = false)
    }

    /** The new parent's position for a move: after its last child. */
    fun endIndex(under: String?): Int {
        if (under == null) return folders.size
        return findFolder(under, folders)?.children?.size ?: 0
    }

    /** The folder and its descendants: where it cannot move. */
    fun subtree(of: String): Set<String> {
        val folder = findFolder(of, folders) ?: return setOf(of)
        val ids = mutableSetOf<String>()
        fun walk(node: Folder) {
            ids += node.id
            node.children.forEach(::walk)
        }
        walk(folder)
        return ids
    }

    fun parentOf(id: String): String? {
        fun search(nodes: List<Folder>, parent: String?): String? {
            for (folder in nodes) {
                if (folder.id == id) return parent
                search(folder.children, folder.id)?.let { return it }
            }
            return null
        }
        return search(folders, null)
    }

    suspend fun renameTag(from: String, to: String): Boolean {
        val trimmed = to.trim()
        if (trimmed.isEmpty() || trimmed == from) return false
        val ok = withContext(Dispatchers.IO) {
            runCatching { library.renameTag(from, trimmed) }.onFailure {
                report(it.message ?: it.toString())
            }.isSuccess
        }
        if (!ok) return false
        withContext(Dispatchers.IO) { runCatching { refresh() } }
        return true
    }

    suspend fun mergeTag(source: String, into: String): Boolean {
        if (source == into) return false
        val ok = withContext(Dispatchers.IO) {
            runCatching { library.mergeTags(source, into) }.onFailure {
                report(it.message ?: it.toString())
            }.isSuccess
        }
        if (!ok) return false
        withContext(Dispatchers.IO) { runCatching { refresh() } }
        return true
    }

    suspend fun deleteTag(name: String) {
        val ok = withContext(Dispatchers.IO) {
            runCatching { library.deleteTag(name) }.onFailure {
                report(it.message ?: it.toString())
            }.isSuccess
        }
        if (!ok) return
        withContext(Dispatchers.IO) { runCatching { refresh() } }
        val ctx = appContext
        show(ctx?.getString(R.string.toast_tag_deleted) ?: "Tag removed from every item", undoable = false)
    }

    suspend fun saveTagGroup(change: TagGroupChange): Boolean {
        val groups = withContext(Dispatchers.IO) {
            runCatching { library.saveTagGroup(change) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        } ?: return false
        tagGroups = groups
        return true
    }

    suspend fun deleteTagGroup(id: String) {
        val groups = withContext(Dispatchers.IO) {
            runCatching { library.deleteTagGroup(id) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        } ?: return
        tagGroups = groups
    }

    suspend fun saveSmartFolder(change: SmartFolderChange): Boolean {
        val tree = withContext(Dispatchers.IO) {
            runCatching { library.saveSmartFolder(change) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        } ?: return false
        smartFolders = tree
        return true
    }

    suspend fun deleteSmartFolder(id: String) {
        val tree = withContext(Dispatchers.IO) {
            runCatching { library.deleteSmartFolder(id) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        } ?: return
        smartFolders = tree
        forgetIfSaved(Place.Smart(id = id, name = ""))
        val ctx = appContext
        show(ctx?.getString(R.string.toast_smart_folder_deleted) ?: "Smart folder deleted", undoable = false)
    }

    /** The rules of a view's query, for "Save as Smart Folder". */
    suspend fun smartDraft(forQuery: LibraryQuery): SmartRulesDraft? =
        withContext(Dispatchers.IO) {
            runCatching { library.smartRules(forQuery) }.onFailure {
                report(it.message ?: it.toString())
            }.getOrNull()
        }

    val hasUnlockedFolders: Boolean
        get() {
            fun any(nodes: List<Folder>): Boolean =
                nodes.any { (it.isLocked && it.isUnlocked) || any(it.children) }
            return any(folders)
        }

    /**
     * Runs a lock operation (Argon2id: slow, off the main thread) and reads the tree again,
     * since a lock changes what the views show. A wrong password is reported to the caller,
     * not the alert.
     */
    suspend fun <T> lockWork(work: (Library) -> T): Result<T> {
        val result = withContext(Dispatchers.IO) {
            runCatching { work(library) }
        }
        withContext(Dispatchers.IO) { runCatching { refresh() } }
        return result.fold(
            onSuccess = { Result.success(it) },
            onFailure = { error ->
                if (error is EglError && error.code == EglError.Code.WRONG_PASSWORD) {
                    Result.failure(error)
                } else {
                    report(error.message ?: error.toString())
                    Result.failure(
                        error as? EglError
                            ?: EglError(EglError.Code.OTHER, error.message ?: error.toString()),
                    )
                }
            },
        )
    }

    /** Locks every open folder again: the Lock All button, and when the app leaves the screen. */
    suspend fun relockAll() {
        if (!hasUnlockedFolders) return
        lockWork { it.relock() }
        originals.removeAll()
        thumbnails.removeAll()
    }

    /** Smart folders in tree order with their depth, for the parent picker. */
    val smartFolderList: List<PickableSmart>
        get() {
            val list = mutableListOf<PickableSmart>()
            fun walk(nodes: List<SmartFolder>, depth: Int) {
                for (folder in nodes) {
                    list += PickableSmart(folder = folder, depth = depth)
                    walk(folder.children, depth + 1)
                }
            }
            walk(smartFolders, 0)
            return list
        }

    /** A deleted folder is not the place the library opens in next time. */
    private fun forgetIfSaved(place: Place) {
        val saved = savedView.place ?: return
        if (saved.key == place.key) forgetPlace()
    }

    override fun remember(view: SavedView) {
        if (view == savedView) return
        savedView = view
        savedViews.save(view, forLibrary = libraryPath)
    }

    override fun forgetPlace() {
        remember(
            savedView.copy(
                place = null,
                filters = BrowseFilters(),
                search = "",
            ),
        )
    }

    override fun report(message: String) {
        lastError = message
    }

    override fun register(browser: BrowseModel) {
        browsers.removeAll { it.get() == null || it.get() === browser }
        browsers.add(WeakReference(browser))
    }

    override fun unregister(browser: BrowseModel) {
        browsers.removeAll { it.get() == null || it.get() === browser }
    }

    override suspend fun <T> perform(block: (ViewSession) -> T): T? =
        try {
            withContext(Dispatchers.IO) { block(session) }
        } catch (error: Exception) {
            report(error.message ?: error.toString())
            null
        }

    override fun show(message: String, undoable: Boolean) {
        val next = ActionToast(message = message, undoable = undoable)
        toast = next
        hidingToast?.cancel()
        hidingToast = scope.launch {
            delay(5_000)
            if (toast == next) toast = null
        }
    }

    fun dismissToast() {
        hidingToast?.cancel()
        toast = null
    }

    override suspend fun undo() {
        dismissToast()
        val status = perform { it.undo() }
        if (status != null) {
            show("Undone", undoable = false)
        }
    }

    fun notifyViewsInvalidated(views: List<ViewOpened>) {
        browsers.mapNotNull { it.get() }.forEach { it.invalidated(views) }
    }

    fun exists(place: Place): Boolean = when (place) {
        Place.All, Place.Uncategorized, Place.Untagged, Place.Trash, is Place.Tag -> true
        is Place.Folder -> findFolder(place.id, folders)?.let { !it.isLocked || it.isUnlocked } == true
        is Place.Smart -> findSmart(place.id, smartFolders) != null
    }

    fun close() {
        browsers.clear()
        thumbnails.removeAll()
        originals.removeAll()
        dismissToast()
        duplicateQuestion = null
        imports = emptyMap()
        exports = emptyMap()
        val cleanups = afterImport.values.toList()
        afterImport.clear()
        cleanups.forEach { runCatching { it() } }
        runCatching { library.setEventHandler(null) }
        library.close()
    }

    companion object {
        fun savedViews(context: Context): SavedViews =
            SavedViews.sharedPreferences(
                context.getSharedPreferences(SavedViews.PREFS_NAME, Context.MODE_PRIVATE),
            )

        fun findFolder(id: String, folders: List<Folder>): Folder? {
            for (folder in folders) {
                if (folder.id == id) return folder
                findFolder(id, folder.children)?.let { return it }
            }
            return null
        }

        fun findSmart(id: String, folders: List<SmartFolder>): SmartFolder? {
            for (folder in folders) {
                if (folder.id == id) return folder
                findSmart(id, folder.children)?.let { return it }
            }
            return null
        }
    }
}
