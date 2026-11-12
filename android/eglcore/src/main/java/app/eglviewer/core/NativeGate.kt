package app.eglviewer.core

import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/**
 * Keeps a native pointer alive until every call that already borrowed it has returned,
 * then hands it back for `egl_close`. A call that arrives after [retire] fails instead of
 * using a freed pointer (the close/use race on [Library]).
 *
 * [retire] does not hold the lock while it waits, so a callback on an in-flight call can
 * re-enter [use], see that the gate is closed, and return. Waiting while holding the lock
 * would deadlock that callback.
 */
internal class NativeGate(initial: Long) {
    class Closed : Exception("library is closed")

    private val lock = ReentrantLock()
    private val idle = lock.newCondition()
    private var handle = initial
    private var inFlight = 0

    inline fun <T> use(block: (Long) -> T): T {
        val borrowed = lock.withLock {
            if (handle == 0L) throw Closed()
            inFlight++
            handle
        }
        try {
            return block(borrowed)
        } finally {
            leave()
        }
    }

    /** Like [use], but a closed gate returns without calling [block]. */
    inline fun tryUse(block: (Long) -> Unit) {
        val borrowed = lock.withLock {
            if (handle == 0L) return
            inFlight++
            handle
        }
        try {
            block(borrowed)
        } finally {
            leave()
        }
    }

    /**
     * Rejects new borrows, waits until borrows already inside [use] / [tryUse] finish,
     * and returns the pointer. A second call returns null.
     */
    fun retire(): Long? {
        lock.lock()
        val closing = handle
        if (closing == 0L) {
            lock.unlock()
            return null
        }
        handle = 0L
        var interrupted = false
        try {
            while (inFlight > 0) {
                try {
                    idle.await()
                } catch (error: InterruptedException) {
                    interrupted = true
                }
            }
        } finally {
            lock.unlock()
            if (interrupted) Thread.currentThread().interrupt()
        }
        return closing
    }

    fun leave() {
        lock.withLock {
            inFlight--
            if (inFlight == 0) idle.signalAll()
        }
    }

    fun isClosed(): Boolean = lock.withLock { handle == 0L }
}

/** What a byte-returning core call means. `NOT_FOUND` is an absence, every other failure is an error. */
internal enum class BytePayload {
    Present,
    Missing,
    Error,
}

internal fun bytePayload(status: Int): BytePayload = when (status) {
    0 -> BytePayload.Present
    EglError.Code.NOT_FOUND.raw -> BytePayload.Missing
    else -> BytePayload.Error
}

/** A view window the core takes as `uint32`. A negative value would wrap to a huge index. */
internal fun requireViewWindow(start: Int, count: Int) {
    if (start < 0 || count < 0) {
        throw EglError(EglError.Code.INVALID, "negative view window")
    }
}
