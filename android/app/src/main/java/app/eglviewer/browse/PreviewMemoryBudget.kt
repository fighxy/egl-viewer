package app.eglviewer.browse

/**
 * How many bytes of decoded originals the preview may keep (D-127).
 *
 * iOS uses a fixed 96 MB: a screen-sized photo is tens of megabytes, and a
 * cache counted in entries ran the phone out of memory. Android heaps differ,
 * so the budget is an eighth of the heap the process may use, never above
 * that 96 MB, and at least 8 MB when the heap itself is that large — enough
 * for one screen-sized bitmap on a small device.
 */
object PreviewMemoryBudget {
    /** D-127's cap. */
    const val CAP_BYTES = 96 * 1024 * 1024

    /** One screen-sized photo, when the heap can hold it. */
    const val FLOOR_BYTES = 8 * 1024 * 1024

    fun bytes(maxMemoryBytes: Long): Int {
        if (maxMemoryBytes <= 0L) return 1
        val eighth = maxMemoryBytes / 8
        val sized = if (eighth < FLOOR_BYTES) minOf(FLOOR_BYTES.toLong(), maxMemoryBytes) else eighth
        return minOf(sized, CAP_BYTES.toLong()).toInt().coerceAtLeast(1)
    }
}
