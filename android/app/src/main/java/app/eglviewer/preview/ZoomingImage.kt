package app.eglviewer.preview

import android.graphics.Bitmap
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.VectorConverter
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculateCentroidSize
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEvent
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Pinch and double-tap zoom over a decoded original (D-129, D-135).
 *
 * The zoom keeps the image point under the fingers. A pan stops at the photo's
 * edges and pulls back past them; a pan well past the left or right edge
 * returns to fit so the next swipe pages. At fit, a one-finger drag is not
 * consumed, so the pager keeps it. [sharp] replaces the whole photo when
 * [sharpFraction] is null, or is drawn over the visible source region.
 */
@Composable
internal fun ZoomingImage(
    bitmap: Bitmap,
    onZoomed: (Boolean) -> Unit,
    onSingleTap: () -> Unit,
    current: Boolean,
    onFrame: (scale: Float, offset: Offset, box: IntSize) -> Unit,
    sharp: Bitmap? = null,
    sharpFraction: NormRect? = null,
    contentDescription: String? = null,
    modifier: Modifier = Modifier,
) {
    val scope = rememberCoroutineScope()
    val controller = remember { ZoomController(scope) }
    val density = LocalDensity.current
    var boxPx by remember { mutableStateOf(IntSize.Zero) }
    val leaveDistance = with(density) { ZoomGeometry.leaveDistance.dp.toPx() }
    controller.image = Size(bitmap.width.toFloat(), bitmap.height.toFloat())
    controller.box = Size(boxPx.width.toFloat(), boxPx.height.toFloat())
    controller.leaveDistance = leaveDistance

    val shown = controller.displayed()
    SideEffect {
        if (!current) return@SideEffect
        onZoomed(shown.scale > ZOOMED_EPSILON)
        onFrame(shown.scale, shown.offset, boxPx)
    }

    val main = if (sharp != null && sharpFraction == null) sharp else bitmap
    Box(
        modifier = modifier
            .fillMaxSize()
            .clipToBounds()
            .onSizeChanged { boxPx = it }
            .pointerInput(controller) {
                detectTapGestures(
                    onTap = { onSingleTap() },
                    onDoubleTap = { point -> controller.onDoubleTap(point) },
                )
            }
            .pointerInput(controller) {
                detectPreviewTransform(
                    zoomed = { controller.displayed().scale > ZOOMED_EPSILON },
                    onPinchStart = controller::beginGesture,
                    onPinch = controller::onPinch,
                    onPinchEnd = controller::onPinchEnd,
                    onCommitPinch = controller::commitPinch,
                    onPanStart = controller::beginGesture,
                    onPan = controller::onPan,
                    onPanEnd = controller::onPanEnd,
                    onCommitPan = controller::commitPan,
                )
            }
            .testTag("preview.zoom"),
        contentAlignment = Alignment.Center,
    ) {
        Image(
            bitmap = main.asImageBitmap(),
            contentDescription = contentDescription,
            contentScale = ContentScale.Fit,
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer {
                    scaleX = shown.scale
                    scaleY = shown.scale
                    translationX = shown.offset.x
                    translationY = shown.offset.y
                },
        )
        val fraction = sharpFraction
        if (sharp != null && fraction != null && boxPx.width > 0 && boxPx.height > 0) {
            val rect = displayRect(fraction, controller.image, controller.box, shown.scale, shown.offset)
            if (rect.width > 1f && rect.height > 1f) {
                Image(
                    bitmap = sharp.asImageBitmap(),
                    contentDescription = null,
                    contentScale = ContentScale.FillBounds,
                    modifier = Modifier
                        .align(Alignment.TopStart)
                        .offset { IntOffset(rect.left.roundToInt(), rect.top.roundToInt()) }
                        .size(
                            with(density) { rect.width.toDp() },
                            with(density) { rect.height.toDp() },
                        ),
                )
            }
        }
    }
}

private class ZoomController(private val scope: CoroutineScope) {
    var image: Size = Size.Zero
    var box: Size = Size.Zero
    var leaveDistance: Float = ZoomGeometry.leaveDistance

    private var scale by mutableFloatStateOf(1f)
    private var offset by mutableStateOf(Offset.Zero)
    private var live by mutableStateOf<ZoomFrame?>(null)
    private var animating by mutableStateOf(false)
    private val animScale = Animatable(1f)
    private val animOffset = Animatable(Offset.Zero, Offset.VectorConverter)
    private var settleJob: Job? = null
    private var panTranslation: Offset = Offset.Zero

    fun displayed(): ZoomFrame = when {
        live != null -> live!!
        animating -> ZoomFrame(animScale.value, animOffset.value)
        else -> ZoomFrame(scale, offset)
    }

    fun beginGesture() {
        settleJob?.cancel()
        val shown = displayed()
        animating = false
        scale = shown.scale
        offset = shown.offset
        live = null
        panTranslation = Offset.Zero
    }

    fun onPinch(focal: Offset, magnification: Float, focalPan: Offset) {
        if (box.width <= 0f || box.height <= 0f) return
        live = pinchZoom(ZoomFrame(scale, offset), focal, box, magnification, focalPan)
    }

    fun onPinchEnd() {
        val current = live ?: ZoomFrame(scale, offset)
        settle(pinchSettled(current, image, box), durationMs = 300)
    }

    /** Caps the pinch and keeps that frame, without the settle animation. */
    fun commitPinch() {
        settleJob?.cancel()
        val current = live ?: ZoomFrame(scale, offset)
        val settled = pinchSettled(current, image, box)
        live = null
        animating = false
        scale = settled.scale
        offset = settled.offset
        panTranslation = Offset.Zero
    }

