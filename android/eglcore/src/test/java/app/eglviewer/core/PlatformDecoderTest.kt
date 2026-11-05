package app.eglviewer.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Host JVM: decoder refuses empty / junk without a device ImageDecoder. */
class PlatformDecoderTest {
    @Test
    fun emptyBytesAreRejected() {
        assertNull(PlatformDecoder.decode(ByteArray(0)))
    }

    @Test
    fun junkBytesAreRejectedOnHostJvm() {
        // No Android ImageDecoder on the host JVM unit test classpath path that
        // succeeds; decode returns null rather than throwing.
        assertNull(PlatformDecoder.decode("not an image".toByteArray()))
    }

    @Test
    fun maxPixelsMatchesIosBudget() {
        assertEquals(2048, PlatformDecoder.MAX_PIXELS)
    }
}
