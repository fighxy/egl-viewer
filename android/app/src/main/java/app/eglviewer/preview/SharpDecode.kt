package app.eglviewer.preview

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import kotlinx.coroutines.Job
import kotlin.math.max
import kotlin.math.min

/** Twice the screen, and never past 4,096 px, matching the iOS cap (D-129). */
internal const val SHARP_MAX_LONG_SIDE = 4096

/** A full-photo decode has to be this much larger than the screen decode to be worth it. */
internal const val SHARP_GAIN = 1.2f

/** Extra image around the visible area, so a small pan does not decode again. */
internal const val REGION_MARGIN = 0.5f

/** A visible edge this close to a decoded region still counts as inside it. */
internal const val REGION_COVERS_SLOP = 0.01f

/** A rectangle in fractions of the source image, each edge in 0..1. */
internal data class NormRect(
    val left: Float,
    val top: Float,
    val right: Float,
    val bottom: Float,
)

/** A rectangle in the box, in the same pixels as the zoom offset. */
internal data class BoxRect(
    val left: Float,
    val top: Float,
    val right: Float,
    val bottom: Float,
) {
    val width: Float get() = right - left
    val height: Float get() = bottom - top
}

/** What to decode for the zoomed page, or nothing. */
internal sealed interface SharpRequest {
    data object None : SharpRequest

    /** The whole photo, long side [longSide] px. */
    data class Full(val longSide: Int) : SharpRequest

    /** A source rectangle, subsampled by [sampleSize] (a power of two). */
    data class Region(
        val left: Int,
        val top: Int,
        val right: Int,
        val bottom: Int,
        val sampleSize: Int,
    ) : SharpRequest
}

/** Everything [selectSharp] needs, and the request already in flight or on screen. */
internal data class SharpContext(
    val zoomed: Boolean,
    val current: Boolean,
    val screenPixels: Int,
    val sourceWidth: Int,
    val sourceHeight: Int,
    val baseLongSide: Int,
    val budgetBytes: Int,
    val retainedBytes: Int,
    val boxWidth: Float,
    val boxHeight: Float,
    val scale: Float,
    val offsetX: Float,
    val offsetY: Float,
    val covered: SharpRequest = SharpRequest.None,
)

/** Decoded ARGB byte cost of a bitmap this size. */
internal fun argbBytes(width: Int, height: Int): Long {
    if (width <= 0 || height <= 0) return 0L
    return width.toLong() * height.toLong() * 4L
}

/** Output size when the long side is [longSide], never larger than the source. */
internal fun scaledLongSide(width: Int, height: Int, longSide: Int): Pair<Int, Int> {
    if (width <= 0 || height <= 0 || longSide <= 0) return 0 to 0
    val sourceLong = max(width, height)
    if (longSide >= sourceLong) return width to height
    val scale = longSide.toFloat() / sourceLong.toFloat()
    return max(1, (width * scale).toInt()) to max(1, (height * scale).toInt())
}

/** The iOS target: twice the screen, at most 4,096, and not past the file. */
internal fun idealSharpLongSide(screenPixels: Int, sourceWidth: Int, sourceHeight: Int): Int {
    val sourceLong = max(sourceWidth, sourceHeight)
    if (screenPixels <= 0 || sourceLong <= 0) return 0
    val twice = screenPixels.toLong() * 2L
    return min(twice, SHARP_MAX_LONG_SIDE.toLong()).toInt().coerceAtMost(sourceLong)
}

/** Largest long side at or under [maxLongSide] whose bitmap fits in [availableBytes]. */
internal fun largestLongSide(
    sourceWidth: Int,
    sourceHeight: Int,
    maxLongSide: Int,
    availableBytes: Long,
): Int {
    if (sourceWidth <= 0 || sourceHeight <= 0 || maxLongSide <= 0 || availableBytes < 4L) return 0
    val capped = min(maxLongSide, max(sourceWidth, sourceHeight))
    var lo = 1
    var hi = capped
    var best = 0
    while (lo <= hi) {
        val mid = (lo + hi) / 2
        val (width, height) = scaledLongSide(sourceWidth, sourceHeight, mid)
        if (argbBytes(width, height) <= availableBytes) {
            best = mid
            lo = mid + 1
        } else {
            hi = mid - 1
        }
    }
    return best
}

