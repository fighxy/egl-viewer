package app.eglviewer.preview

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Fit vs zoomed threshold used to free the pager's pans (D-128). */
class ZoomingImageTest {
    @Test
    fun fitScaleDoesNotCountAsZoomed() {
        assertFalse(1f > ZOOMED_EPSILON)
        assertFalse(1.01f > ZOOMED_EPSILON)
    }

    @Test
    fun pinchPastFitCountsAsZoomed() {
        assertTrue(1.02f > ZOOMED_EPSILON)
        assertTrue(2.5f > ZOOMED_EPSILON)
    }
}