    fun onPan(translation: Offset) {
        if (box.width <= 0f || box.height <= 0f) return
        panTranslation = translation
        live = panZoom(ZoomFrame(scale, offset), translation, image, box)
    }

    fun onPanEnd() {
        val translation = panTranslation
        panTranslation = Offset.Zero
        settle(
            panSettled(ZoomFrame(scale, offset), translation, image, box, leaveDistance),
            durationMs = 300,
        )
    }

    /** Stops the pan on the edge (or back at fit) and keeps that frame. */
    fun commitPan() {
        settleJob?.cancel()
        val settled = panSettled(ZoomFrame(scale, offset), panTranslation, image, box, leaveDistance)
        panTranslation = Offset.Zero
        live = null
        animating = false
        scale = settled.scale
        offset = settled.offset
    }

    fun onDoubleTap(point: Offset) {
        beginGesture()
        settle(doubleTapZoom(scale, point, image, box), durationMs = 220)
    }

    private fun settle(target: ZoomFrame, durationMs: Int) {
        val from = displayed()
        settleJob?.cancel()
        settleJob = scope.launch {
            animScale.snapTo(from.scale)
            animOffset.snapTo(from.offset)
            live = null
            animating = true
            coroutineScope {
                launch { animScale.animateTo(target.scale, tween(durationMillis = durationMs)) }
                launch { animOffset.animateTo(target.offset, tween(durationMillis = durationMs)) }
            }
            if (!isActive) return@launch
            scale = target.scale
            offset = target.offset
            animating = false
        }
    }
}

private enum class PreviewGesture { Watching, Pinch, Pan }

/**
 * Pinch, or a one-finger pan while zoomed. A one-finger drag at fit is left
 * unconsumed so the pager can take it.
 */
private suspend fun androidx.compose.ui.input.pointer.PointerInputScope.detectPreviewTransform(
    zoomed: () -> Boolean,
    onPinchStart: () -> Unit,
    onPinch: (focal: Offset, magnification: Float, focalPan: Offset) -> Unit,
    onPinchEnd: () -> Unit,
    onCommitPinch: () -> Unit,
    onPanStart: () -> Unit,
    onPan: (translation: Offset) -> Unit,
    onPanEnd: () -> Unit,
    onCommitPan: () -> Unit,
) {
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false)
        val slop = viewConfiguration.touchSlop
        var panOrigin = down.position
        var mode = PreviewGesture.Watching
        var pinchFocal = Offset.Zero
        var pinchSpan = 1f
        // Set only when the gesture ends on its own. Cancellation of the pointer
        // input must not spring the photo: this lambda cannot read the job.
        var settle = false
        try {
            while (true) {
                val event = awaitPointerEvent()
                if (event.changes.any { it.isConsumed }) {
                    settle = true
                    break
                }
                val pressed = event.changes.filter { it.pressed }
                if (pressed.isEmpty()) {
                    settle = true
                    break
                }
                when (mode) {
                    PreviewGesture.Watching -> {
                        val moved = (pressed.first().position - panOrigin).getDistance()
                        if (pressed.size >= 2 && (moved > slop || event.centroidSize() > slop)) {
                            mode = PreviewGesture.Pinch
                            pinchFocal = event.calculateCentroid(useCurrent = true)
                            pinchSpan = event.centroidSize().coerceAtLeast(1f)
                            onPinchStart()
                            onPinch(pinchFocal, 1f, Offset.Zero)
                            event.consumePressed()
                        } else if (moved > slop) {
                            // At fit the pager owns the drag, including one that starts at the edge.
                            if (!zoomed()) {
                                settle = true
                                break
                            }
                            mode = PreviewGesture.Pan
                            onPanStart()
                            onPan(pressed.first().position - panOrigin)
                            event.consumePressed()
                        }
                    }
                    PreviewGesture.Pinch -> {
                        if (pressed.size < 2) {
                            if (zoomed() && pressed.size == 1) {
                                onCommitPinch()
                                mode = PreviewGesture.Pan
                                panOrigin = pressed.first().position
                                onPanStart()
                            } else {
                                settle = true
                                break
                            }
                        } else {
                            val span = event.centroidSize().coerceAtLeast(1f)
                            val centroid = event.calculateCentroid(useCurrent = true)
                            onPinch(pinchFocal, span / pinchSpan, centroid - pinchFocal)
                            event.consumePressed()
                        }
                    }
                    PreviewGesture.Pan -> {
                        if (pressed.size >= 2) {
                            onCommitPan()
                            mode = PreviewGesture.Pinch
                            pinchFocal = event.calculateCentroid(useCurrent = true)
                            pinchSpan = event.centroidSize().coerceAtLeast(1f)
                            onPinchStart()
                            onPinch(pinchFocal, 1f, Offset.Zero)
                        } else {
                            onPan(pressed.first().position - panOrigin)
                        }
                        event.consumePressed()
                    }
                }
            }
        } finally {
            if (settle) {
                when (mode) {
                    PreviewGesture.Pinch -> onPinchEnd()
                    PreviewGesture.Pan -> onPanEnd()
                    PreviewGesture.Watching -> Unit
                }
            }
        }
    }
}

private fun PointerEvent.centroidSize(): Float = calculateCentroidSize(useCurrent = true)

private fun PointerEvent.consumePressed() {
    changes.forEach { change ->
        if (change.pressed && change.positionChanged()) change.consume()
    }
}
