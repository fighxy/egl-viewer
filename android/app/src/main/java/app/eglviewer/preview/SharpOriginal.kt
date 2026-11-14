package app.eglviewer.preview

import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.unit.IntSize
import app.eglviewer.browse.OriginalCache
import kotlin.coroutines.coroutineContext
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/** The zoom the page is showing, written on the main thread for the sharper decode. */
internal class ZoomFrameState {
    var scale by mutableFloatStateOf(1f)
    var offset by mutableStateOf(Offset.Zero)
    var box by mutableStateOf(IntSize.Zero)
}

/**
 * A sharper bitmap for the zoomed page. [fraction] is the source region the
 * bitmap covers; null means the bitmap is the whole photo.
 */
internal data class SharpPicture(
    val bitmap: Bitmap,
    val fraction: NormRect?,
)

/**
 * While this page is zoomed, decode a sharper original and drop it when the
 * zoom ends or the page leaves the screen. A newer request cancels the one
 * still running. The bitmap is published on the main thread; the decode runs
 * in [OriginalCache].
 */
@Composable
internal fun rememberSharpPicture(
    cellId: String?,
    version: Long,
    base: Bitmap?,
    pixels: Int,
    current: Boolean,
    frame: ZoomFrameState,
    originals: OriginalCache,
): SharpPicture? {
    var picture by remember(cellId, version) { mutableStateOf<SharpPicture?>(null) }
    LaunchedEffect(cellId, version, pixels, current, base != null) {
        picture = null
        if (!current || cellId == null || base == null || pixels <= 0) return@LaunchedEffect
        val bounds = originals.sourceBounds(cellId, version)
        if (!isActive || bounds == null) return@LaunchedEffect
        val (sourceWidth, sourceHeight) = bounds
        var shownKey: String? = null
        var covered: SharpRequest = SharpRequest.None
        val loads = SharpLoads()
        try {
            snapshotFlow {
                selectSharp(
                    SharpContext(
                        zoomed = frame.scale > ZOOMED_EPSILON,
                        current = true,
                        screenPixels = pixels,
                        sourceWidth = sourceWidth,
                        sourceHeight = sourceHeight,
                        baseLongSide = maxOf(base.width, base.height),
                        budgetBytes = originals.budgetBytes,
                        retainedBytes = base.byteCount,
                        boxWidth = frame.box.width.toFloat(),
                        boxHeight = frame.box.height.toFloat(),
                        scale = frame.scale,
                        offsetX = frame.offset.x,
                        offsetY = frame.offset.y,
                        covered = covered,
                    ),
                )
            }
                .distinctUntilChanged()
                .collect { request ->
                    if (request !is SharpRequest.None) covered = request
                    loads.update(request) { next, token ->
                        launch {
                            val key = sharpCacheKey(cellId, version, next)
                            var published = false
                            try {
                                val decoded = loadSharp(originals, cellId, version, next)
                                coroutineContext.ensureActive()
                                if (decoded == null || !loads.isCurrent(token)) {
                                    originals.drop(key)
                                    return@launch
                                }
                                val previous = shownKey
                                shownKey = key
                                published = true
                                picture = SharpPicture(
                                    bitmap = decoded,
                                    fraction = next.fraction(sourceWidth, sourceHeight),
                                )
                                if (previous != null && previous != key) originals.drop(previous)
                            } catch (error: CancellationException) {
                                if (!published) originals.drop(key)
                                throw error
                            }
                        }
                    }
                    if (request is SharpRequest.None) {
                        picture = null
                        shownKey?.let { originals.drop(it) }
                        shownKey = null
                        covered = SharpRequest.None
                    }
                }
        } catch (error: CancellationException) {
            loads.cancel()
            shownKey?.let { originals.drop(it) }
            throw error
        }
    }
    if (!current) return null
    return picture
}

private suspend fun loadSharp(
    originals: OriginalCache,
    id: String,
    version: Long,
    request: SharpRequest,
): Bitmap? = when (request) {
    SharpRequest.None -> null
    is SharpRequest.Full -> originals.image(id, version, request.longSide)
    is SharpRequest.Region -> originals.region(
        id,
        version,
        request.left,
        request.top,
        request.right,
        request.bottom,
        request.sampleSize,
    )
}

private fun sharpCacheKey(id: String, version: Long, request: SharpRequest): String = when (request) {
    SharpRequest.None -> ""
    is SharpRequest.Full -> OriginalCache.cacheKey(id, version, request.longSide)
    is SharpRequest.Region -> OriginalCache.regionKey(
        id,
        version,
        request.left,
        request.top,
        request.right,
        request.bottom,
        request.sampleSize,
    )
}

private fun SharpRequest.fraction(sourceWidth: Int, sourceHeight: Int): NormRect? {
    val region = this as? SharpRequest.Region ?: return null
    if (sourceWidth <= 0 || sourceHeight <= 0) return null
    return NormRect(
        left = region.left.toFloat() / sourceWidth,
        top = region.top.toFloat() / sourceHeight,
        right = region.right.toFloat() / sourceWidth,
        bottom = region.bottom.toFloat() / sourceHeight,
    )
}
