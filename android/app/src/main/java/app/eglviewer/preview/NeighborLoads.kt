package app.eglviewer.preview

import kotlinx.coroutines.Job

/**
 * The pages on both sides of [index]. The page on screen is loaded by the
 * pager, not by this list.
 */
internal fun neighborIndices(index: Int, total: Int): List<Int> {
    if (total <= 0) return emptyList()
    val at = index.coerceIn(0, total - 1)
    return listOf(at - 1, at + 1).filter { it in 0 until total }
}

/**
 * What to start and what to drop when the resting page changes (D-127).
 *
 * [keep] is the page on screen and its neighbours: a prefetch that is still
 * one of those is not cancelled, so landing on a page does not throw away the
 * decode that warmed it. [start] is only the neighbours that are not already
 * running. Callers start those before cancelling [cancel].
 */
internal data class NeighborPlan(
    val start: List<Int>,
    val cancel: List<Int>,
)

internal fun neighborPlan(inflight: Set<Int>, index: Int, total: Int): NeighborPlan {
    if (total <= 0) return NeighborPlan(start = emptyList(), cancel = inflight.sorted())
    val at = index.coerceIn(0, total - 1)
    val next = neighborIndices(at, total)
    val keep = next.toSet() + at
    return NeighborPlan(
        start = next.filter { it !in inflight },
        cancel = inflight.filter { it !in keep }.sorted(),
    )
}

/**
 * Prefetch jobs for the pages beside the one on screen.
 *
 * A new size cancels every job first: the old bitmaps are the wrong number of
 * pixels. A page whose cell is not loaded yet returns null from [launch] and
 * is not recorded, so the next update tries it again. A finished job is
 * dropped, so a later visit can ask again (a hit is then the cache).
 */
internal class NeighborLoads {
    private var pixels: Int = Int.MIN_VALUE
    private val jobs = LinkedHashMap<Int, Job>()

    val inflight: Set<Int> get() = jobs.keys.toSet()

    fun update(index: Int, total: Int, pixels: Int, launch: (Int) -> Job?) {
        if (pixels <= 0 || total <= 0) return
        if (this.pixels != pixels) {
            jobs.values.forEach { it.cancel() }
            jobs.clear()
            this.pixels = pixels
        }
        jobs.entries.filter { !it.value.isActive }.map { it.key }.forEach { jobs.remove(it) }
        val plan = neighborPlan(jobs.keys, index, total)
        for (page in plan.start) {
            val job = launch(page) ?: continue
            jobs[page] = job
        }
        for (page in plan.cancel) {
            jobs.remove(page)?.cancel()
        }
    }

    fun cancelAll() {
        jobs.values.forEach { it.cancel() }
        jobs.clear()
    }
}
