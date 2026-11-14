package app.eglviewer.preview

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/**
 * The arithmetic of the zoom, apart from the view (D-129, D-135).
 *
 * Offsets are measured from the centre of the box. The view scales the image
 * about that centre, then moves it by the offset. The same rules as iOS
 * `ZoomGeometry`.
 */
internal object ZoomGeometry {
    const val maxScale = 6f
    const val doubleTapScale = 2.5f

    /** Past an edge a pan moves at this fraction of the finger's speed. */
    const val rubber = 0.35f

    /** A pan this far past the left or right edge leaves the zoom, in the same unit as the offset. */
    const val leaveDistance = 60f

    /** During a pinch the scale may pass [maxScale], then it settles back. */
    const val pinchHeadroom = 1.15f

    /** The image fitted into the box, at scale 1. */
    fun fitted(image: Size, box: Size): Size {
        if (image.width <= 0f || image.height <= 0f || box.width <= 0f || box.height <= 0f) return box
        val ratio = min(box.width / image.width, box.height / image.height)
        return Size(image.width * ratio, image.height * ratio)
    }

    /**
     * The furthest the image may move from the centre on each axis at [scale]:
     * its edges stay at or past the box's, and an axis the image does not fill
     * stays centred.
     */
    fun limit(image: Size, box: Size, scale: Float): Size {
        val size = fitted(image, box)
        return Size(
            width = max((size.width * scale - box.width) / 2f, 0f),
            height = max((size.height * scale - box.height) / 2f, 0f),
        )
    }

    fun clamp(offset: Offset, limit: Size): Offset = Offset(
        offset.x.coerceIn(-limit.width, limit.width),
        offset.y.coerceIn(-limit.height, limit.height),
    )

    /** Past the limit a pan moves at [rubber] of the finger's speed, so the edge is felt. */
    fun banded(offset: Offset, limit: Size): Offset {
        fun band(value: Float, edge: Float): Float {
            val over = abs(value) - edge
            if (over <= 0f) return value
            return (edge + over * rubber) * if (value < 0f) -1f else 1f
        }
        return Offset(band(offset.x, limit.width), band(offset.y, limit.height))
    }

    /**
     * The offset that keeps the image point under [at] (in the box) where it is
     * while the scale goes from [from] to [to].
     */
    fun anchored(offset: Offset, at: Offset, box: Size, from: Float, to: Float): Offset {
        if (from <= 0f) return offset
        val x = at.x - box.width / 2f
        val y = at.y - box.height / 2f
        val ratio = to / from
        return Offset(
            x - (x - offset.x) * ratio,
            y - (y - offset.y) * ratio,
        )
    }

    /** How far a pan went past the left or right edge, positive either way. */
    fun overshoot(offset: Offset, limit: Size): Float = max(abs(offset.x) - limit.width, 0f)

    /**
     * A horizontal pan far enough past the edge that releasing it leaves the
     * zoom, so the next swipe can page (D-129).
     */
    fun leavesZoom(moved: Offset, limit: Size, leaveDistance: Float = this.leaveDistance): Boolean {
        val shown = banded(moved, limit)
        return overshoot(shown, limit) > leaveDistance * rubber
    }
}

/** Scale above fit counts as zoomed: the pager stops paging and pans move the photo. */
internal const val ZOOMED_EPSILON = 1.01f
