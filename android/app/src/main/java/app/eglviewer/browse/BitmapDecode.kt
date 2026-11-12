package app.eglviewer.browse

/**
 * How [android.graphics.BitmapFactory] should decode an original for the preview.
 *
 * [sampleSize] is the power-of-two [android.graphics.BitmapFactory.Options.inSampleSize].
 * When the subsampled long side is still larger than the screen, [targetLongSide]
 * is the screen: the decoder scales to it (and a later exact scale covers a
 * decoder that ignored the density). Zero means the subsampled bitmap is
 * already at or under the screen and must not be enlarged.
 */
internal data class BitmapDecode(
    val sampleSize: Int,
    val targetLongSide: Int,
)

internal fun bitmapDecode(width: Int, height: Int, maxPixels: Int): BitmapDecode {
    val pixels = maxPixels.coerceAtLeast(1)
    if (width <= 0 || height <= 0) return BitmapDecode(sampleSize = 1, targetLongSide = 0)
    val sample = sampleSize(width, height, pixels)
    val longSide = maxOf(width, height) / sample
    val target = if (longSide > pixels) pixels else 0
    return BitmapDecode(sampleSize = sample, targetLongSide = target)
}

/** Power of two so the long side falls under 2× [maxPixels], never zero. */
internal fun sampleSize(width: Int, height: Int, maxPixels: Int): Int {
    if (width <= 0 || height <= 0 || maxPixels <= 0) return 1
    var sample = 1
    var longSide = maxOf(width, height)
    while (longSide / 2 >= maxPixels) {
        sample *= 2
        longSide /= 2
    }
    return sample
}

/**
 * Pixel size after subsampling and the target-size scale. The long side is at
 * most [maxPixels]; a photo already smaller than the screen stays its own size.
 */
internal fun scaledSize(width: Int, height: Int, maxPixels: Int): Pair<Int, Int> {
    if (width <= 0 || height <= 0) return 0 to 0
    val request = bitmapDecode(width, height, maxPixels)
    var w = (width / request.sampleSize).coerceAtLeast(1)
    var h = (height / request.sampleSize).coerceAtLeast(1)
    val target = request.targetLongSide
    if (target > 0) {
        val longSide = maxOf(w, h)
        if (longSide > target) {
            val scale = target.toFloat() / longSide.toFloat()
            w = maxOf(1, (w * scale).toInt())
            h = maxOf(1, (h * scale).toInt())
        }
    }
    return w to h
}
