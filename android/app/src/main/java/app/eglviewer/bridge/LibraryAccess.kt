package app.eglviewer.bridge

import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * Persisted recent `.eglib` libraries via SAF tree/document URIs (D-085).
 * Rust still needs a filesystem path; we resolve primary-storage tree URIs and
 * fall back to app-specific external files for create.
 */
class LibraryAccess(context: Context) {
    private val appContext = context.applicationContext
    private val prefs: SharedPreferences =
        appContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun loadRecents(): List<RecentLibrary> {
        val raw = prefs.getString(KEY_RECENTS, null) ?: return emptyList()
        return runCatching {
            val arr = JSONArray(raw)
            (0 until arr.length()).map { index ->
                val o = arr.getJSONObject(index)
                val path = o.optString("path")
                val uri = o.optString("uri")
                val missing = when {
                    path.isNotEmpty() -> !File(path).exists()
                    uri.isNotEmpty() -> false
                    else -> true
                }
                RecentLibrary(
                    name = o.getString("name"),
                    path = path.ifEmpty { uri },
                    locationHint = o.optString("locationHint", "Storage"),
                    missing = missing,
                    uri = uri.ifEmpty { null },
                )
            }
        }.getOrDefault(emptyList())
    }

    fun remember(name: String, path: String, locationHint: String, uri: Uri?) {
        val list = loadRecents().toMutableList()
        list.removeAll { it.path == path || (uri != null && it.uri == uri.toString()) }
        list.add(
            0,
            RecentLibrary(
                name = name.removeSuffix(".eglib").ifEmpty { name },
                path = path,
                locationHint = locationHint,
                missing = false,
                uri = uri?.toString(),
            ),
        )
        while (list.size > 12) {
            list.removeAt(list.lastIndex)
        }
        persist(list)
        if (uri != null) {
            takePersistablePermission(uri)
        }
    }

    fun forget(pathOrUri: String) {
        val list = loadRecents().filterNot { it.path == pathOrUri || it.uri == pathOrUri }
        persist(list)
    }

    fun takePersistablePermission(uri: Uri) {
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        runCatching {
            appContext.contentResolver.takePersistableUriPermission(uri, flags)
        }
    }

    fun resolveFilesystemPath(uri: Uri): String? {
        return when {
            "file".equals(uri.scheme, ignoreCase = true) -> uri.path
            DocumentsContract.isTreeUri(uri) -> pathFromTreeUri(uri)
            DocumentsContract.isDocumentUri(appContext, uri) -> pathFromDocumentUri(uri)
            else -> null
        }?.takeIf { File(it).exists() }
    }

    fun defaultCreateParent(): File {
        val base = appContext.getExternalFilesDir(null) ?: appContext.filesDir
        val parent = File(base, "libraries")
        if (!parent.exists()) {
            parent.mkdirs()
        }
        return parent
    }

    fun locationHint(uri: Uri?, path: String?): String {
        val probe = (path ?: uri?.toString().orEmpty()).lowercase()
        return when {
            probe.contains("cloud") || probe.contains("onedrive") || probe.contains("dropbox") -> "Cloud"
            probe.contains("emulated/0") || probe.startsWith("/storage/") -> "Internal storage"
            uri != null -> "Files"
            else -> "App storage"
        }
    }

    fun isSyncFolder(hint: String): Boolean =
        hint.contains("cloud", ignoreCase = true)

    private fun persist(list: List<RecentLibrary>) {
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
        prefs.edit().putString(KEY_RECENTS, arr.toString()).apply()
    }

    private fun pathFromTreeUri(treeUri: Uri): String? {
        val docId = DocumentsContract.getTreeDocumentId(treeUri)
        return pathFromDocId(docId)
    }

    private fun pathFromDocumentUri(uri: Uri): String? {
        val docId = DocumentsContract.getDocumentId(uri)
        return pathFromDocId(docId)
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
        private const val PREFS = "egl_library_access"
        private const val KEY_RECENTS = "recents_v1"
    }
}
