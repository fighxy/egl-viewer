package app.eglviewer.browse

import android.content.ComponentCallbacks2
import android.content.Context
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import java.io.File
import kotlin.coroutines.coroutineContext

/**
 * Decodes originals for the preview at the screen's long side (D-127, was 1.5×
 * in D-103). A stored original is read by path; an unlocked-folder item comes
 * through the core's decrypted bytes.
 *
 * The cache is a byte budget ([PreviewMemoryBudget]), cost the decoded bytes,
 * not a count of bitmaps. At most two decodes run at once. Two callers of one
 * original share one decode, and a caller that goes away cancels it only when
 * nobody else is waiting. A memory trim empties the bitmaps.
 */
class OriginalCache(
    private val session: ViewSession,
    maxMemoryBytes: Long = Runtime.getRuntime().maxMemory(),
    private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
) {
    val budgetBytes: Int = PreviewMemoryBudget.bytes(maxMemoryBytes)

    private val cache = object : LruCache<String, Bitmap>(budgetBytes) {
        override fun sizeOf(key: String, value: Bitmap): Int = value.byteCount.coerceAtLeast(1)
    }

    private val loads = SharedDecodes<Bitmap>(scope, limit = 2) { key ->
        val parsed = decodeKey(key) ?: return@SharedDecodes null
        coroutineContext.ensureActive()
        val bitmap = decode(parsed.id, parsed.pixels) ?: return@SharedDecodes null
        try {
            coroutineContext.ensureActive()
        } catch (error: CancellationException) {
            bitmap.recycle()
            throw error
        }
        cache.put(key, bitmap)
        bitmap
    }

    private var watching: Context? = null
    private val trim = object : ComponentCallbacks2 {
        override fun onTrimMemory(level: Int) {
            if (clearOnMemoryTrim(level)) cache.evictAll()
        }

        override fun onConfigurationChanged(newConfig: Configuration) = Unit

        @Deprecated("Deprecated in Java")
        override fun onLowMemory() {
            cache.evictAll()
        }
    }

    /** The original already in memory, for the first frame of a page. */
    fun cached(id: String, version: Long, pixels: Int): Bitmap? =
        cache.get(cacheKey(id, version, pixels))

    /**
     * The original of [id] at most [pixels] on its long side. Null when the
     * decoder cannot read it. A cancelled caller is cancelled, not given null.
     */
    suspend fun image(id: String, version: Long, pixels: Int): Bitmap? {
        val key = cacheKey(id, version, pixels)
        cache.get(key)?.let { return it }
        return try {
            loads.load(key)
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            null
        }
    }

    /**
     * Warms the cache for a page the user is likely to swipe to. Cancel the
     * job when that page is no longer next.
     */
    fun prefetch(id: String, version: Long, pixels: Int): Job =
        scope.launch { image(id, version, pixels) }

    fun removeAll() {
        loads.cancelAll()
        cache.evictAll()
    }

    /** Drops bitmaps when the system reports memory pressure (D-127). */
    fun watchMemory(context: Context) {
        if (watching != null) return
        val app = context.applicationContext
        app.registerComponentCallbacks(trim)
        watching = app
    }

    /** Cancels decodes and forgets the trim callback. The cache is not reused. */
    fun shutdown() {
        watching?.unregisterComponentCallbacks(trim)
        watching = null
        removeAll()
        scope.cancel()
    }

    private fun decode(id: String, pixels: Int): Bitmap? {
        val path = catchFailure { session.originalPath(id) }
        val bytes = if (path == null) catchFailure { session.originalBytes(id) } else null
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        when {
            path != null -> BitmapFactory.decodeFile(path, bounds)
            bytes != null -> BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            else -> return null
        }
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
        val request = bitmapDecode(bounds.outWidth, bounds.outHeight, pixels)
        val opts = BitmapFactory.Options().apply {
            inSampleSize = request.sampleSize
            if (request.targetLongSide > 0) {
                val subsampled = maxOf(bounds.outWidth, bounds.outHeight) / request.sampleSize
                inScaled = true
                inDensity = subsampled.coerceAtLeast(1)
                inTargetDensity = request.targetLongSide
            } else {
                inScaled = false
            }
        }
        val bitmap = when {
            path != null && File(path).isFile -> BitmapFactory.decodeFile(path, opts)
            bytes != null -> BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
            else -> null
        } ?: return null
        return fitLongSide(bitmap, pixels.coerceAtLeast(1))
    }

    private fun fitLongSide(bitmap: Bitmap, pixels: Int): Bitmap {
        val longSide = maxOf(bitmap.width, bitmap.height)
        if (longSide <= pixels || longSide <= 0) return bitmap
        val scale = pixels.toFloat() / longSide.toFloat()
        val width = maxOf(1, (bitmap.width * scale).toInt())
        val height = maxOf(1, (bitmap.height * scale).toInt())
        val scaled = Bitmap.createScaledBitmap(bitmap, width, height, true)
        if (scaled !== bitmap) bitmap.recycle()
        return scaled
    }

    private data class DecodedKey(val id: String, val pixels: Int)

    private fun decodeKey(key: String): DecodedKey? {
        val mark = key.lastIndexOf('#')
        if (mark <= 0 || mark == key.lastIndex) return null
        val pixels = key.substring(mark + 1).toIntOrNull() ?: return null
        val rest = key.substring(0, mark)
        val versionMark = rest.lastIndexOf('#')
        if (versionMark <= 0) return null
        return DecodedKey(id = rest.substring(0, versionMark), pixels = pixels)
    }

    private inline fun <T> catchFailure(block: () -> T): T? =
        try {
            block()
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            null
        }

    companion object {
        fun cacheKey(id: String, version: Long, pixels: Int): String =
            "$id#$version#${pixels.coerceAtLeast(1)}"

        /**
         * True for a real memory warning. [ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN]
         * is only "another activity is in front", and its numeric value sits
         * above the running-low levels, so a `>=` check would clear the cache
         * every time the user left the app for a moment.
         */
        fun clearOnMemoryTrim(level: Int): Boolean =
            level == ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW ||
                level == ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL ||
                level >= ComponentCallbacks2.TRIM_MEMORY_BACKGROUND
    }
}
