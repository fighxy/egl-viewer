package app.eglviewer.preview

import android.graphics.Bitmap
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.rememberTransformableState
import androidx.compose.foundation.gestures.transformable
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag

/**
 * Pinch / double-tap zoom over a decoded original. While zoomed, [onZoomed] is true so the
 * pager stops paging (A3 ≈ D-095). At fit size, pan is not consumed so [HorizontalPager]
 * keeps horizontal swipes (D-128); the default `transformable` `canPan = { true }` was
 * eating those drags.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun ZoomingImage(
    bitmap: Bitmap,
    onZoomed: (Boolean) -> Unit,
    onSingleTap: () -> Unit,
    contentDescription: String? = null,
    modifier: Modifier = Modifier,
) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }

    fun publishZoomed(next: Float) {
        onZoomed(next > ZOOMED_EPSILON)
    }

    val state = rememberTransformableState { zoomChange, panChange, _ ->
        val next = (scale * zoomChange).coerceIn(1f, 6f)
        scale = next
        offset = if (next <= ZOOMED_EPSILON) Offset.Zero else offset + panChange
        publishZoomed(next)
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .pointerInput(Unit) {
                detectTapGestures(
                    onTap = { onSingleTap() },
                    onDoubleTap = {
                        if (scale > ZOOMED_EPSILON) {
                            scale = 1f
                            offset = Offset.Zero
                            onZoomed(false)
                        } else {
                            scale = 2.5f
                            onZoomed(true)
                        }
                    },
                )
            }
            // Only consume pans while zoomed; otherwise the pager owns horizontal swipes.
            .transformable(
                state = state,
                canPan = { scale > ZOOMED_EPSILON },
            )
            .testTag("preview.zoom"),
        contentAlignment = Alignment.Center,
    ) {
        Image(
            bitmap = bitmap.asImageBitmap(),
            contentDescription = contentDescription,
            contentScale = ContentScale.Fit,
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer {
                    scaleX = scale
                    scaleY = scale
                    translationX = offset.x
                    translationY = offset.y
                },
        )
    }
}

/** Scale above fit counts as zoomed for pager locking and pan consumption (D-128). */
internal const val ZOOMED_EPSILON = 1.01f
