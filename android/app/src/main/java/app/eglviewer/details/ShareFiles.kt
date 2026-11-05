package app.eglviewer.details

import app.eglviewer.browse.ViewSession
import app.eglviewer.core.Item
import java.io.File
import java.util.UUID

/**
 * Originals handed to the system share sheet under their item names (A3 ≈ D-095). Copies go
 * into a temporary folder; the library's own files are never moved. The folder is removed when
 * the share completes.
 */
class ShareFiles(
    val folder: File,
    val files: List<File>,
) {
    companion object {
        /** At most this many originals go to one share sheet. */
        const val LIMIT = 100

        /** Prepares the originals of [ids] (blocking: run off the main thread). */
        fun prepare(ids: List<String>, from: ViewSession, cacheDir: File): ShareFiles {
            val folder = File(cacheDir, "share-${UUID.randomUUID()}").also { it.mkdirs() }
            val used = mutableSetOf<String>()
            val files = mutableListOf<File>()
            try {
                for (id in ids.take(LIMIT)) {
                    val item = from.item(id)
                    val target = File(folder, unique(fileName(item), used))
                    val path = from.originalPath(id)
                    if (path != null) {
                        File(path).copyTo(target, overwrite = false)
                    } else {
                        val bytes = from.originalBytes(id) ?: continue
                        target.writeBytes(bytes)
                    }
                    files += target
                }
            } catch (error: Exception) {
                folder.deleteRecursively()
                throw error
            }
            return ShareFiles(folder = folder, files = files)
        }

        /** `name.ext`, with characters a file name cannot hold replaced. */
        fun fileName(item: Item): String {
            val name = item.name
                .replace('/', '-')
                .replace(':', '-')
                .trim()
            val base = name.ifEmpty { item.id }
            return if (item.ext.isEmpty()) base else "$base.${item.ext}"
        }

        /** `name (2).ext` when `name.ext` is taken already. */
        fun unique(name: String, used: MutableSet<String>): String {
            var candidate = name
            var n = 2
            val dot = name.lastIndexOf('.')
            val ext = if (dot > 0) name.substring(dot + 1) else ""
            val base = if (dot > 0) name.substring(0, dot) else name
            while (used.contains(candidate.lowercase())) {
                candidate = if (ext.isEmpty()) "$base ($n)" else "$base ($n).$ext"
                n += 1
            }
            used.add(candidate.lowercase())
            return candidate
        }
    }

    fun clean() {
        folder.deleteRecursively()
    }
}
