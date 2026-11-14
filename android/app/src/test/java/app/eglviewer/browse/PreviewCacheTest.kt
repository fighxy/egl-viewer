package app.eglviewer.browse

import android.content.ComponentCallbacks2
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Byte budget, screen-size subsample, and which thumbnail is already in memory (D-127). */
class PreviewCacheTest {
    @Test
    fun budgetIsAnEighthOfTheHeapCappedAtTheIosLimit() {
        val cap = PreviewMemoryBudget.CAP_BYTES
        assertEquals(cap, PreviewMemoryBudget.bytes(2L * 1024 * 1024 * 1024))
        assertEquals(cap, PreviewMemoryBudget.bytes(768L * 1024 * 1024))
        assertEquals(32 * 1024 * 1024, PreviewMemoryBudget.bytes(256L * 1024 * 1024))
        assertEquals(16 * 1024 * 1024, PreviewMemoryBudget.bytes(128L * 1024 * 1024))
    }

    @Test
    fun budgetStaysInsideASmallHeap() {
        assertEquals(PreviewMemoryBudget.FLOOR_BYTES, PreviewMemoryBudget.bytes(32L * 1024 * 1024))
        assertEquals(6 * 1024 * 1024, PreviewMemoryBudget.bytes(6L * 1024 * 1024))
        assertEquals(1, PreviewMemoryBudget.bytes(0))
        assertEquals(1, PreviewMemoryBudget.bytes(-1))
    }

    @Test
    fun aLargePhotoIsDecodedToTheScreenNotFullResolution() {
        val full = bitmapDecode(8000, 6000, 1080)
        assertTrue("inSampleSize drops the 48 MP file", full.sampleSize >= 4)
        val (width, height) = scaledSize(8000, 6000, 1080)
        assertTrue(maxOf(width, height) <= 1080)
        assertTrue(maxOf(width, height) >= 1000)
        assertTrue(width < 8000 && height < 6000)
    }

    @Test
    fun subsampleLandsOnTheScreenWhenItIsNotExact() {
        assertEquals(2, bitmapDecode(4032, 3024, 1080).sampleSize)
        assertEquals(1080, bitmapDecode(4032, 3024, 1080).targetLongSide)
        assertEquals(1080 to 810, scaledSize(4032, 3024, 1080))
    }

    @Test
    fun aSmallPhotoIsNotEnlarged() {
        assertEquals(1, sampleSize(800, 600, 1080))
        assertEquals(0, bitmapDecode(800, 600, 1080).targetLongSide)
        assertEquals(800 to 600, scaledSize(800, 600, 1080))
        assertEquals(0 to 0, scaledSize(0, 0, 1080))
    }

    @Test
    fun anExactPowerOfTwoNeedsNoSecondScale() {
        assertEquals(4, sampleSize(4000, 2000, 1000))
        assertEquals(0, bitmapDecode(4000, 2000, 1000).targetLongSide)
        assertEquals(1000 to 500, scaledSize(4000, 2000, 1000))
    }

    @Test
    fun theGridThumbnailKeyMatchesOnlyThatItemAndVersion() {
        assertEquals("ab#3#256", thumbnailCacheKey("ab", 3, 256))
        assertEquals(256, cachedThumbnailPixels(thumbnailCacheKey("ab", 3, 256), "ab", 3))
        assertEquals(512, cachedThumbnailPixels("id#7#512", "id", 7))
        assertNull(cachedThumbnailPixels(thumbnailCacheKey("ab", 3, 256), "ab", 4))
        assertNull(cachedThumbnailPixels(thumbnailCacheKey("ab", 3, 256), "a", 3))
        assertEquals(10, cachedThumbnailPixels(thumbnailCacheKey("a#b", 1, 10), "a#b", 1))
    }

    @Test
    fun cacheKeysClampANonPositivePixelCap() {
        assertEquals("a#1#1", OriginalCache.cacheKey("a", 1, 0))
    }

    @Test
    fun aCacheKeyRoundTripsForAFullPhotoAndARegionAndAnIdThatContainsAHash() {
        assertEquals(OriginalRequest.Full("a#b", 1080), OriginalCache.parseKey("a#b#7#1080"))
        val region = OriginalCache.regionKey("a#b", 7, 10, 20, 110, 220, 2)
        assertEquals(OriginalRequest.Region("a#b", 10, 20, 110, 220, 2), OriginalCache.parseKey(region))
        assertNull(OriginalCache.parseKey("nope"))
    }

    @Test
    fun memoryTrimClearsOnPressureButNotWhenAnotherActivityCoversTheApp() {
        assertFalse(OriginalCache.clearOnMemoryTrim(ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN))
        assertFalse(OriginalCache.clearOnMemoryTrim(ComponentCallbacks2.TRIM_MEMORY_RUNNING_MODERATE))
        assertTrue(OriginalCache.clearOnMemoryTrim(ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW))
        assertTrue(OriginalCache.clearOnMemoryTrim(ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL))
        assertTrue(OriginalCache.clearOnMemoryTrim(ComponentCallbacks2.TRIM_MEMORY_BACKGROUND))
        assertTrue(OriginalCache.clearOnMemoryTrim(ComponentCallbacks2.TRIM_MEMORY_COMPLETE))
    }
}
