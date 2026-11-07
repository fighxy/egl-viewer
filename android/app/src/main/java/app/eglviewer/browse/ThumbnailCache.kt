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
        if (!shouldLoad(forCell)) return null
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
        /**
         * Prefer the cell flag; also try when a positive version is present so a stale
         * `hasThumbnail=false` after generation still paints (D-120).
         */
        fun shouldLoad(forCell: ItemCell): Boolean =
            forCell.hasThumbnail || forCell.thumbVersion > 0

        fun decode(data: ByteArray, pixels: Int): Bitmap? {
            if (data.isEmpty()) return null
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(data, 0, data.size, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
                // Bounds probe failed (odd WebP); still try a full decode.
                return BitmapFactory.decodeByteArray(data, 0, data.size)
            }
            val maxSide = maxOf(bounds.outWidth, bounds.outHeight)
            val sample = maxOf(1, maxSide / maxOf(pixels, 1))
            val opts = BitmapFactory.Options().apply { inSampleSize = sample }
            return BitmapFactory.decodeByteArray(data, 0, data.size, opts)
                ?: BitmapFactory.decodeByteArray(data, 0, data.size)
        }
    }
}
