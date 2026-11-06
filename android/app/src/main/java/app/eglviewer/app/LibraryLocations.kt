package app.eglviewer.app

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import androidx.core.content.ContextCompat
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * Where libraries live on the phone (D-101 / A1, mirrors iOS D-093):
 * - **App-specific storage** (`files/libraries`): the default for new libraries;
 * - **SAF tree** the user picks, kept with a persistable URI grant;
 * - a path that looks cloud-synced opens **read-only** (D-021).
 */
class LibraryLocations(context: Context) {
    private val appContext = context.applicationContext

    /** Folder new libraries go to. */
    val root: File
        get() {
            val base = appContext.getExternalFilesDir(null) ?: appContext.filesDir
            return File(base, "libraries")
        }

    fun ensureRoot() {
        if (!root.exists()) {
            root.mkdirs()
        }
    }

    /** [name], or `name 2`, `name 3`… when a library of that name is already here (D-116). */
    fun freeName(name: String): String = freeName(root, name)

    /** Libraries already in the default folder, by name. */
    fun localLibraries(): List<File> {
        ensureRoot()
        return root.listFiles()
            ?.filter { it.isDirectory && it.name.endsWith(".eglib") }
            ?.sortedBy { it.name.lowercase() }
            .orEmpty()
    }

    fun isLocal(path: File): Boolean {
        val rootPath = root.canonicalFile.absolutePath
        return path.canonicalFile.absolutePath.startsWith(rootPath)
    }

    /**
     * Heuristic for a synced / cloud place (D-021). Android has no single iCloud flag;
     * provider document ids and path segments that name known sync roots count.
     */
    fun isSynced(path: File?, uri: Uri?): Boolean {
        val probe = buildString {
            if (path != null) append(path.absolutePath)
            append(' ')
            if (uri != null) append(uri.toString())
        }
        return looksSynced(probe)
    }

    fun locationHint(path: File?, uri: Uri?): String {
        if (path != null && isLocal(path)) return "App storage"
        if (isSynced(path, uri)) return "Cloud"
        val probe = (path?.absolutePath ?: uri?.toString().orEmpty()).lowercase()
        return when {
            probe.contains("emulated/0") || probe.startsWith("/storage/") -> "Internal storage"
            uri != null -> "Files"
            else -> "Storage"
        }
    }

    /**
     * The app may read and write [path] itself: its own storage always; elsewhere only with
     * "All files access" (Android 11+) or the storage permission before (D-111).
     */
    fun canAccess(path: File): Boolean = isLocal(path) || hasAllFilesAccess()

    fun hasAllFilesAccess(): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            ContextCompat.checkSelfPermission(
                appContext,
                Manifest.permission.WRITE_EXTERNAL_STORAGE,
            ) == PackageManager.PERMISSION_GRANTED
        }

    /** The path of a picked tree or document, whether or not the app may open it yet. */
    fun resolveFilesystemPath(uri: Uri): String? {
        return when {
            "file".equals(uri.scheme, ignoreCase = true) -> uri.path
            DocumentsContract.isTreeUri(uri) -> pathFromDocId(DocumentsContract.getTreeDocumentId(uri))
            DocumentsContract.isDocumentUri(appContext, uri) ->
                pathFromDocId(DocumentsContract.getDocumentId(uri))
            else -> null
        }
    }

    fun takePersistablePermission(uri: Uri) {
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        runCatching {
            appContext.contentResolver.takePersistableUriPermission(uri, flags)
        }
    }

    private fun pathFromDocId(docId: String): String? {
        val split = docId.split(":", limit = 2)
        if (split.isEmpty()) return null
        val type = split[0]
        val relative = split.getOrNull(1).orEmpty()
        return when {
            type.equals("primary", ignoreCase = true) -> {
                val root = Environment.getExternalStorageDirectory().absolutePath
                if (relative.isEmpty()) root else "$root/$relative"
            }
            type.startsWith("/storage") || type.startsWith("/mnt") -> {
                if (relative.isEmpty()) type else "$type/$relative"
            }
            else -> null
        }
    }

    companion object {
        /** [name], or `name 2`, `name 3`… when `<folder>/<name>.eglib` exists (D-116). */
        fun freeName(folder: File, name: String): String {
            var candidate = name
            var number = 2
            while (File(folder, "$candidate.eglib").exists()) {
                candidate = "$name $number"
                number += 1
            }
            return candidate
        }

        private val SYNC_MARKERS = listOf(
            "cloud",
            "onedrive",
            "dropbox",
            "google/android/apps/docs",
            "com.google.android.apps.docs",
            "com.microsoft.skydrive",
            "com.dropbox",
            "file provider storage",
        )

        /** Pure probe used by [isSynced]; also unit-tested on the host JVM. */
        fun looksSynced(probe: String): Boolean {
            val lower = probe.lowercase()
            return SYNC_MARKERS.any { lower.contains(it) }
        }
    }
}

/** One library the app opened before. */
data class RecentLibrary(
    val name: String,
    val path: String,
    val locationHint: String,
    val uri: String? = null,
    val missing: Boolean = false,
)

/** Recent libraries, newest first, in SharedPreferences (last 10). */
class RecentLibraries(context: Context) {
    private val prefs: SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun load(): List<RecentLibrary> {
        val raw = prefs.getString(KEY, null) ?: return emptyList()
        return runCatching {
            val arr = JSONArray(raw)
            (0 until arr.length()).map { index ->
                val o = arr.getJSONObject(index)
                val path = o.optString("path")
                val uri = o.optString("uri").ifEmpty { null }
                val missing = when {
                    path.isNotEmpty() && !path.startsWith("content:") -> !File(path).exists()
                    else -> false
                }
                RecentLibrary(
                    name = o.getString("name"),
                    path = path,
                    locationHint = o.optString("locationHint", "Storage"),
                    uri = uri,
                    missing = missing,
                )
            }
        }.getOrDefault(emptyList())
    }

    fun remember(library: RecentLibrary): List<RecentLibrary> {
        val list = load().toMutableList()
        list.removeAll { it.path == library.path || (library.uri != null && it.uri == library.uri) }
        list.add(0, library.copy(missing = false))
        while (list.size > KEPT) {
            list.removeAt(list.lastIndex)
        }
        save(list)
        return list
    }

    fun forget(pathOrUri: String): List<RecentLibrary> {
        val list = load().filterNot { it.path == pathOrUri || it.uri == pathOrUri }
        save(list)
        return list
    }

    fun clear() {
        prefs.edit().remove(KEY).apply()
    }

    private fun save(list: List<RecentLibrary>) {
        val arr = JSONArray()
        list.forEach { recent ->
            arr.put(
                JSONObject()
                    .put("name", recent.name)
                    .put("path", recent.path)
                    .put("locationHint", recent.locationHint)
                    .put("uri", recent.uri ?: ""),
            )
        }
        prefs.edit().putString(KEY, arr.toString()).apply()
    }

    companion object {
        private const val PREFS = "egl_library_access"
        private const val KEY = "recents_v2"
        const val KEPT = 10
    }
}
