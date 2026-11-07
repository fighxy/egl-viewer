package app.eglviewer.adding

import android.content.ClipboardManager
import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import androidx.documentfile.provider.DocumentFile
import java.io.File
import java.io.FileOutputStream
import java.util.UUID

/**
 * A file handed over by the gallery, a drop or the clipboard, copied into a folder of its
 * own under the app cache (D-104 ≈ D-096). The core copies it into the library; the folder
 * goes when the import ends. Leftovers from a kill are removed at launch.
 */
class PickedFile(val file: File) {
    fun remove() {
        runCatching { file.parentFile?.deleteRecursively() }
    }

    companion object {
        const val FOLDER_PREFIX = "picked-"

        fun newFolder(cacheDir: File): File =
            File(cacheDir, "$FOLDER_PREFIX${UUID.randomUUID()}").also { it.mkdirs() }

        /**
         * A name another app gave (a provider's DISPLAY_NAME, a document in a picked tree,
         * a clipboard suggestion), safe as one path segment: its last segment only, never
         * "." or "..", no NUL, at most 255 bytes, [fallback] when nothing is left. A name
         * like "../../shared_prefs/x.xml" would otherwise write outside the copy's folder
         * (D-109).
         */
        fun safeName(raw: String?, fallback: String = "Imported"): String {
            var name = raw.orEmpty().replace('\u0000', ' ')
            name = name.substringAfterLast('/').substringAfterLast('\\').trim()
            if (name == "." || name == "..") name = ""
            while (name.toByteArray(Charsets.UTF_8).size > 255) {
                // Whole code points, so an emoji is never cut in half.
                name = name.substring(0, name.offsetByCodePoints(name.length, -1))
            }
            return name.ifEmpty { fallback }
        }

        /** [name] made safe inside [folder]; refuses anything that would leave it. */
        fun child(folder: File, name: String?, fallback: String = "Imported"): File {
            val target = File(folder, safeName(name, fallback))
            val base = folder.canonicalFile
            require(target.canonicalFile.parentFile == base) { "unsafe file name: $name" }
            return target
        }

        /** Copies [source] under its own name ([fallback] when it has none worth keeping). */
        fun copying(cacheDir: File, source: File, fallback: String? = null): PickedFile {
            val folder = newFolder(cacheDir)
            val target = child(folder, fallback ?: source.name)
            source.copyTo(target, overwrite = true)
            return PickedFile(target)
        }

        fun writing(cacheDir: File, bytes: ByteArray, named: String): PickedFile {
            val folder = newFolder(cacheDir)
            val target = child(folder, named)
            target.writeBytes(bytes)
            return PickedFile(target)
        }

        /** Copies left over by a crash or a kill: removed at launch. */
        fun removeLeftovers(cacheDir: File) {
            cacheDir.listFiles()
                ?.filter { it.isDirectory && it.name.startsWith(FOLDER_PREFIX) }
                ?.forEach { runCatching { it.deleteRecursively() } }
        }
    }
}

/**
 * Materialises content / tree URIs into filesystem paths the core can read.
 *
 * - Gallery / paste / drops: always [PickedFile] temp copies.
 * - Files via SAF: when [LibraryLocations]-style primary paths resolve, use them and keep
 *   the URI grant until import ends; otherwise copy into `picked-*` while the grant is held.
 */
object ImportPaths {
    data class Batch(
        val paths: List<String>,
        val cleanup: () -> Unit,
    )

    fun fromPicked(files: List<PickedFile>): Batch =
        Batch(paths = files.map { it.file.absolutePath }) {
            files.forEach { it.remove() }
        }

    fun fromContentUris(
        context: Context,
        uris: List<Uri>,
        cacheDir: File,
        resolvePath: (Uri) -> String?,
        takeGrant: (Uri) -> Unit = {},
        releaseGrant: (Uri) -> Unit = {},
    ): Batch {
        val paths = mutableListOf<String>()
        val picked = mutableListOf<PickedFile>()
        val granted = mutableListOf<Uri>()
        for (uri in uris) {
            takeGrant(uri)
            granted += uri
            val resolved = resolvePath(uri)
            if (resolved != null && File(resolved).exists()) {
                paths += resolved
            } else {
                val name = displayName(context, uri) ?: "Imported"
                val copied = copyUri(context, uri, cacheDir, name) ?: continue
                picked += copied
                paths += copied.file.absolutePath
            }
        }
        return Batch(paths = paths) {
            picked.forEach { it.remove() }
            granted.forEach { releaseGrant(it) }
        }
    }

