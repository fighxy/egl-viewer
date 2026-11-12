package app.eglviewer.adding

import java.util.concurrent.ConcurrentHashMap

/**
 * Cleanups for files an import is still reading (gallery copies, SAF grants).
 * [seal] is called only after the core has finished or cancelled the import, so a cleanup
 * never deletes a path the core is copying. A cleanup registered after [seal] is the
 * caller's to run.
 */
internal class ImportSources {
    private val cleanups = ConcurrentHashMap<Long, () -> Unit>()
    private val gate = Any()
    private var sealed = false

    /** False when the library is already closed and [cleanup] was not stored. */
    fun track(id: Long, cleanup: () -> Unit): Boolean = synchronized(gate) {
        if (sealed) return false
        cleanups[id] = cleanup
        true
    }

    fun remove(id: Long): (() -> Unit)? = synchronized(gate) {
        if (sealed) null else cleanups.remove(id)
    }

    /** Pending cleanups. Further [track] calls return false. */
    fun seal(): List<() -> Unit> = synchronized(gate) {
        sealed = true
        val pending = cleanups.values.toList()
        cleanups.clear()
        pending
    }
}
