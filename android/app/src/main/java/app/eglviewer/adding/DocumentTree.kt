package app.eglviewer.adding

/**
 * A child in a SAF document tree (or a host-test stand-in). Directories are walked;
 * everything else is a file to import — including documents whose MIME type is empty.
 * [DocumentFile.isFile] returns false for an empty MIME and used to skip those files (D-120).
 */
data class TreeChild(
    val name: String,
    val isDirectory: Boolean,
    /** Provider MIME; unused for the take/skip decision, kept for naming helpers. */
    val mimeType: String? = null,
    val children: List<TreeChild> = emptyList(),
)

/**
 * Depth-first relative paths of every non-directory under [root]'s children, including
 * nested folders. Blank names, "." and ".." are skipped (D-109). No MIME filter: JPEG, PNG,
 * HEIC, WebP and other images are taken the same way as on iOS and the desktop.
 */
fun collectTreeFiles(root: TreeChild): List<String> {
    require(root.isDirectory) { "root must be a directory" }
    val out = mutableListOf<String>()
    walk(root.children, prefix = "", out = out)
    return out
}

private fun walk(children: List<TreeChild>, prefix: String, out: MutableList<String>) {
    for (child in children) {
        val name = PickedFile.safeName(child.name, fallback = "")
        if (name.isEmpty()) continue
        val rel = if (prefix.isEmpty()) name else "$prefix/$name"
        if (child.isDirectory) {
            walk(child.children, rel, out)
        } else {
            out += rel
        }
    }
}

/**
 * Extension for a content MIME when the display name has none. Covers the image types the
 * core previews (D-015 / D-104), including ones [MimeTypeMap] often misses (HEIC, WebP).
 */
fun extensionForMime(mime: String?): String? {
    if (mime.isNullOrBlank()) return null
    val lower = mime.lowercase()
    if (lower == "vnd.android.document/directory") return null
    return when (lower) {
        "image/jpeg", "image/jpg" -> "jpg"
        "image/png" -> "png"
        "image/gif" -> "gif"
        "image/webp" -> "webp"
        "image/heic", "image/heif", "image/heic-sequence", "image/heif-sequence" -> "heic"
        "image/avif" -> "avif"
        "image/tiff" -> "tiff"
        "image/bmp", "image/x-bmp" -> "bmp"
        "image/svg+xml" -> "svg"
        "video/mp4" -> "mp4"
        "video/quicktime" -> "mov"
        "video/webm" -> "webm"
        else -> null
    }
}

/** [name] with an extension from [mime] when it has none worth keeping. */
fun fileNameWithMime(name: String, mime: String?): String {
    val safe = PickedFile.safeName(name, fallback = "Imported")
    if (safe.contains('.')) return safe
    val ext = extensionForMime(mime) ?: return safe
    return "$safe.$ext"
}
