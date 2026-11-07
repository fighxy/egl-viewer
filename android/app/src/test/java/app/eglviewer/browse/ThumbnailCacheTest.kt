package app.eglviewer.browse

import app.eglviewer.core.ItemCell
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pure JVM checks for when the grid asks the core for thumbnail bytes (D-120). */
class ThumbnailCacheTest {
    @Test
    fun shouldLoad_when_hasThumbnail() {
        assertTrue(
            ThumbnailCache.shouldLoad(
                ItemCell(id = "a", name = "a", hasThumbnail = true, thumbVersion = 1),
            ),
        )
    }

    @Test
    fun shouldLoad_when_version_positive_even_if_flag_false() {
        assertTrue(
            ThumbnailCache.shouldLoad(
                ItemCell(id = "b", name = "b", hasThumbnail = false, thumbVersion = 2),
            ),
        )
    }

    @Test
    fun shouldNotLoad_without_flag_or_version() {
        assertFalse(
            ThumbnailCache.shouldLoad(
                ItemCell(id = "c", name = "c", hasThumbnail = false, thumbVersion = 0),
            ),
        )
    }
}
