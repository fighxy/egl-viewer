package app.eglviewer.bridge

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.File

/**
 * Thin façade used by Compose screens.
 *
 * Mock data by default (no `.so`). When `libegl_jni.so` + `libegl_library.so` are
 * packaged, [EglNative.isAvailable] flips and open/create/thumbs talk to Rust (D-085).
 */
class LibraryBridge {
    var info: LibraryInfoDto? by mutableStateOf(null)
        private set
    var items: List<ItemCellDto> by mutableStateOf(emptyList())
        private set
    var total: Int by mutableIntStateOf(0)
        private set
    var folders: List<FolderDto> by mutableStateOf(emptyList())
        private set
    var tags: List<TagDto> by mutableStateOf(emptyList())
        private set
    var usingMockData: Boolean by mutableStateOf(true)
        private set
    var lastError: String? by mutableStateOf(null)
    var recents: List<RecentLibrary> by mutableStateOf(emptyList())
        private set

    private var mockStore: MockLibraryStore? = null
    private var ffi: NativeFfiSession? = null
    private var access: LibraryAccess? = null
    private val thumbCache = mutableMapOf<String, Bitmap>()

    val isOpen: Boolean get() = info != null
    val libraryName: String get() = info?.name ?: "Library"
    val ffiLinked: Boolean get() = EglNative.isAvailable

    fun attach(context: Context) {
        if (access == null) {
            access = LibraryAccess(context)
            refreshRecents()
        }
    }

    fun refreshRecents() {
        recents = access?.loadRecents().orEmpty()
    }

    fun openRecent(recent: RecentLibrary) {
        val uri = recent.uri?.let(Uri::parse)
        if (uri != null) {
            openUri(uri)
            return
        }
        open(path = recent.path, syncFolder = access?.isSyncFolder(recent.locationHint) == true)
    }

    fun openUri(uri: Uri) {
        lastError = null
        val path = access?.resolveFilesystemPath(uri)
        if (path == null) {
            lastError =
                "SAF URI has no filesystem path for Rust (pick a local primary-storage .eglib folder)"
            mockStore = MockLibraryStore.sample(uri.lastPathSegment ?: "Library")
            usingMockData = true
            publishMock()
            return
        }
        val hint = access?.locationHint(uri, path) ?: "Storage"
        val name = File(path).name
        access?.remember(name = name, path = path, locationHint = hint, uri = uri)
        refreshRecents()
        open(path = path, syncFolder = access?.isSyncFolder(hint) == true)
    }

    fun open(path: String, syncFolder: Boolean) {
        close()
        lastError = null
        if (EglNative.isAvailable) {
            try {
                val session = NativeFfiSession()
                session.open(path, syncFolder)
                ffi = session
                usingMockData = false
                reloadFromFfi()
                thumbCache.clear()
                return
            } catch (error: Exception) {
                lastError = error.message
            }
        }
        mockStore = MockLibraryStore.sample(path.substringAfterLast('/'))
        usingMockData = true
        publishMock()
        if (lastError == null) {
            lastError = if (ffiLinked) {
                "Could not open library — mock fallback"
            } else {
                "Rust FFI not linked — mock library"
            }
        }
    }

    fun createLibrary(named: String, parentPath: String? = null) {
        close()
        lastError = null
        if (EglNative.isAvailable) {
            try {
                val parent = parentPath ?: access?.defaultCreateParent()?.absolutePath
                    ?: throw IllegalStateException("no create parent")
                val created = NativeFfiSession.createLibrary(parent, named)
                val hint = access?.locationHint(null, created) ?: "App storage"
                access?.remember(
                    name = named,
                    path = created,
                    locationHint = hint,
                    uri = null,
                )
                refreshRecents()
                open(path = created, syncFolder = false)
                return
            } catch (error: Exception) {
                lastError = error.message
            }
        }
        mockStore = MockLibraryStore.sample(named)
        usingMockData = true
        publishMock()
    }

