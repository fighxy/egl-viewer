package app.eglviewer.preview

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import kotlinx.coroutines.Job
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Which sharper decode a zoom asks for, and when that ask is cancelled (D-135). */
class SharpDecodeTest {
    @Test
    fun aZoomedPhotoDecodesAtTwiceTheScreenWhenItFits() {
        val request = selectSharp(zoomed())
        assertEquals(SharpRequest.Full(2160), request)
        val (width, height) = scaledLongSide(8000, 6000, 2160)
        assertTrue(argbBytes(width, height) <= 96L * 1024 * 1024)
    }

    @Test
    fun theSharpTargetNeverPasses4096() {
        val request = selectSharp(
            zoomed(
                screenPixels = 3000,
                sourceWidth = 10_000,
                sourceHeight = 7500,
                baseLongSide = 1080,
                retainedBytes = 1024 * 1024,
            ),
        )
        assertEquals(SharpRequest.Full(SHARP_MAX_LONG_SIDE), request)
    }

    @Test
    fun aPhotoAlreadyAsSharpAsTheFileIsNotDecodedAgain() {
        assertEquals(
            SharpRequest.None,
            selectSharp(zoomed(screenPixels = 1080, sourceWidth = 800, sourceHeight = 600, baseLongSide = 800)),
        )
    }

    @Test
    fun zoomedOutOrOffScreenDecodesNothing() {
        assertEquals(SharpRequest.None, selectSharp(zoomed(zoomed = false)))
        assertEquals(SharpRequest.None, selectSharp(zoomed(current = false)))
        assertEquals(SharpRequest.None, selectSharp(zoomed(scale = 1f)))
    }

    @Test
    fun aBudgetThatCannotHoldTwiceTheScreenStillHoldsASharperFullPhoto() {
        val available = 8_000_000
        val request = selectSharp(zoomed(budgetBytes = available, retainedBytes = 0, scale = 2f))
        val full = request as SharpRequest.Full
        assertTrue(full.longSide >= (1080 * SHARP_GAIN).toInt())
        assertTrue(full.longSide < 2160)
        val (width, height) = scaledLongSide(8000, 6000, full.longSide)
        assertTrue(argbBytes(width, height) <= available)
    }

    @Test
    fun aTightBudgetDecodesTheVisibleAreaInsteadOfTheWholePhoto() {
        val request = selectSharp(
            zoomed(
                screenPixels = 100,
                sourceWidth = 1000,
                sourceHeight = 1000,
                baseLongSide = 100,
                budgetBytes = 50_000,
                retainedBytes = 0,
                boxWidth = 100f,
                boxHeight = 100f,
                scale = 5f,
            ),
        )
        assertEquals(SharpRequest.Region(300, 300, 700, 700, 4), request)
        assertTrue(argbBytes(400 / 4, 400 / 4) <= 50_000)
    }

    @Test
    fun aPanInsideTheDecodedRegionDoesNotAskAgain() {
        val first = selectSharp(tight())
        val inside = selectSharp(tight(offsetX = 10f, covered = first))
        assertEquals(first, inside)
        val outside = selectSharp(tight(offsetX = 80f, covered = first))
        assertNotEquals(first, outside)
        assertTrue(outside is SharpRequest.Region)
    }

    @Test
    fun theVisibleFractionLandsBackOnTheBox() {
        val image = Size(4000f, 3000f)
        val box = Size(400f, 800f)
        val scale = 2f
        val offset = Offset(-40f, 0f)
        val visible = visibleFraction(image, box, scale, offset)
        val drawn = displayRect(visible, image, box, scale, offset)
        val fitted = ZoomGeometry.fitted(image, box)
        val dispW = fitted.width * scale
        val left = box.width / 2f + offset.x - dispW / 2f
        assertEquals(0f, drawn.left, 0.01f)
        assertEquals(box.width, drawn.right, 0.01f)
        assertTrue(drawn.top >= -0.01f)
        assertTrue(drawn.bottom <= box.height + 0.01f)
        assertTrue(left < 0f)
    }

    @Test
    fun zoomingOutOrLeavingThePageCancelsTheSharperDecode() {
        val request = selectSharp(zoomed())
        assertTrue(request is SharpRequest.Full)
        val loads = SharpLoads()
        val running = Job()
        var token = 0
        loads.update(request) { _, started ->
            token = started
            running
        }
        assertTrue(loads.isCurrent(token))
        assertFalse(running.isCancelled)

        val leave = selectSharp(zoomed(current = false))
        assertEquals(SharpRequest.None, leave)
        var launched = false
        loads.update(leave) { _, _ ->
            launched = true
            Job()
        }
        assertTrue("the decode still running is cancelled", running.isCancelled)
        assertFalse("zooming out does not start another decode", launched)
        assertFalse(loads.isCurrent(token))
        assertEquals(SharpRequest.None, loads.active)
    }

    @Test
    fun aNewerSharperRequestCancelsTheOneItReplaces() {
        val loads = SharpLoads()
        val first = Job()
        var firstToken = 0
        loads.update(SharpRequest.Full(2000)) { _, token ->
            firstToken = token
            first
        }
        val second = Job()
        var launches = 0
        loads.update(SharpRequest.Full(2000)) { _, _ ->
            launches += 1
            Job()
        }
        assertEquals("the same request is not started twice", 0, launches)
        assertFalse(first.isCancelled)

        loads.update(SharpRequest.Region(0, 0, 10, 10, 2)) { _, token ->
            assertFalse(loads.isCurrent(firstToken))
            assertTrue(loads.isCurrent(token))
            second
        }
        assertTrue(first.isCancelled)
        assertFalse(second.isCancelled)

        loads.cancel()
        assertTrue(second.isCancelled)
        assertEquals(SharpRequest.None, loads.active)
    }

    private fun zoomed(
        zoomed: Boolean = true,
        current: Boolean = true,
        screenPixels: Int = 1080,
        sourceWidth: Int = 8000,
        sourceHeight: Int = 6000,
        baseLongSide: Int = 1080,
        budgetBytes: Int = 96 * 1024 * 1024,
        retainedBytes: Int = 4 * 1024 * 1024,
        boxWidth: Float = 1080f,
        boxHeight: Float = 1920f,
        scale: Float = 2.5f,
        offsetX: Float = 0f,
        offsetY: Float = 0f,
        covered: SharpRequest = SharpRequest.None,
    ) = SharpContext(
        zoomed = zoomed,
        current = current,
        screenPixels = screenPixels,
        sourceWidth = sourceWidth,
        sourceHeight = sourceHeight,
        baseLongSide = baseLongSide,
        budgetBytes = budgetBytes,
        retainedBytes = retainedBytes,
        boxWidth = boxWidth,
        boxHeight = boxHeight,
        scale = scale,
        offsetX = offsetX,
        offsetY = offsetY,
        covered = covered,
    )

    private fun tight(offsetX: Float = 0f, covered: SharpRequest = SharpRequest.None) = zoomed(
        screenPixels = 100,
        sourceWidth = 1000,
        sourceHeight = 1000,
        baseLongSide = 100,
        budgetBytes = 50_000,
        retainedBytes = 0,
        boxWidth = 100f,
        boxHeight = 100f,
        scale = 5f,
        offsetX = offsetX,
        covered = covered,
    )
}
