package app.eglviewer.preview

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Where a zoom lands, and when a pan gives the pager its swipe back (D-129, D-135). */
class ZoomGeometryTest {
    private val box = Size(400f, 800f)

    /** A landscape photo: fitted 400 × 300, centred. */
    private val photo = Size(4000f, 3000f)

    @Test
    fun aFittedPhotoCannotMoveAndAZoomedOneStopsAtItsEdges() {
        assertEquals(400f, ZoomGeometry.fitted(photo, box).width, 0.01f)
        assertEquals(300f, ZoomGeometry.fitted(photo, box).height, 0.01f)
        assertEquals(Size.Zero, ZoomGeometry.limit(photo, box, 1f))
        // At 2× it is 800 × 600: 200 px each side horizontally, still shorter than the box.
        val limit = ZoomGeometry.limit(photo, box, 2f)
        assertEquals(200f, limit.width, 0.01f)
        assertEquals(0f, limit.height, 0.01f)
        val clamped = ZoomGeometry.clamp(Offset(-350f, 90f), limit)
        assertEquals(-200f, clamped.x, 0.01f)
        assertEquals(0f, clamped.y, 0.01f)
    }

    @Test
    fun theZoomKeepsThePointUnderTheFingers() {
        val point = Offset(380f, 400f)
        val offset = ZoomGeometry.anchored(Offset.Zero, point, box, from = 1f, to = 2.5f)
        // The image point under the tap was 180 px right of centre; at 2.5× it would sit at 450,
        // so the image moves 270 px left to keep it at 180.
        assertEquals(-270f, offset.x, 0.001f)
        assertEquals(0f, offset.y, 0.001f)
        val centre = ZoomGeometry.anchored(Offset.Zero, Offset(200f, 400f), box, from = 1f, to = 3f)
        assertEquals(Offset.Zero, centre)
    }

    @Test
    fun aPinchFollowsTheFingersAsTheyMove() {
        val focal = Offset(380f, 400f)
        val frame = pinchZoom(
            ZoomFrame(1f, Offset.Zero),
            focal,
            box,
            magnification = 2.5f,
            focalPan = Offset(12f, -8f),
        )
        assertEquals(2.5f, frame.scale, 0.001f)
        val centre = Offset(box.width / 2f, box.height / 2f)
        val imagePoint = focal - centre
        val at = centre + frame.offset + imagePoint * frame.scale
        assertEquals(focal.x + 12f, at.x, 0.01f)
        assertEquals(focal.y - 8f, at.y, 0.01f)
    }

    @Test
    fun aPinchSettlesBetweenFitAndSix() {
        val over = pinchZoom(ZoomFrame(1f, Offset.Zero), Offset(200f, 400f), box, magnification = 10f, focalPan = Offset.Zero)
        assertEquals(ZoomGeometry.maxScale * ZoomGeometry.pinchHeadroom, over.scale, 0.001f)
        val settled = pinchSettled(over, photo, box)
        assertEquals(ZoomGeometry.maxScale, settled.scale, 0.001f)
        val limit = ZoomGeometry.limit(photo, box, ZoomGeometry.maxScale)
        assertTrue(kotlin.math.abs(settled.offset.x) <= limit.width + 0.01f)
        assertTrue(kotlin.math.abs(settled.offset.y) <= limit.height + 0.01f)

        val back = pinchZoom(ZoomFrame(2f, Offset(40f, 0f)), Offset(200f, 400f), box, magnification = 0.1f, focalPan = Offset.Zero)
        assertEquals(1f, back.scale, 0.001f)
        assertEquals(ZoomFrame(1f, Offset.Zero), pinchSettled(back, photo, box))
    }

    @Test
    fun aDoubleTapZoomsAtTheTapAndStaysInsideTheEdges() {
        val frame = doubleTapZoom(1f, Offset(380f, 400f), photo, box)
        assertEquals(ZoomGeometry.doubleTapScale, frame.scale, 0.001f)
        assertEquals(-270f, frame.offset.x, 0.01f)
        assertEquals(0f, frame.offset.y, 0.01f)
        assertEquals(ZoomFrame(1f, Offset.Zero), doubleTapZoom(frame.scale, Offset(10f, 10f), photo, box))
    }

    @Test
    fun pastTheEdgeAPanPullsAndFarPastItLeavesTheZoom() {
        val limit = Size(200f, 0f)
        val banded = ZoomGeometry.banded(Offset(300f, 0f), limit)
        assertEquals(200f + 100f * ZoomGeometry.rubber, banded.x, 0.001f)
        assertEquals(-150f, ZoomGeometry.banded(Offset(-150f, 0f), limit).x, 0.001f)
        assertEquals(60f, ZoomGeometry.overshoot(Offset(260f, 0f), limit), 0.001f)
        assertEquals(0f, ZoomGeometry.overshoot(Offset(-120f, 0f), limit), 0.001f)
        assertFalse(ZoomGeometry.leavesZoom(Offset(260f, 0f), limit))
        assertTrue(ZoomGeometry.leavesZoom(Offset(261f, 0f), limit))
    }

    @Test
    fun aPanPastTheSideLeavesTheZoomSoTheNextSwipeCanPage() {
        val stay = panSettled(ZoomFrame(2f, Offset.Zero), Offset(260f, 0f), photo, box)
        assertEquals(2f, stay.scale, 0.001f)
        assertEquals(200f, stay.offset.x, 0.01f)
        assertFalse(pagerMayScroll(stay.scale))

        // Still zoomed while the finger is down, so the pager does not take this drag.
        val pulling = panZoom(ZoomFrame(2f, Offset.Zero), Offset(261f, 0f), photo, box)
        assertEquals(2f, pulling.scale, 0.001f)
        assertFalse(pagerMayScroll(pulling.scale))

        val leave = panSettled(ZoomFrame(2f, Offset.Zero), Offset(261f, 0f), photo, box)
        assertEquals(ZoomFrame(1f, Offset.Zero), leave)
        assertTrue(pagerMayScroll(leave.scale))
    }

    @Test
    fun aVerticalPullDoesNotLeaveTheZoomAndAnUnzoomedPanDoesNotMove() {
        val vertical = panSettled(ZoomFrame(2f, Offset.Zero), Offset(0f, 400f), photo, box)
        assertEquals(2f, vertical.scale, 0.001f)
        assertEquals(0f, vertical.offset.y, 0.01f)
        assertFalse(pagerMayScroll(vertical.scale))
        val fit = panZoom(ZoomFrame(1f, Offset.Zero), Offset(80f, 20f), photo, box)
        assertEquals(ZoomFrame(1f, Offset.Zero), fit)
        assertTrue(pagerMayScroll(1f))
        assertFalse(pagerMayScroll(1.02f))
    }
}