    fun close() {
        ffi?.close()
        ffi = null
        mockStore = null
        info = null
        items = emptyList()
        total = 0
        folders = emptyList()
        tags = emptyList()
        thumbCache.clear()
        usingMockData = true
    }

    fun itemDetail(id: String): ItemDto? {
        ffi?.let {
            return runCatching { it.item(id) }.getOrNull()
        }
        return mockStore?.item(id)
    }

    fun applyAddTag(itemId: String, tag: String) {
        ffi?.let {
            runCatching { it.applyAddTag(itemId, tag) }
            runCatching { reloadFromFfi() }
            return
        }
        mockStore?.addTag(itemId, tag)
        publishMock()
    }

    fun thumbnailBitmap(id: String): Bitmap? {
        thumbCache[id]?.let { return it }
        val bytes = ffi?.thumbnailBytes(id) ?: return null
        val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size) ?: return null
        thumbCache[id] = bitmap
        return bitmap
    }

    private fun publishMock() {
        val mock = mockStore ?: return
        info = mock.info
        items = mock.cells
        total = mock.cells.size
        folders = mock.folders
        tags = mock.tags
    }

    private fun reloadFromFfi() {
        val session = ffi ?: return
        info = session.libraryInfo()
        val window = session.viewWindow("""{"kind":"all"}""", 0, 200)
        items = window.items
        total = window.total
        folders = session.folderTree()
        tags = session.tags()
    }
}

class MockLibraryStore(
    var info: LibraryInfoDto,
    var cells: MutableList<ItemCellDto>,
    var folders: List<FolderDto>,
    var tags: MutableList<TagDto>,
    var details: MutableMap<String, ItemDto>,
) {
    fun item(id: String): ItemDto? = details[id]

    fun addTag(itemId: String, tag: String) {
        val item = details[itemId] ?: return
        val nextTags = if (tag in item.tags) item.tags else item.tags + tag
        details[itemId] = item.copy(tags = nextTags)
        val index = cells.indexOfFirst { it.id == itemId }
        if (index >= 0) {
            cells[index] = cells[index].copy(tags = nextTags)
        }
        if (tags.none { it.name == tag }) {
            tags += TagDto(name = tag, imageCount = 1)
        }
    }

    companion object {
        fun sample(named: String): MockLibraryStore {
            val clean = named.removeSuffix(".eglib").ifEmpty { "Demo" }
            val cells = (1..12).map { index ->
                ItemCellDto(
                    id = "mock%02d".format(index),
                    name = "Reference $index",
                    ext = if (index % 3 == 0) "png" else "jpg",
                    width = 1600,
                    height = if (index % 2 == 0) 900 else 1200,
                    star = index % 6,
                    hasThumbnail = false,
                    thumbVersion = 1,
                    tags = if (index % 2 == 0) listOf("Ref") else listOf("Mood"),
                )
            }.toMutableList()
            val folders = listOf(
                FolderDto(
                    id = "fld-clients",
                    name = "Clients",
                    children = listOf(
                        FolderDto(id = "fld-acme", name = "Acme", imageCount = 4),
                    ),
                    imageCount = 4,
                ),
                FolderDto(id = "fld-mood", name = "Moodboards", imageCount = 8),
            )
            val tags = mutableListOf(
                TagDto("Ref", 6),
                TagDto("Mood", 6),
            )
            val details = cells.associate { cell ->
                cell.id to ItemDto(
                    id = cell.id,
                    name = cell.name,
                    ext = cell.ext,
                    star = cell.star,
                    tags = cell.tags,
                    folders = if ("Ref" in cell.tags) listOf("fld-clients") else listOf("fld-mood"),
                    annotation = "",
                    isDeleted = false,
                )
            }.toMutableMap()
            return MockLibraryStore(
                info = LibraryInfoDto(
                    kind = "native",
                    id = "mock-library",
                    name = clean,
                    path = null,
                    readOnly = false,
                ),
                cells = cells,
                folders = folders,
                tags = tags,
                details = details,
            )
        }
    }
}
