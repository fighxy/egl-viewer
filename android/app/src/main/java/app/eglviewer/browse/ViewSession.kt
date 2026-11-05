package app.eglviewer.browse

import app.eglviewer.core.ExtensionCount
import app.eglviewer.core.ItemCell
import app.eglviewer.core.ItemsTarget
import app.eglviewer.core.Library
import app.eglviewer.core.LibraryQuery
import app.eglviewer.core.Tag
import app.eglviewer.core.ViewOpened

/**
 * What [BrowseModel] needs from the open library's core. Live [Library] or an in-memory
 * fake for JVM tests. Item lists never cross as a full id list — only windows of cells.
 */
interface ViewSession {
    fun openView(query: LibraryQuery): ViewOpened
    fun range(viewId: Long, start: Int, count: Int): List<ItemCell>
    fun closeView(viewId: Long)
    fun tags(): List<Tag>
    fun extensions(): List<ExtensionCount>
    fun thumbnailBytes(id: String): ByteArray?
    fun editItems(target: ItemsTarget, editType: String): Int =
        throw UnsupportedOperationException("editItems")
}

/** [ViewSession] over the real [Library] / `egl_open_view` · `egl_view_range`. */
class LiveViewSession(private val library: Library) : ViewSession {
    override fun openView(query: LibraryQuery): ViewOpened = library.openView(query)
    override fun range(viewId: Long, start: Int, count: Int): List<ItemCell> =
        library.range(viewId, start, count)
    override fun closeView(viewId: Long) = library.closeView(viewId)
    override fun tags(): List<Tag> = library.tags()
    override fun extensions(): List<ExtensionCount> = library.extensions()
    override fun thumbnailBytes(id: String): ByteArray? = library.thumbnailBytes(id)
    override fun editItems(target: ItemsTarget, editType: String): Int =
        library.editItems(target, editType)
}

/**
 * Host for one open library: saved view, browser registration, error reporting.
 */
interface BrowseHost {
    val libraryPath: String
    val readOnly: Boolean
    val savedView: SavedView
    val session: ViewSession
    val thumbnails: ThumbnailCache

    fun remember(view: SavedView)
    fun forgetPlace()
    fun report(message: String)
    fun register(browser: BrowseModel)
    fun unregister(browser: BrowseModel)
}
