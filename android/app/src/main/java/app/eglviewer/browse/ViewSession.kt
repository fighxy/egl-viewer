package app.eglviewer.browse

import app.eglviewer.core.ExtensionCount
import app.eglviewer.core.Item
import app.eglviewer.core.ItemCell
import app.eglviewer.core.ItemEdit
import app.eglviewer.core.ItemsTarget
import app.eglviewer.core.Library
import app.eglviewer.core.LibraryQuery
import app.eglviewer.core.SelectionTags
import app.eglviewer.core.Tag
import app.eglviewer.core.TrashInfo
import app.eglviewer.core.UndoStatus
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

    fun item(id: String): Item = throw UnsupportedOperationException("item")
    fun applyItemEdit(id: String, edit: ItemEdit): Item =
        throw UnsupportedOperationException("applyItemEdit")
    fun editItems(target: ItemsTarget, edit: ItemEdit): Int =
        throw UnsupportedOperationException("editItems")
    fun selectionTags(target: ItemsTarget): SelectionTags =
        throw UnsupportedOperationException("selectionTags")
    fun undo(): UndoStatus? = throw UnsupportedOperationException("undo")
    fun undoStatus(): UndoStatus? = null
    fun deleteItems(target: ItemsTarget): Int =
        throw UnsupportedOperationException("deleteItems")
    fun emptyTrash(): Int = throw UnsupportedOperationException("emptyTrash")
    fun trashInfo(): TrashInfo = TrashInfo(items = 0, bytes = 0)
    fun viewIds(viewId: Long, start: Int, count: Int): List<String> =
        range(viewId, start, count).map { it.id }
    fun originalPath(id: String): String? = null
    fun originalBytes(id: String): ByteArray? = null
}

/** [ViewSession] over the real [Library] / `egl_*` C ABI. */
class LiveViewSession(private val library: Library) : ViewSession {
    override fun openView(query: LibraryQuery): ViewOpened = library.openView(query)
    override fun range(viewId: Long, start: Int, count: Int): List<ItemCell> =
        library.range(viewId, start, count)
    override fun closeView(viewId: Long) = library.closeView(viewId)
    override fun tags(): List<Tag> = library.tags()
    override fun extensions(): List<ExtensionCount> = library.extensions()
    override fun thumbnailBytes(id: String): ByteArray? = library.thumbnailBytes(id)

    override fun item(id: String): Item = library.item(id)
    override fun applyItemEdit(id: String, edit: ItemEdit): Item = library.applyItemEdit(id, edit)
    override fun editItems(target: ItemsTarget, edit: ItemEdit): Int = library.editItems(target, edit)
    override fun selectionTags(target: ItemsTarget): SelectionTags = library.selectionTags(target)
    override fun undo(): UndoStatus? = library.undo()
    override fun undoStatus(): UndoStatus? = library.undoStatus()
    override fun deleteItems(target: ItemsTarget): Int = library.deleteItems(target)
    override fun emptyTrash(): Int = library.emptyTrash()
    override fun trashInfo(): TrashInfo = library.trashInfo()
    override fun viewIds(viewId: Long, start: Int, count: Int): List<String> =
        library.viewIds(viewId, start, count)
    override fun originalPath(id: String): String? = library.originalPath(id)
    override fun originalBytes(id: String): ByteArray? = library.originalBytes(id)
}

/**
 * Host for one open library: saved view, browser registration, error reporting, toast / undo.
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

    /** Runs [block] off the UI thread; reports errors and returns null on failure. */
    suspend fun <T> perform(block: (ViewSession) -> T): T? =
        try {
            block(session)
        } catch (error: Exception) {
            report(error.message ?: error.toString())
            null
        }

    fun show(message: String, undoable: Boolean) = Unit
    suspend fun undo() = Unit
    val toast: ActionToast? get() = null
    val pickableFolders: List<PickableFolder> get() = emptyList()
}

data class ActionToast(val message: String, val undoable: Boolean)

data class PickableFolder(val folder: app.eglviewer.core.Folder, val depth: Int)

data class PickableSmart(val folder: app.eglviewer.core.SmartFolder, val depth: Int)
