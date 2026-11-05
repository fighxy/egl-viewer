package app.eglviewer.browse

import app.eglviewer.core.ExtensionCount
import app.eglviewer.core.ItemCell
import app.eglviewer.core.ItemsTarget
import app.eglviewer.core.LibraryQuery
import app.eglviewer.core.Tag
import app.eglviewer.core.TagMode
import app.eglviewer.core.ViewOpened
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger

/**
 * One place on screen: a core view for the place, sort/filters/search, cells in windows, and
 * selection (Android A2 ≈ iOS D-094). The whole item list never crosses JNI: cells come in
 * windows of [WINDOW] around what the user sees.
 */
class BrowseModel(
    val open: BrowseHost,
    val place: Place,
    private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
    private val compute: kotlin.coroutines.CoroutineContext = Dispatchers.IO,
) {
    companion object {
        /** Cells per request; the core serves at most 200. */
        const val WINDOW = 100
        /** Windows kept around the last cell seen; others drop on a refresh. */
        const val KEPT_WINDOWS = 3
        const val SEARCH_DEBOUNCE_MS = 250L
    }

    @Volatile
    var settings: SavedView
        private set

    @Volatile
    var viewId: Long? = null
        private set

    @Volatile
    var total: Int = 0
        private set

    /** True until the first answer of the core for the current query. */
    @Volatile
    var loading: Boolean = true
        private set

    @Volatile
    var selecting: Boolean = false
        private set

    @Volatile
    var allSelected: Boolean = false
        private set

    private val picked = ConcurrentHashMap.newKeySet<String>()

    @Volatile
    var vocabulary: List<Tag> = emptyList()
        private set

    @Volatile
    var extensions: List<ExtensionCount> = emptyList()
        private set

    /** Index → cells of that window. */
    private val windows = ConcurrentHashMap<Int, List<ItemCell>>()
    private val requested = ConcurrentHashMap.newKeySet<Int>()
    private val viewGeneration = AtomicInteger(0)
    private val cellGeneration = AtomicInteger(0)
    private var lastSeen = 0
    private var active = false
    private var pendingSearch: Job? = null
    var wantsVocabulary: Boolean = false
    private val mutex = Mutex()

    /** Listeners notified when state that the Compose UI reads changes. */
    private val listeners = mutableListOf<() -> Unit>()

    init {
        var next = open.savedView
        if (next.place != place) {
            next = next.copy(filters = BrowseFilters(), search = "")
        }
        settings = next.copy(place = place)
    }

    fun addListener(listener: () -> Unit) {
        listeners += listener
    }

    fun removeListener(listener: () -> Unit) {
        listeners.remove(listener)
    }

    private fun notifyChanged() {
        listeners.toList().forEach { it() }
    }

    val query: LibraryQuery
        get() {
            val filters = settings.filters
            val text = settings.search.trim()
            val tags = place.fixedTag?.let { listOf(it) } ?: filters.tags
            return LibraryQuery(
                scope = place.scope,
                tags = tags.takeIf { it.isNotEmpty() },
                tagMode = when {
                    tags.isEmpty() -> null
                    place.fixedTag != null -> TagMode.Any
                    else -> filters.tagMode
                },
                ext = filters.ext,
                rating = filters.rating,
                shape = filters.shape?.wire,
                text = text.takeIf { it.isNotEmpty() },
                sort = settings.sort,
                ascending = settings.ascending,
            )
        }

    fun start() {
        if (active) return
        active = true
        open.register(this)
        open.remember(settings)
        if (viewId == null) {
            scope.launch { reload() }
        }
    }

    fun stop() {
        active = false
        open.unregister(this)
        pendingSearch?.cancel()
        viewGeneration.incrementAndGet()
        cellGeneration.incrementAndGet()
        val closing = viewId
        viewId = null
        windows.clear()
        requested.clear()
        if (closing != null) {
            scope.launch(compute) {
                runCatching { open.session.closeView(closing) }
            }
        }
        scope.cancel()
    }

    /**
     * Changes sort, look, filters or search; a query change opens a new view.
     * Search alone is debounced [SEARCH_DEBOUNCE_MS].
     */
    fun change(edit: (SavedView) -> SavedView) {
        val before = query
        val next = edit(settings)
        if (next == settings) return
        val searchOnly = next.copy(search = settings.search) == settings
        settings = next
        open.remember(settings)
        notifyChanged()
        if (query == before || !active) return
        pendingSearch?.cancel()
        if (searchOnly) {
            pendingSearch = scope.launch {
                delay(SEARCH_DEBOUNCE_MS)
                reload()
            }
        } else {
            scope.launch { reload() }
        }
    }

    fun resetFilters() {
        change { it.copy(filters = BrowseFilters()) }
    }

    suspend fun reload() {
        val current = viewGeneration.incrementAndGet()
        val session = open.session
        val q = query
        val old = viewId
        try {
            val opened = withContext(compute) {
                if (old != null) runCatching { session.closeView(old) }
                session.openView(q)
            }
            if (current != viewGeneration.get() || !active) {
                withContext(compute) { runCatching { session.closeView(opened.viewId) } }
                return
            }
            cellGeneration.incrementAndGet()
            viewId = opened.viewId
            total = opened.total
            windows.clear()
            requested.clear()
            allSelected = false
            picked.clear()
            loading = false
            notifyChanged()
            load(window = 0)
        } catch (error: Exception) {
            if (current != viewGeneration.get()) return
            viewId = null
            total = 0
            windows.clear()
            loading = false
            notifyChanged()
            open.report(error.message ?: error.toString())
        }
    }

    fun cell(at: Int): ItemCell? {
        val window = windows[at / WINDOW] ?: return null
        val offset = at % WINDOW
        return window.getOrNull(offset)
    }

    /** The user sees [index]: load its window and the next one when past halfway. */
    fun need(index: Int) {
        lastSeen = index
        val window = index / WINDOW
        load(window = window)
        if ((index % WINDOW) > WINDOW / 2) {
            load(window = window + 1)
        }
    }

    /** Core recomputed views: refresh nearby windows; drop the rest. */
    fun invalidated(views: List<ViewOpened>) {
        val id = viewId ?: return
        val match = views.firstOrNull { it.viewId == id } ?: return
        cellGeneration.incrementAndGet()
        total = match.total
        val seen = lastSeen / WINDOW
        val keep = (seen - KEPT_WINDOWS)..(seen + KEPT_WINDOWS)
        val kept = windows.filter { (key, _) -> keep.contains(key) && key * WINDOW < total }
        windows.clear()
        windows.putAll(kept)
        requested.clear()
        for (window in windows.keys.sorted()) {
            load(window = window, force = true)
        }
        if (windows.isEmpty()) {
            load(window = seen)
        }
        notifyChanged()
        if (wantsVocabulary) {
            scope.launch { loadVocabulary() }
        }
    }

    private fun load(window: Int, force: Boolean = false) {
        val id = viewId ?: return
        if (window < 0 || window * WINDOW >= total) return
        if (!force && (window in requested || windows.containsKey(window))) return
        requested.add(window)
        val current = cellGeneration.get()
        val session = open.session
        val start = window * WINDOW
        scope.launch {
            val cells = withContext(compute) {
                runCatching { session.range(id, start, WINDOW) }.getOrNull()
            }
            if (current != cellGeneration.get()) return@launch
            requested.remove(window)
            if (cells != null) {
                windows[window] = cells
                notifyChanged()
            }
        }
    }

    val selectedCount: Int
        get() = if (allSelected) maxOf(total - picked.size, 0) else picked.size

    fun isSelected(id: String): Boolean = allSelected != picked.contains(id)

    /** What selection actions act on (A3). */
    val target: ItemsTarget
        get() = ItemsTarget(viewId = viewId, all = allSelected, ids = picked.sorted())

    fun setSelecting(on: Boolean) {
        selecting = on
        if (!on) {
            allSelected = false
            picked.clear()
        }
        notifyChanged()
    }

    fun toggle(id: String) {
        if (!picked.add(id)) {
            picked.remove(id)
        }
        if (allSelected && picked.size >= total) {
            allSelected = false
            picked.clear()
        }
        notifyChanged()
    }

    fun selectAll() {
        allSelected = true
        picked.clear()
        notifyChanged()
    }

    fun deselectAll() {
        allSelected = false
        picked.clear()
        notifyChanged()
    }

    fun tap(cell: ItemCell, at: Int): BrowseTap {
        return if (selecting) {
            toggle(cell.id)
            BrowseTap.Toggled
        } else {
            BrowseTap.OpenPreview(at)
        }
    }

    fun startSelecting(with: ItemCell) {
        if (!selecting) selecting = true
        if (!isSelected(with.id)) toggle(with.id)
        notifyChanged()
    }

    suspend fun loadVocabulary() {
        val session = open.session
        vocabulary = withContext(compute) {
            runCatching { session.tags() }.getOrDefault(emptyList())
                .sortedBy { it.name.lowercase() }
        }
        extensions = withContext(compute) {
            runCatching { session.extensions() }.getOrDefault(emptyList())
        }
        notifyChanged()
    }

    fun toggleTag(name: String) {
        change { view ->
            val tags = view.filters.tags.toMutableList()
            if (!tags.remove(name)) tags.add(name)
            view.copy(filters = view.filters.copy(tags = tags))
        }
    }

    /** For tests: wait until [predicate] or timeout. */
    suspend fun waitUntil(timeoutMs: Long = 15_000, predicate: () -> Boolean): Boolean {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (!predicate()) {
            if (System.currentTimeMillis() >= deadline) return false
            delay(20)
            mutex.withLock { }
        }
        return true
    }
}

sealed class BrowseTap {
    data object Toggled : BrowseTap()
    data class OpenPreview(val index: Int) : BrowseTap()
}
