package app.eglviewer.core

import android.annotation.TargetApi
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.os.Build
import java.io.File
import java.nio.ByteBuffer
import kotlin.math.max
import kotlin.math.roundToInt

/**
 * HEIC and other formats this Android build of the core cannot read itself (no libheif,
 * D-101 / D-104), decoded by [ImageDecoder] for the core's thumbnails — mirror of iOS
 * ImageIO (D-096).
 *
 * Install once at launch. The callback runs on import threads; ImageDecoder is thread-safe
 * per source.
 */
object PlatformDecoder {
    /** Pixels handed to the core are at most this many on the long side. */
    const val MAX_PIXELS = 2048

    data class Image(
        val width: Int,
        val height: Int,
        val pixelWidth: Int,
        val pixelHeight: Int,
        val rgba: ByteArray,
    ) {
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is Image) return false
            return width == other.width &&
                height == other.height &&
                pixelWidth == other.pixelWidth &&
                pixelHeight == other.pixelHeight &&
                rgba.contentEquals(other.rgba)
        }

        override fun hashCode(): Int {
            var result = width
            result = 31 * result + height
            result = 31 * result + pixelWidth
            result = 31 * result + pixelHeight
            result = 31 * result + rgba.contentHashCode()
            return result
        }
    }

    /** Lends ImageDecoder to the core for the whole process. */
    @JvmStatic
    fun install() {
        if (!EglNative.isAvailable) return
        EglNative.setPlatformDecoder(true)
    }

    @JvmStatic
    fun uninstall() {
        if (!EglNative.isAvailable) return
        EglNative.setPlatformDecoder(false)
    }

    /**
     * Decodes [bytes] with orientation applied; null when the platform cannot read them.
     * HEIC needs API 28+ ImageDecoder (and device support).
     */
    @JvmStatic
    fun decode(bytes: ByteArray): Image? {
        if (bytes.isEmpty()) return null
        if (Build.VERSION.SDK_INT < 28) return null
        return runCatching { decodeWithImageDecoder(bytes) }.getOrNull()
    }

    /**
     * Called from JNI on an import thread. Returns 1 after [EglNative.decodeSinkPut], else 0.
     */
    @JvmStatic
    fun decodeFromJni(bytes: ByteArray, ext: String, sinkPtr: Long): Int {
        if (sinkPtr == 0L) return 0
        val image = decode(bytes) ?: return 0
        EglNative.decodeSinkPut(
            sinkPtr,
            image.width,
            image.height,
            image.pixelWidth,
            image.pixelHeight,
            image.rgba,
        )
        return 1
    }

    @TargetApi(28)
    private fun decodeWithImageDecoder(bytes: ByteArray): Image? {
        var displayedWidth = 0
        var displayedHeight = 0
        val decoded: Bitmap = if (Build.VERSION.SDK_INT >= 31) {
            val source = ImageDecoder.createSource(ByteBuffer.wrap(bytes))
            ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                displayedWidth = info.size.width
                displayedHeight = info.size.height
                configure(decoder, displayedWidth, displayedHeight)
            }
        } else {
            val tmp = File.createTempFile("egl-decode-", ".bin")
            try {
                tmp.writeBytes(bytes)
                val source = ImageDecoder.createSource(tmp)
                ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                    displayedWidth = info.size.width
                    displayedHeight = info.size.height
                    configure(decoder, displayedWidth, displayedHeight)
                }
            } finally {
                tmp.delete()
            }
        }
        val software =
            if (decoded.config == Bitmap.Config.ARGB_8888) {
                decoded
            } else {
                val copy = decoded.copy(Bitmap.Config.ARGB_8888, false) ?: return null
                if (copy !== decoded) decoded.recycle()
                copy
            }
        val pixelWidth = software.width
        val pixelHeight = software.height
        if (displayedWidth <= 0 || displayedHeight <= 0) {
            displayedWidth = pixelWidth
            displayedHeight = pixelHeight
        }
        val rgba = rgbaBytes(software)
        software.recycle()
        return Image(
            width = displayedWidth,
            height = displayedHeight,
            pixelWidth = pixelWidth,
            pixelHeight = pixelHeight,
            rgba = rgba,
        )
    }

    @TargetApi(28)
    private fun configure(decoder: ImageDecoder, width: Int, height: Int) {
        decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
        decoder.isMutableRequired = false
        val longSide = max(width, height)
        if (longSide > MAX_PIXELS) {
            val scale = MAX_PIXELS.toFloat() / longSide.toFloat()
            decoder.setTargetSize(
                max(1, (width * scale).roundToInt()),
                max(1, (height * scale).roundToInt()),
            )
        }
    }

    private fun rgbaBytes(bitmap: Bitmap): ByteArray {
        val w = bitmap.width
        val h = bitmap.height
        val pixels = IntArray(w * h)
        bitmap.getPixels(pixels, 0, w, 0, 0, w, h)
        val rgba = ByteArray(w * h * 4)
        var o = 0
        for (c in pixels) {
            rgba[o++] = ((c ushr 16) and 0xff).toByte()
            rgba[o++] = ((c ushr 8) and 0xff).toByte()
            rgba[o++] = (c and 0xff).toByte()
            rgba[o++] = ((c ushr 24) and 0xff).toByte()
        }
        return rgba
    }
}
