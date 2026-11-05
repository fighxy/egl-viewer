package app.eglviewer.browse

import app.eglviewer.core.ExtensionCount
import app.eglviewer.core.ItemCell
import app.eglviewer.core.ItemsTarget
import app.eglviewer.core.LibraryQuery
import app.eglviewer.core.Scope
import app.eglviewer.core.SortKey
import app.eglviewer.core.Tag
import app.eglviewer.core.TagMode
import app.eglviewer.core.ViewOpened
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/**
 * In-memory [ViewSession] for JVM unit tests (no `.so`). Enough for windows, sort, search,
 * filters, Select All and a trash refresh (D-094 / Android A2).
 */
class FakeViewSession(
    cells: List<ItemCell>,
) : ViewSession {
    private val live = cells.toMutableList()
    private val trash = mutableListOf<ItemCell>()
    private val views = ConcurrentHashMap<Long, Pair<LibraryQuery, List<ItemCell>>>()
    private val nextId = AtomicLong(1)
    var onViewsChanged: ((List<ViewOpened>) -> Unit)? = null

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

    override fun editItems(target: ItemsTarget, editType: String): Int {
        val ids = resolve(target)
        when (editType) {
            "trash" -> {
                val moving = live.filter { it.id in ids }
                live.removeAll { it.id in ids }
                trash.addAll(moving)
            }
            "restore" -> {
                val moving = trash.filter { it.id in ids }
                trash.removeAll { it.id in ids }
                live.addAll(moving)
            }
        }
        recompute()
        return ids.size
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
                SortKey.Random -> Comparator { _, _ -> 0 } // keep insertion; shuffle separately
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
