package app.eglviewer.browse

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import app.eglviewer.core.ItemCell
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Thumbnails decoded for the grid, kept in memory by cost (D-094). The core hands WebP bytes
 * from `thumbs/`; [BitmapFactory] decodes them off the main thread at the cell's pixel size.
 */
class ThumbnailCache(
    private val session: ViewSession,
    budgetBytes: Int = 64 * 1024 * 1024,
) {
    private val cache = object : LruCache<String, Bitmap>(budgetBytes) {
        override fun sizeOf(key: String, value: Bitmap): Int = value.byteCount
    }

    suspend fun image(forCell: ItemCell, pixels: Int): Bitmap? {
        if (!forCell.hasThumbnail) return null
        val key = "${forCell.id}#${forCell.thumbVersion}#$pixels"
        cache.get(key)?.let { return it }
        val bitmap = withContext(Dispatchers.IO) {
            val data = runCatching { session.thumbnailBytes(forCell.id) }.getOrNull() ?: return@withContext null
            decode(data, pixels)
        } ?: return null
        cache.put(key, bitmap)
        return bitmap
    }

    fun removeAll() {
        cache.evictAll()
    }

    companion object {
        fun decode(data: ByteArray, pixels: Int): Bitmap? {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(data, 0, data.size, bounds)
            val maxSide = maxOf(bounds.outWidth, bounds.outHeight, 1)
            val sample = maxOf(1, maxSide / maxOf(pixels, 1))
            val opts = BitmapFactory.Options().apply { inSampleSize = sample }
            return BitmapFactory.decodeByteArray(data, 0, data.size, opts)
        }
    }
}
