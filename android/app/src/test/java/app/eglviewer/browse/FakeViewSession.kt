package app.eglviewer.browse

import app.eglviewer.core.ExtensionCount
import app.eglviewer.core.Item
import app.eglviewer.core.ItemCell
import app.eglviewer.core.ItemEdit
import app.eglviewer.core.ItemsTarget
import app.eglviewer.core.LibraryQuery
import app.eglviewer.core.Scope
import app.eglviewer.core.SelectionTags
import app.eglviewer.core.SortKey
import app.eglviewer.core.Tag
import app.eglviewer.core.TagMode
import app.eglviewer.core.TrashInfo
import app.eglviewer.core.UndoStatus
import app.eglviewer.core.ViewOpened
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/**
 * In-memory [ViewSession] for JVM unit tests (no `.so`). Enough for windows, sort, search,
 * filters, Select All, edits with a one-step undo stack, and trash (D-094 / D-095 / A2–A3).
 */
class FakeViewSession(
    cells: List<ItemCell>,
) : ViewSession {
    private val live = cells.toMutableList()
    private val trash = mutableListOf<ItemCell>()
    private val items = ConcurrentHashMap<String, Item>()
    private val views = ConcurrentHashMap<Long, Pair<LibraryQuery, List<ItemCell>>>()
    private val nextId = AtomicLong(1)
    private var undoStack: UndoFrame? = null
    var onViewsChanged: ((List<ViewOpened>) -> Unit)? = null

    /** Optional bytes returned from [originalBytes] for share tests. */
    var originalPayload: ByteArray? = null

    init {
        for (cell in cells) {
            items[cell.id] = itemFrom(cell)
        }
    }

    override fun openView(query: LibraryQuery): ViewOpened {
        val sorted = evaluate(query)
        val id = nextId.getAndIncrement()
        views[id] = query to sorted
        return ViewOpened(viewId = id, total = sorted.size)
    }

    override fun range(viewId: Long, start: Int, count: Int): List<ItemCell> {
        val list = views[viewId]?.second ?: return emptyList()
        if (start >= list.size) return emptyList()
        return list.subList(start, minOf(start + count, list.size))
    }

    override fun closeView(viewId: Long) {
        views.remove(viewId)
    }

    fun openViewCount(): Int = views.size

    override fun tags(): List<Tag> {
        val counts = mutableMapOf<String, Int>()
        for (cell in live) {
            for (tag in cell.tags) {
                counts[tag] = (counts[tag] ?: 0) + 1
            }
        }
        return counts.map { Tag(name = it.key, imageCount = it.value) }
    }

    override fun extensions(): List<ExtensionCount> {
        val counts = mutableMapOf<String, Int>()
        for (cell in live) {
            counts[cell.ext] = (counts[cell.ext] ?: 0) + 1
        }
        return counts.map { ExtensionCount(ext = it.key, count = it.value) }.sortedBy { it.ext }
    }

    override fun thumbnailBytes(id: String): ByteArray? = null

    override fun item(id: String): Item =
        items[id] ?: throw NoSuchElementException("item $id")

    override fun applyItemEdit(id: String, edit: ItemEdit): Item {
        val before = item(id)
        val after = applyEdit(before, edit)
        pushUndo(UndoFrame.Single(before))
        store(after)
        recompute()
        return after
    }

    override fun editItems(target: ItemsTarget, edit: ItemEdit): Int {
        val ids = resolve(target)
        when (edit) {
            ItemEdit.Trash -> {
                val moving = live.filter { it.id in ids }
                val snapshot = moving.map { item(it.id) }
                pushUndo(UndoFrame.TrashMove(snapshot))
                live.removeAll { it.id in ids }
                trash.addAll(moving)
                for (before in snapshot) {
                    items[before.id] = before.copy(isDeleted = true)
                }
            }
            ItemEdit.Restore -> {
                val moving = trash.filter { it.id in ids }
                val snapshot = moving.map { item(it.id) }
                pushUndo(UndoFrame.RestoreMove(snapshot))
                trash.removeAll { it.id in ids }
                live.addAll(moving)
                for (before in snapshot) {
                    items[before.id] = before.copy(isDeleted = false, deletedAt = null)
                }
            }
            else -> {
                val before = ids.map { item(it) }
                pushUndo(UndoFrame.Bulk(before))
                for (id in ids) {
                    store(applyEdit(item(id), edit))
                }
            }
        }
        recompute()
        return ids.size
    }

    override fun selectionTags(target: ItemsTarget): SelectionTags {
        val ids = resolve(target)
        val counts = mutableMapOf<String, Int>()
        var size = 0
        for (id in ids) {
            val it = item(id)
            size += it.size ?: 0
            for (tag in it.tags) {
                counts[tag] = (counts[tag] ?: 0) + 1
            }
        }
        return SelectionTags(
            items = ids.size,
            tags = counts.map { Tag(name = it.key, imageCount = it.value) }
                .sortedBy { it.name.lowercase() },
            size = size,
        )
    }

    override fun undo(): UndoStatus? {
        val frame = undoStack ?: return null
        undoStack = null
        when (frame) {
            is UndoFrame.Single -> {
                store(frame.before)
                recompute()
                return UndoStatus(kind = "edit", items = 1)
            }
            is UndoFrame.Bulk -> {
                for (before in frame.before) store(before)
                recompute()
                return UndoStatus(kind = "edit", items = frame.before.size)
            }
            is UndoFrame.TrashMove -> {
                val ids = frame.items.map { it.id }.toSet()
                trash.removeAll { it.id in ids }
                live.addAll(frame.items.map { cellFrom(it) })
                for (before in frame.items) {
                    items[before.id] = before.copy(isDeleted = false, deletedAt = null)
                }
                recompute()
                return UndoStatus(kind = "trash", items = frame.items.size)
            }
            is UndoFrame.RestoreMove -> {
                val ids = frame.items.map { it.id }.toSet()
                live.removeAll { it.id in ids }
                trash.addAll(frame.items.map { cellFrom(it) })
                for (before in frame.items) {
                    items[before.id] = before.copy(isDeleted = true)
                }
                recompute()
                return UndoStatus(kind = "restore", items = frame.items.size)
            }
        }
    }

    override fun undoStatus(): UndoStatus? = when (val frame = undoStack) {
        null -> null
        is UndoFrame.Single -> UndoStatus("edit", 1)
        is UndoFrame.Bulk -> UndoStatus("edit", frame.before.size)
        is UndoFrame.TrashMove -> UndoStatus("trash", frame.items.size)
        is UndoFrame.RestoreMove -> UndoStatus("restore", frame.items.size)
    }

    override fun deleteItems(target: ItemsTarget): Int {
        val ids = resolve(target)
        trash.removeAll { it.id in ids }
        live.removeAll { it.id in ids }
        for (id in ids) items.remove(id)
        undoStack = null
        recompute()
        return ids.size
    }

    override fun emptyTrash(): Int {
        val count = trash.size
        for (cell in trash) items.remove(cell.id)
        trash.clear()
        undoStack = null
        recompute()
        return count
    }

    override fun trashInfo(): TrashInfo =
        TrashInfo(items = trash.size, bytes = trash.sumOf { it.size ?: 0 })

    override fun viewIds(viewId: Long, start: Int, count: Int): List<String> =
        range(viewId, start, count).map { it.id }

    override fun originalBytes(id: String): ByteArray? {
        if (!items.containsKey(id)) return null
        return originalPayload
    }

    private fun pushUndo(frame: UndoFrame) {
        undoStack = frame
    }

    /** Updates the item record and the matching live/trash cell without moving lists. */
    private fun store(item: Item) {
        items[item.id] = item
        val cell = cellFrom(item)
        val liveIdx = live.indexOfFirst { it.id == item.id }
        if (liveIdx >= 0) live[liveIdx] = cell
        val trashIdx = trash.indexOfFirst { it.id == item.id }
        if (trashIdx >= 0) trash[trashIdx] = cell
    }

    private fun recompute() {
        val updates = mutableListOf<ViewOpened>()
        for ((id, pair) in views) {
            val next = evaluate(pair.first)
            views[id] = pair.first to next
            updates += ViewOpened(viewId = id, total = next.size)
        }
        onViewsChanged?.invoke(updates)
    }

    private fun resolve(target: ItemsTarget): Set<String> {
        if (!target.all) return target.ids.toSet()
        val viewId = target.viewId ?: return emptySet()
        val list = views[viewId]?.second ?: return emptySet()
        val excluded = target.ids.toSet()
        return list.map { it.id }.filterNot { it in excluded }.toSet()
    }

    private fun evaluate(query: LibraryQuery): List<ItemCell> {
        var list = when (val scope = query.scope) {
            null, Scope.All -> live.toList()
            Scope.Unfiled -> live.toList()
            Scope.Untagged -> live.filter { it.tags.isEmpty() }
            Scope.Trash -> trash.toList()
            is Scope.FolderScope, is Scope.SmartScope -> live.toList()
        }
        val tags = query.tags.orEmpty()
        if (tags.isNotEmpty()) {
            list = when (query.tagMode ?: TagMode.Any) {
                TagMode.Any -> list.filter { cell -> tags.any { it in cell.tags } }
                TagMode.All -> list.filter { cell -> tags.all { it in cell.tags } }
                TagMode.None -> list.filter { cell -> tags.none { it in cell.tags } }
            }
        }
        query.ext?.let { ext -> list = list.filter { it.ext.equals(ext, ignoreCase = true) } }
        query.rating?.let { rating ->
            list = if (rating == 0) list.filter { it.star == 0 } else list.filter { it.star == rating }
        }
        query.shape?.let { shape -> list = list.filter { shapeOf(it) == shape } }
        query.text?.let { text ->
            val needle = text.lowercase()
            list = list.filter { it.name.lowercase().contains(needle) }
        }
        return sort(list, query.sort ?: SortKey.Added, query.ascending == true)
    }

    private sealed class UndoFrame {
        data class Single(val before: Item) : UndoFrame()
        data class Bulk(val before: List<Item>) : UndoFrame()
        data class TrashMove(val items: List<Item>) : UndoFrame()
        data class RestoreMove(val items: List<Item>) : UndoFrame()
    }

    companion object {
        const val TAG = "Red"
        const val TAGGED = 4

        fun sample(images: Int, tagged: Int = TAGGED, tag: String = TAG): FakeViewSession {
            val cells = (0 until images).map { i ->
                val w = when (i % 3) {
                    0 -> 100
                    1 -> 200
                    else -> 80
                }
                val h = when (i % 3) {
                    0 -> 100
                    1 -> 100
                    else -> 160
                }
                ItemCell(
                    id = "id-$i",
                    name = "sample-$i",
                    ext = "png",
                    width = w,
                    height = h,
                    size = 1000 + i,
                    star = 0,
                    palette0 = listOf(i % 256, 40, 80),
                    hasThumbnail = false,
                    thumbVersion = 1,
                    tags = if (i < tagged) listOf(tag) else emptyList(),
                )
            }
            return FakeViewSession(cells)
        }

        fun itemFrom(cell: ItemCell): Item = Item(
            id = cell.id,
            name = cell.name,
            ext = cell.ext,
            width = cell.width,
            height = cell.height,
            size = cell.size,
            star = cell.star,
            tags = cell.tags,
            folders = emptyList(),
            annotation = "",
            url = "",
            thumbVersion = cell.thumbVersion,
        )

        fun cellFrom(item: Item): ItemCell = ItemCell(
            id = item.id,
            name = item.name,
            ext = item.ext,
            width = item.width,
            height = item.height,
            size = item.size,
            star = item.star,
            hasThumbnail = !item.noThumbnail,
            thumbVersion = item.thumbVersion,
            tags = item.tags,
        )

        fun applyEdit(item: Item, edit: ItemEdit): Item = when (edit) {
            is ItemEdit.AddTags -> item.copy(tags = (item.tags + edit.value).distinct())
            is ItemEdit.RemoveTags -> item.copy(tags = item.tags.filterNot { it in edit.value })
            is ItemEdit.AddToFolders -> item.copy(folders = (item.folders + edit.value).distinct())
            is ItemEdit.RemoveFromFolders ->
                item.copy(folders = item.folders.filterNot { it in edit.value })
            is ItemEdit.SetFolders -> item.copy(folders = edit.value)
            is ItemEdit.SetStar -> item.copy(star = edit.value)
            is ItemEdit.SetAnnotation -> item.copy(annotation = edit.value)
            is ItemEdit.SetUrl -> item.copy(url = edit.value)
            is ItemEdit.Rename -> item.copy(name = edit.value)
            ItemEdit.Trash -> item.copy(isDeleted = true)
            ItemEdit.Restore -> item.copy(isDeleted = false, deletedAt = null)
        }

        private fun shapeOf(cell: ItemCell): String? {
            val w = cell.width ?: return null
            val h = cell.height ?: return null
            if (w == 0 || h == 0) return null
            val ratio = w.toDouble() / h.toDouble()
            return when {
                ratio in 0.95..1.05 -> "square"
                ratio > 2.0 -> "panoramic-landscape"
                ratio > 1.05 -> "landscape"
                ratio < 0.5 -> "panoramic-portrait"
                else -> "portrait"
            }
        }

        private fun sort(cells: List<ItemCell>, key: SortKey, ascending: Boolean): List<ItemCell> {
            fun indexName(c: ItemCell): Int =
                c.name.removePrefix("sample-").toIntOrNull() ?: Int.MAX_VALUE

            val comparator: Comparator<ItemCell> = when (key) {
                SortKey.Name, SortKey.Added -> compareBy(::indexName)
                SortKey.Size -> compareBy { it.size ?: 0 }
                SortKey.Ext -> compareBy { it.ext.lowercase() }
                SortKey.Star -> compareBy { it.star }
                SortKey.Resolution -> compareBy { (it.width ?: 0) * (it.height ?: 0) }
                SortKey.Btime, SortKey.Mtime -> compareBy(::indexName)
                SortKey.Random -> Comparator { _, _ -> 0 }
            }
            val ordered = if (key == SortKey.Random) {
                cells.shuffled()
            } else {
                cells.sortedWith(comparator)
            }
            return if (ascending || key == SortKey.Random) ordered else ordered.reversed()
        }
    }
}