/**
 * The part of the image that lies inside the box, as fractions of the source.
 * [image] is the source aspect (pixel size, not the fitted size).
 */
internal fun visibleFraction(image: Size, box: Size, scale: Float, offset: Offset): NormRect {
    if (image.width <= 0f || image.height <= 0f || box.width <= 0f || box.height <= 0f || scale <= 0f) {
        return NormRect(0f, 0f, 1f, 1f)
    }
    val fitted = ZoomGeometry.fitted(image, box)
    val dispW = fitted.width * scale
    val dispH = fitted.height * scale
    if (dispW <= 0f || dispH <= 0f) return NormRect(0f, 0f, 1f, 1f)
    val left = box.width / 2f + offset.x - dispW / 2f
    val top = box.height / 2f + offset.y - dispH / 2f
    val iLeft = max(left, 0f)
    val iTop = max(top, 0f)
    val iRight = min(left + dispW, box.width)
    val iBottom = min(top + dispH, box.height)
    if (iRight <= iLeft || iBottom <= iTop) return NormRect(0f, 0f, 1f, 1f)
    return NormRect(
        left = ((iLeft - left) / dispW).coerceIn(0f, 1f),
        top = ((iTop - top) / dispH).coerceIn(0f, 1f),
        right = ((iRight - left) / dispW).coerceIn(0f, 1f),
        bottom = ((iBottom - top) / dispH).coerceIn(0f, 1f),
    )
}

/** Where a source fraction is drawn in the box at this zoom. */
internal fun displayRect(
    fraction: NormRect,
    image: Size,
    box: Size,
    scale: Float,
    offset: Offset,
): BoxRect {
    val fitted = ZoomGeometry.fitted(image, box)
    val dispW = fitted.width * scale
    val dispH = fitted.height * scale
    val left = box.width / 2f + offset.x - dispW / 2f
    val top = box.height / 2f + offset.y - dispH / 2f
    return BoxRect(
        left = left + fraction.left * dispW,
        top = top + fraction.top * dispH,
        right = left + fraction.right * dispW,
        bottom = top + fraction.bottom * dispH,
    )
}

internal fun expandFraction(rect: NormRect, margin: Float): NormRect {
    val width = (rect.right - rect.left).coerceAtLeast(0f)
    val height = (rect.bottom - rect.top).coerceAtLeast(0f)
    return NormRect(
        left = (rect.left - width * margin).coerceIn(0f, 1f),
        top = (rect.top - height * margin).coerceIn(0f, 1f),
        right = (rect.right + width * margin).coerceIn(0f, 1f),
        bottom = (rect.bottom + height * margin).coerceIn(0f, 1f),
    )
}

/**
 * The finest power-of-two sample whose decoded region fits in [availableBytes]
 * and is sharper than the screen-size bitmap. Null when nothing qualifies.
 */
internal fun sharpestSample(
    regionWidth: Int,
    regionHeight: Int,
    availableBytes: Long,
    sourceLongSide: Int,
    baseLongSide: Int,
): Int? {
    if (regionWidth <= 1 || regionHeight <= 1 || availableBytes < 4L) return null
    if (sourceLongSide <= 0 || baseLongSide <= 0) return null
    val maxSample = sourceLongSide.toFloat() / baseLongSide.toFloat()
    var sample = 1
    while (sample.toFloat() < maxSample && sample <= 64) {
        if (regionWidth / sample < 1 || regionHeight / sample < 1) return null
        val width = regionWidth / sample
        val height = regionHeight / sample
        if (argbBytes(width, height) <= availableBytes) return sample
        sample *= 2
    }
    return null
}

/**
 * What the zoomed page should decode.
 *
 * The whole photo at twice the screen (at most 4,096) when those bytes fit in
 * the budget beside the screen-size bitmap. Otherwise the visible area, with a
 * margin, at the finest sample that fits and is actually sharper. A region
 * that still covers the visible area is kept, so a small pan does not cancel
 * the decode. Zoomed out, or not the page on screen, decodes nothing.
 */
