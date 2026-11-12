package app.eglviewer.core

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Close must not free a pointer a call is still using, and a late call must not see it. */
class NativeGateTest {
    @Test
    fun retireWaitsForTheBorrowThenRejectsNewOnes() {
        val gate = NativeGate(42L)
        val inside = CountDownLatch(1)
        val release = CountDownLatch(1)
        val saw = AtomicInteger()
        val worker = Thread {
            gate.use {
                saw.set(it.toInt())
                inside.countDown()
                assertTrue(release.await(5, TimeUnit.SECONDS))
            }
        }
        worker.start()
        assertTrue(inside.await(5, TimeUnit.SECONDS))

        val retired = AtomicReference<Long?>()
        val closer = Thread { retired.set(gate.retire()) }
        closer.start()
        val deadline = System.currentTimeMillis() + 2_000
        while (!gate.isClosed() && System.currentTimeMillis() < deadline) {
            Thread.sleep(5)
        }
        assertTrue(gate.isClosed())
        // The borrow is still inside use(), so retire has not returned the pointer.
        assertTrue(closer.isAlive)
        assertNull(retired.get())
        var late: Long? = null
        val rejected = runCatching {
            gate.use { late = it }
        }.exceptionOrNull()
        assertTrue(rejected is NativeGate.Closed)
        assertNull(late)

        release.countDown()
        closer.join(5_000)
        worker.join(5_000)
        assertEquals(42L, retired.get())
        assertEquals(42, saw.get())
        assertNull(gate.retire())
    }

    @Test
    fun bytePayloadTreatsNotFoundAsMissingAndOtherFailuresAsErrors() {
        assertEquals(BytePayload.Present, bytePayload(0))
        assertEquals(BytePayload.Missing, bytePayload(EglError.Code.NOT_FOUND.raw))
        assertEquals(BytePayload.Error, bytePayload(EglError.Code.LOCKED.raw))
        assertEquals(BytePayload.Error, bytePayload(EglError.Code.READ_ONLY.raw))
        assertEquals(BytePayload.Error, bytePayload(EglError.Code.STORAGE.raw))
    }

    @Test
    fun aNegativeViewWindowIsRejected() {
        requireViewWindow(0, 0)
        requireViewWindow(10, 100)
        val error = runCatching { requireViewWindow(-1, 10) }.exceptionOrNull() as EglError
        assertEquals(EglError.Code.INVALID, error.code)
        val count = runCatching { requireViewWindow(0, -1) }.exceptionOrNull() as EglError
        assertEquals(EglError.Code.INVALID, count.code)
    }
}
