package app.eglviewer.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Host JVM tests for Settings helpers (Android A7 ≈ D-099 / D-107). */
class SettingsFormatTest {
    @Test
    fun formatBytesUsesBinaryUnits() {
        assertEquals("512 B", formatBytes(512))
        assertTrue(formatBytes(1536).endsWith(" KB"))
        assertTrue(formatBytes(3L * 1024 * 1024).endsWith(" MB"))
    }
}
