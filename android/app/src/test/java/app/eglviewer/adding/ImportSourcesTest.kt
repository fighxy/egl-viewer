package app.eglviewer.adding

import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Import sources are deleted once, and only by whoever still owns the cleanup. */
class ImportSourcesTest {
    @Test
    fun sealHandsBackWhatWasTrackedAndRejectsLaterTracks() {
        val sources = ImportSources()
        val ran = AtomicInteger()
        val cleanup = {
            ran.incrementAndGet()
            Unit
        }
        assertTrue(sources.track(1L, cleanup))
        val pending = sources.seal()
        assertEquals(1, pending.size)
        assertFalse(sources.track(2L, cleanup))
        assertEquals(null, sources.remove(1L))
        pending.forEach { it() }
        assertEquals(1, ran.get())
    }

    @Test
    fun removeBeforeSealKeepsTheCleanupOutOfTheSeal() {
        val sources = ImportSources()
        var ran = 0
        val cleanup = { ran += 1 }
        sources.track(7L, cleanup)
        val taken = sources.remove(7L)
        assertTrue(sources.seal().isEmpty())
        taken?.invoke()
        assertEquals(1, ran)
    }
}
