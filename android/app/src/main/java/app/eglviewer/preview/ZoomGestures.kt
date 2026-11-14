package app.eglviewer.preview

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size

/** A settled or in-progress scale and offset. */
internal data class ZoomFrame(val scale: Float, val offset: Offset)

/**
 * What a pinch shows. [magnification] is cumulative from the start of the
 * gesture (1 = no change). [focal] is the point in the box the gesture started
 * on, and [focalPan] is how far that point has moved, so the image point under
 * the fingers stays under them.
 */
internal fun pinchZoom(
    settled: ZoomFrame,
    focal: Offset,
    box: Size,
    magnification: Float,
    focalPan: Offset,
): ZoomFrame {
    if (settled.scale <= 0f || box.width <= 0f || box.height <= 0f) return settled
    val to = (settled.scale * magnification).coerceIn(
        1f,
        ZoomGeometry.maxScale * ZoomGeometry.pinchHeadroom,
    )
    val anchored = ZoomGeometry.anchored(settled.offset, focal, box, settled.scale, to)
    return ZoomFrame(to, anchored + focalPan)
}

/** Where a pinch rests: fit below the zoom threshold, otherwise at most 6× and inside the edges. */
internal fun pinchSettled(live: ZoomFrame, image: Size, box: Size): ZoomFrame {
    val to = live.scale.coerceAtMost(ZoomGeometry.maxScale)
    if (to <= ZOOMED_EPSILON) return ZoomFrame(1f, Offset.Zero)
    return ZoomFrame(to, ZoomGeometry.clamp(live.offset, ZoomGeometry.limit(image, box, to)))
}

/** A pan while zoomed, pulled back past the edges. At fit the offset does not move. */
internal fun panZoom(settled: ZoomFrame, translation: Offset, image: Size, box: Size): ZoomFrame {
    if (settled.scale <= ZOOMED_EPSILON) return settled
    val moved = settled.offset + translation
    return ZoomFrame(settled.scale, ZoomGeometry.banded(moved, ZoomGeometry.limit(image, box, settled.scale)))
}

/**
 * Where a pan rests. Far enough past the left or right edge the photo springs
 * back to fit, and the pager takes the next swipe. Otherwise the offset stops
 * on the edge.
 */
internal fun panSettled(
    settled: ZoomFrame,
    translation: Offset,
    image: Size,
    box: Size,
    leaveDistance: Float = ZoomGeometry.leaveDistance,
): ZoomFrame {
    if (settled.scale <= ZOOMED_EPSILON) return ZoomFrame(1f, Offset.Zero)
    val moved = settled.offset + translation
    val limit = ZoomGeometry.limit(image, box, settled.scale)
    if (ZoomGeometry.leavesZoom(moved, limit, leaveDistance)) return ZoomFrame(1f, Offset.Zero)
    return ZoomFrame(settled.scale, ZoomGeometry.clamp(moved, limit))
}

/** A double tap zooms to 2.5× at [point], or back to fit when already zoomed. */
internal fun doubleTapZoom(scale: Float, point: Offset, image: Size, box: Size): ZoomFrame {
    if (scale > ZOOMED_EPSILON) return ZoomFrame(1f, Offset.Zero)
    val to = ZoomGeometry.doubleTapScale
    val anchored = ZoomGeometry.anchored(Offset.Zero, point, box, 1f, to)
    return ZoomFrame(to, ZoomGeometry.clamp(anchored, ZoomGeometry.limit(image, box, to)))
}

/**
 * The pager may take a horizontal drag only at fit. While zoomed the drag pans
 * the photo, including at an edge; paging waits until the zoom is released.
 */
internal fun pagerMayScroll(scale: Float): Boolean = scale <= ZOOMED_EPSILON
