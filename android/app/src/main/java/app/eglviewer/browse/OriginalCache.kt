package app.eglviewer.browse

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Decodes originals for the preview at screen size (A3 ≈ D-095): a stored original is read by
 * path, an unlocked-folder item through the core's decrypted bytes. A few stay in memory.
 */
class OriginalCache(private val session: ViewSession) {
    private val cache = object : LruCache<String, Bitmap>(6) {}

    suspend fun image(forId: String, version: Long, pixels: Int): Bitmap? {
        val key = "$forId#$version#$pixels"
        cache.get(key)?.let { return it }
        val bitmap = withContext(Dispatchers.IO) {
            val opts = BitmapFactory.Options().apply {
                inJustDecodeBounds = true
            }
            val path = runCatching { session.originalPath(forId) }.getOrNull()
            val bytes = if (path == null) {
                runCatching { session.originalBytes(forId) }.getOrNull()
            } else {
                null
            }
            if (path != null) {
                BitmapFactory.decodeFile(path, opts)
            } else if (bytes != null) {
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
            } else {
                return@withContext null
            }
            val sample = sampleSize(opts.outWidth, opts.outHeight, pixels)
            val decode = BitmapFactory.Options().apply { inSampleSize = sample }
            when {
                path != null && File(path).isFile ->
                    BitmapFactory.decodeFile(path, decode)
                bytes != null ->
                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, decode)
                else -> null
            }
        } ?: return null
        cache.put(key, bitmap)
        return bitmap
    }

    fun removeAll() {
        cache.evictAll()
    }

    private fun sampleSize(width: Int, height: Int, maxPixels: Int): Int {
        if (width <= 0 || height <= 0 || maxPixels <= 0) return 1
        var sample = 1
        var w = width
        var h = height
        while (maxOf(w, h) / 2 >= maxPixels) {
            sample *= 2
            w /= 2
            h /= 2
        }
        return sample
    }
}