internal fun selectSharp(context: SharpContext): SharpRequest {
    if (!context.zoomed || !context.current || context.scale <= ZOOMED_EPSILON) return SharpRequest.None
    val sourceW = context.sourceWidth
    val sourceH = context.sourceHeight
    if (sourceW <= 0 || sourceH <= 0 || context.boxWidth <= 1f || context.boxHeight <= 1f) {
        return SharpRequest.None
    }
    val base = context.baseLongSide.coerceAtLeast(1)
    val ideal = idealSharpLongSide(context.screenPixels, sourceW, sourceH)
    if (ideal <= base) return SharpRequest.None
    val available = context.budgetBytes.toLong() - context.retainedBytes.coerceAtLeast(0).toLong()
    if (available < 4L) return SharpRequest.None

    val image = Size(sourceW.toFloat(), sourceH.toFloat())
    val box = Size(context.boxWidth, context.boxHeight)
    val offset = Offset(context.offsetX, context.offsetY)
    val visible = visibleFraction(image, box, context.scale, offset)
    val area = ((visible.right - visible.left) * (visible.bottom - visible.top)).coerceIn(0f, 1f)

    val bestFull = largestLongSide(sourceW, sourceH, ideal, available)
    if (bestFull > base) {
        val worthFull = bestFull >= (base * SHARP_GAIN).toInt() || area >= 0.85f
        if (worthFull) return SharpRequest.Full(bestFull)
    }

    val region = regionRequest(sourceW, sourceH, visible, available, base)
    if (region != null) {
        val covered = context.covered
        if (covered is SharpRequest.Region && regionContains(covered, visible, sourceW, sourceH)) {
            return covered
        }
        return region
    }
    if (bestFull > base) return SharpRequest.Full(bestFull)
    return SharpRequest.None
}

/**
 * One sharper decode at a time. A new request cancels the job still running;
 * leaving the zoom or the page passes [SharpRequest.None] and cancels without
 * starting another. [isCurrent] is false for a token whose request was replaced.
 */
internal class SharpLoads {
    private var request: SharpRequest = SharpRequest.None
    private var job: Job? = null
    private var generation: Int = 0

    val active: SharpRequest get() = request

    fun update(next: SharpRequest, launch: (SharpRequest, Int) -> Job) {
        val same = next == request && (next is SharpRequest.None || (job != null && job?.isCancelled != true))
        if (same) return
        job?.cancel()
        job = null
        request = next
        generation += 1
        if (next is SharpRequest.None) return
        val token = generation
        job = launch(next, token)
    }

    fun isCurrent(token: Int): Boolean = token == generation && request !is SharpRequest.None

    fun cancel() {
        job?.cancel()
        job = null
        request = SharpRequest.None
        generation += 1
    }
}

private fun regionRequest(
    sourceWidth: Int,
    sourceHeight: Int,
    visible: NormRect,
    availableBytes: Long,
    baseLongSide: Int,
): SharpRequest.Region? {
    val pixels = fractionToPixels(expandFraction(visible, REGION_MARGIN), sourceWidth, sourceHeight)
    val width = pixels.right - pixels.left
    val height = pixels.bottom - pixels.top
    val sample = sharpestSample(
        width,
        height,
        availableBytes,
        max(sourceWidth, sourceHeight),
        baseLongSide,
    ) ?: return null
    return SharpRequest.Region(pixels.left, pixels.top, pixels.right, pixels.bottom, sample)
}

private fun fractionToPixels(rect: NormRect, width: Int, height: Int): PixelRect {
    if (width <= 0 || height <= 0) return PixelRect(0, 0, 0, 0)
    val left = (rect.left * width).toInt().coerceIn(0, width)
    val top = (rect.top * height).toInt().coerceIn(0, height)
    val right = (rect.right * width).toInt().coerceIn(0, width)
    val bottom = (rect.bottom * height).toInt().coerceIn(0, height)
    return PixelRect(left, top, max(right, left), max(bottom, top))
}

private fun regionContains(
    region: SharpRequest.Region,
    visible: NormRect,
    sourceWidth: Int,
    sourceHeight: Int,
): Boolean {
    if (sourceWidth <= 0 || sourceHeight <= 0) return false
    val left = region.left.toFloat() / sourceWidth
    val top = region.top.toFloat() / sourceHeight
    val right = region.right.toFloat() / sourceWidth
    val bottom = region.bottom.toFloat() / sourceHeight
    return visible.left >= left - REGION_COVERS_SLOP &&
        visible.top >= top - REGION_COVERS_SLOP &&
        visible.right <= right + REGION_COVERS_SLOP &&
        visible.bottom <= bottom + REGION_COVERS_SLOP
}

private data class PixelRect(val left: Int, val top: Int, val right: Int, val bottom: Int)
