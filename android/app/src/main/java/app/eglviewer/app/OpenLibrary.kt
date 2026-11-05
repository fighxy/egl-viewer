package app.eglviewer.app

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import app.eglviewer.browse.ActionToast
import app.eglviewer.browse.BrowseFilters
import app.eglviewer.browse.BrowseHost
import app.eglviewer.browse.BrowseModel
import app.eglviewer.browse.LiveViewSession
import app.eglviewer.browse.OriginalCache
import app.eglviewer.browse.PickableFolder
import app.eglviewer.browse.Place
import app.eglviewer.browse.SavedView
import app.eglviewer.browse.SavedViews
import app.eglviewer.browse.ThumbnailCache
import app.eglviewer.browse.ViewSession
import app.eglviewer.core.Folder
import app.eglviewer.core.Library
import app.eglviewer.core.LibraryCounts
import app.eglviewer.core.LibraryEvent
import app.eglviewer.core.LibraryInfo
import app.eglviewer.core.LibraryKind
import app.eglviewer.core.SmartFolder
import app.eglviewer.core.ViewOpened
import java.io.File
import java.lang.ref.WeakReference
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * One open library: core handle, places/counts/folders, saved view, toast / undo, and
 * browse-model registration (A1–A3, D-101 / D-102 / D-103).
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

    private var hidingToast: Job? = null

    private val browsers = CopyOnWriteArrayList<WeakReference<BrowseModel>>()

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

    fun attachEvents() {
        library.setEventHandler { event ->
            when (event) {
                is LibraryEvent.ViewsInvalidated -> {
                    notifyViewsInvalidated(event.views)
                    runCatching { refresh() }
                }
                is LibraryEvent.ImportProgress -> runCatching { refresh() }
                else -> Unit
            }
        }
    }

    fun refresh() {
        info = library.info()
        counts = library.counts()
        folders = runCatching { library.folders() }.getOrDefault(emptyList())
        smartFolders = runCatching { library.smartFolders() }.getOrDefault(emptyList())
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
