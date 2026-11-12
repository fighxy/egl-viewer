package app.eglviewer.browse

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.async
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.isActive
import kotlinx.coroutines.sync.Semaphore
import kotlin.coroutines.coroutineContext

/**
 * One decode per key, at most [limit] at once (D-127).
 *
 * Two callers of the same original share the decode. A caller that is
 * cancelled stops waiting; the decode itself is cancelled only when nobody
 * is left waiting, so a page that is still next keeps the work a prefetch
 * already started. [CancellationException] is rethrown, not turned into a
 * missing bitmap.
 */
internal class SharedDecodes<T>(
    private val scope: CoroutineScope,
    private val limit: Int = 2,
    private val decode: suspend (String) -> T?,
) {
    private class Slot<T>(val deferred: Deferred<T?>) {
        var waiters: Int = 0
    }

    private val lock = Any()
    private val running = HashMap<String, Slot<T>>()
    private val gate = Semaphore(limit.coerceAtLeast(1))

    val runningCount: Int
        get() = synchronized(lock) { running.size }

    fun cancelAll() {
        val slots = synchronized(lock) {
            val all = running.values.toList()
            running.clear()
            all
        }
        slots.forEach { it.deferred.cancel() }
    }

    suspend fun load(key: String): T? {
        val slot = synchronized(lock) {
            val existing = running[key]
            val slot = if (existing != null && !existing.deferred.isCancelled) {
                existing
            } else {
                Slot(start(key)).also { running[key] = it }
            }
            slot.waiters += 1
            slot
        }
        try {
            return slot.deferred.await()
        } catch (error: CancellationException) {
            leave(key, slot)
            throw error
        } finally {
            if (coroutineContext.isActive) {
                synchronized(lock) {
                    if (running[key] === slot) {
                        slot.waiters -= 1
                        if (slot.waiters <= 0) running.remove(key)
                    }
                }
            }
        }
    }

    /** A caller stopped waiting. The last one cancels the decode. */
    private fun leave(key: String, slot: Slot<T>) {
        synchronized(lock) {
            val current = running[key] ?: return
            if (current !== slot) return
            current.waiters -= 1
            if (current.waiters <= 0) {
                current.deferred.cancel()
                running.remove(key)
            }
        }
    }

    private fun start(key: String): Deferred<T?> =
        scope.async {
            gate.acquire()
            try {
                ensureActive()
                decode(key)
            } finally {
                gate.release()
            }
        }
}