    /**
     * A folder from [OpenDocumentTree]: every file under it, including nested folders (D-121).
     * When [resolvePath] yields a directory the app can read ([canReadPath]), that path is
     * handed to the core (its own recursive walk); otherwise the tree is copied into
     * `picked-*` while the SAF grant is held. Grants are released when the import ends.
     */
    fun fromTreeUri(
        context: Context,
        tree: Uri,
        cacheDir: File,
        takeGrant: (Uri) -> Unit = {},
        releaseGrant: (Uri) -> Unit = {},
        resolvePath: (Uri) -> String? = { null },
        canReadPath: (File) -> Boolean = { false },
    ): Batch {
        takeGrant(tree)
        val resolved = resolvePath(tree)?.let { File(it) }
        if (resolved != null && resolved.isDirectory && canReadPath(resolved)) {
            return Batch(paths = listOf(resolved.absolutePath)) { releaseGrant(tree) }
        }
        val root = DocumentFile.fromTreeUri(context, tree)
            ?: return Batch(emptyList()) { releaseGrant(tree) }
        val folder = PickedFile.newFolder(cacheDir)
        val copiedRoot = PickedFile.child(folder, root.name, fallback = "Folder")
        copiedRoot.mkdirs()
        copyDocumentTree(context, root, copiedRoot)
        val picked = PickedFile(copiedRoot)
        return Batch(paths = listOf(copiedRoot.absolutePath)) {
            picked.remove()
            releaseGrant(tree)
        }
    }

    fun clipboardFiles(context: Context, cacheDir: File): List<PickedFile> {
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
            ?: return emptyList()
        val clip = clipboard.primaryClip ?: return emptyList()
        val out = mutableListOf<PickedFile>()
        for (i in 0 until clip.itemCount) {
            val item = clip.getItemAt(i)
            val uri = item.uri ?: continue
            val name = displayName(context, uri) ?: clipboardFileName(null, guessExt(context, uri), i)
            copyUri(context, uri, cacheDir, name)?.let { out += it }
        }
        return out
    }

    fun clipboardHasSomething(context: Context): Boolean {
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
            ?: return false
        if (!clipboard.hasPrimaryClip()) return false
        val clip = clipboard.primaryClip ?: return false
        for (i in 0 until clip.itemCount) {
            if (clip.getItemAt(i).uri != null) return true
        }
        val desc = clipboard.primaryClipDescription ?: return false
        for (i in 0 until desc.mimeTypeCount) {
            val mime = desc.getMimeType(i)
            if (mime.startsWith("image/") || mime.startsWith("video/") ||
                mime == android.content.ClipDescription.MIMETYPE_TEXT_URILIST
            ) {
                return true
            }
        }
        return false
    }

    /** `name.ext`: suggested name, or Pasted / Pasted 2 …, with the type's extension. */
    fun clipboardFileName(suggested: String?, ext: String?, index: Int): String {
        val base = suggested?.trim()
        val stem = if (!base.isNullOrEmpty()) {
            base.replace('/', '-')
        } else if (index == 0) {
            "Pasted"
        } else {
            "Pasted ${index + 1}"
        }
        if (ext.isNullOrBlank()) return stem
        return if (stem.lowercase().endsWith(".${ext.lowercase()}")) stem else "$stem.$ext"
    }

    private fun copyUri(context: Context, uri: Uri, cacheDir: File, name: String): PickedFile? {
        val folder = PickedFile.newFolder(cacheDir)
        return runCatching {
            val target = PickedFile.child(folder, name)
            context.contentResolver.openInputStream(uri)?.use { input ->
                FileOutputStream(target).use { output -> input.copyTo(output) }
            } ?: return null
            PickedFile(target)
        }.getOrNull()
    }

    /**
     * Copies every document under [from] into [into], recursively through subfolders.
     * Anything that is not a directory is treated as a file: [DocumentFile.isFile] is false
     * when the MIME type is empty, which skipped photos on some providers (D-121).
     */
    private fun copyDocumentTree(context: Context, from: DocumentFile, into: File) {
        for (child in from.listFiles()) {
            val rawName = child.name?.ifEmpty { null } ?: continue
            if (child.isDirectory) {
                val dir = PickedFile.child(into, rawName, fallback = "Folder")
                dir.mkdirs()
                copyDocumentTree(context, child, dir)
            } else {
                // Not isFile(): empty MIME must still be copied (JPEG/PNG/HEIC/WebP, …).
                val name = fileNameWithMime(rawName, child.type)
                val target = PickedFile.child(into, name)
                context.contentResolver.openInputStream(child.uri)?.use { input ->
                    FileOutputStream(target).use { output -> input.copyTo(output) }
                }
            }
        }
    }

    private fun displayName(context: Context, uri: Uri): String? {
        context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (idx >= 0) {
                        val name = cursor.getString(idx)
                        if (!name.isNullOrBlank()) return name
                    }
                }
            }
        return uri.lastPathSegment?.substringAfterLast('/')
    }

    private fun guessExt(context: Context, uri: Uri): String? {
        val mime = context.contentResolver.getType(uri) ?: return null
        return MimeTypeMap.getSingleton().getExtensionFromMimeType(mime)
    }
}
