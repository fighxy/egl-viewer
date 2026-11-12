package app.eglviewer.browse

import android.content.ComponentCallbacks2
import android.content.Context
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.BitmapRegionDecoder
import android.graphics.Rect
import android.util.LruCache
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import kotlin.coroutines.coroutineContext
import kotlin.math.sqrt

/** A screen-size original, or one region of it. Parsed from a cache key. */
internal sealed interface OriginalRequest {
    val id: String

    data class Full(override val id: String, val pixels: Int) : OriginalRequest

    data class Region(
        override val id: String,
        val left: Int,
        val top: Int,
        val right: Int,
        val bottom: Int,
        val sampleSize: Int,
    ) : OriginalRequest
}

/**
 * Decodes originals for the preview at the screen's long side (D-127, was 1.5×
 * in D-103). A stored original is read by path; an unlocked-folder item comes
 * through the core's decrypted bytes.
 *
 * The cache is a byte budget ([PreviewMemoryBudget]), cost the decoded bytes,
 * not a count of bitmaps. At most two decodes run at once. Two callers of one
 * original share one decode, and a caller that goes away cancels it only when
 * nobody else is waiting. A memory trim empties the bitmaps. A zoomed page may
 * also ask for a larger long side, or one region of the file; that shares the
 * same budget and the same two decode slots.
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

    private val boundsCache = HashMap<String, Pair<Int, Int>>()

    private val loads = SharedDecodes<Bitmap>(scope, limit = 2) { key ->
        val parsed = parseKey(key) ?: return@SharedDecodes null
        coroutineContext.ensureActive()
        val bitmap = when (parsed) {
            is OriginalRequest.Full -> decode(parsed.id, parsed.pixels)
            is OriginalRequest.Region -> decodeRegion(parsed)
        } ?: return@SharedDecodes null
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

    /**
     * The file's pixel size, read off the main thread. Cached; it is not a bitmap
     * and does not spend the preview budget. Null when the file has no size.
     */
    suspend fun sourceBounds(id: String, version: Long): Pair<Int, Int>? {
        val key = "$id#$version"
        synchronized(boundsCache) { boundsCache[key] }?.let { return it }
        return withContext(Dispatchers.IO) {
            coroutineContext.ensureActive()
            val size = readBounds(id) ?: return@withContext null
            synchronized(boundsCache) { boundsCache[key] = size }
            size
        }
    }

    /**
     * The visible region of an original, for a zoom past the screen-size decode.
     * [sampleSize] is the power-of-two subsample. Null when the decoder cannot
     * read that region. A cancelled caller is cancelled, not given null.
     */
    suspend fun region(
        id: String,
        version: Long,
        left: Int,
        top: Int,
        right: Int,
        bottom: Int,
        sampleSize: Int,
    ): Bitmap? {
        val key = regionKey(id, version, left, top, right, bottom, sampleSize)
        cache.get(key)?.let { return it }
        return try {
            loads.load(key)
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            null
        }
    }

    /** Drops one decoded bitmap. The caller still holding it is responsible for not drawing a recycled one. */
    fun drop(key: String) {
        if (key.isEmpty()) return
        cache.remove(key)
    }

    fun removeAll() {
        loads.cancelAll()
        synchronized(boundsCache) { boundsCache.clear() }
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

    private fun readBounds(id: String): Pair<Int, Int>? {
        val path = catchFailure { session.originalPath(id) }
        val bytes = if (path == null) catchFailure { session.originalBytes(id) } else null
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        when {
            path != null -> BitmapFactory.decodeFile(path, bounds)
            bytes != null -> BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            else -> return null
        }
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
        return bounds.outWidth to bounds.outHeight
    }

    @Suppress("DEPRECATION")
    private fun decodeRegion(request: OriginalRequest.Region): Bitmap? {
        if (request.sampleSize < 1 || request.right <= request.left || request.bottom <= request.top) return null
        val path = catchFailure { session.originalPath(id = request.id) }
        val bytes = if (path == null) catchFailure { session.originalBytes(request.id) } else null
        val decoder = catchFailure {
            when {
                path != null && File(path).isFile -> BitmapRegionDecoder.newInstance(path, false)
                bytes != null -> BitmapRegionDecoder.newInstance(bytes, 0, bytes.size, false)
                else -> null
            }
        } ?: return null
        try {
            val width = decoder.width
            val height = decoder.height
            if (width <= 0 || height <= 0) return null
            val rect = Rect(
                request.left.coerceIn(0, width),
                request.top.coerceIn(0, height),
                request.right.coerceIn(0, width),
                request.bottom.coerceIn(0, height),
            )
            if (rect.width() <= 0 || rect.height() <= 0) return null
            val opts = BitmapFactory.Options().apply {
                inSampleSize = request.sampleSize
                inScaled = false
                inPreferredConfig = Bitmap.Config.ARGB_8888
            }
            val bitmap = catchFailure { decoder.decodeRegion(rect, opts) } ?: return null
            return fitBytes(bitmap, budgetBytes)
        } finally {
            decoder.recycle()
        }
    }

    /** A decoder that ignored the sample size must still fit the preview budget. */
    private fun fitBytes(bitmap: Bitmap, maxBytes: Int): Bitmap {
        val cost = bitmap.byteCount
        if (maxBytes <= 0 || cost <= maxBytes || cost <= 0) return bitmap
        val scale = sqrt(maxBytes.toFloat() / cost.toFloat()).coerceIn(0.01f, 1f)
        val width = maxOf(1, (bitmap.width * scale).toInt())
        val height = maxOf(1, (bitmap.height * scale).toInt())
        if (width == bitmap.width && height == bitmap.height) return bitmap
        val scaled = Bitmap.createScaledBitmap(bitmap, width, height, true)
        if (scaled !== bitmap) bitmap.recycle()
        return scaled
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

        fun regionKey(
            id: String,
            version: Long,
            left: Int,
            top: Int,
            right: Int,
            bottom: Int,
            sampleSize: Int,
        ): String = "$id#$version#r#$left,$top,$right,$bottom,$sampleSize"

        /**
         * [cacheKey] or [regionKey]. The version is only there so two edits of
         * one item do not share a bitmap; the id may itself contain `#`.
         */
        internal fun parseKey(key: String): OriginalRequest? {
            val last = key.lastIndexOf('#')
            if (last <= 0 || last == key.lastIndex) return null
            val tail = key.substring(last + 1)
            val head = key.substring(0, last)
            if (',' in tail) {
                val kind = head.lastIndexOf('#')
                if (kind <= 0 || head.substring(kind + 1) != "r") return null
                val rest = head.substring(0, kind)
                val versionMark = rest.lastIndexOf('#')
                if (versionMark <= 0) return null
                val id = rest.substring(0, versionMark)
                if (id.isEmpty()) return null
                val parts = tail.split(',')
                if (parts.size != 5) return null
                val nums = parts.map { it.toIntOrNull() ?: return null }
                return OriginalRequest.Region(id, nums[0], nums[1], nums[2], nums[3], nums[4])
            }
            val pixels = tail.toIntOrNull() ?: return null
            val versionMark = head.lastIndexOf('#')
            if (versionMark <= 0) return null
            val id = head.substring(0, versionMark)
            if (id.isEmpty()) return null
            return OriginalRequest.Full(id, pixels)
        }

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
