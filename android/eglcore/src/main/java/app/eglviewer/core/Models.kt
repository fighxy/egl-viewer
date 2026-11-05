package app.eglviewer.core

import org.json.JSONArray
import org.json.JSONObject

/** Wire shapes of the Rust core, camelCase as on the desktop / Swift EglCore. */

enum class LibraryKind {
    NATIVE,
    EAGLE_API,
    EAGLE_FOLDER,
    ;

    companion object {
        fun fromWire(raw: String): LibraryKind =
            when (raw) {
                "eagleApi" -> EAGLE_API
                "eagleFolder" -> EAGLE_FOLDER
                else -> NATIVE
            }
    }
}

enum class ReadOnlyReason {
    OTHER_INSTANCE,
    SYNC_FOLDER,
    READ_ONLY_MEDIUM,
    ;

    companion object {
        fun fromWire(raw: String?): ReadOnlyReason? =
            when (raw) {
                "otherInstance" -> OTHER_INSTANCE
                "syncFolder" -> SYNC_FOLDER
                "readOnlyMedium" -> READ_ONLY_MEDIUM
                else -> null
            }
    }
}

data class LibraryInfo(
    val kind: LibraryKind,
    val id: String?,
    val name: String,
    val path: String?,
    val readOnly: Boolean,
    val readOnlyReason: ReadOnlyReason? = null,
) {
    companion object {
        fun fromJson(json: String): LibraryInfo {
            val o = JSONObject(json)
            return LibraryInfo(
                kind = LibraryKind.fromWire(o.optString("kind", "native")),
                id = o.optString("id").ifEmpty { null },
                name = o.getString("name"),
                path = o.optString("path").ifEmpty { null },
                readOnly = o.optBoolean("readOnly", false),
                readOnlyReason = ReadOnlyReason.fromWire(
                    if (o.has("readOnlyReason") && !o.isNull("readOnlyReason")) {
                        o.getString("readOnlyReason")
                    } else {
                        null
                    },
                ),
            )
        }
    }
}

data class LibraryCounts(
    val all: Int,
    val uncategorized: Int,
    val untagged: Int,
    val trash: Int,
) {
    companion object {
        fun fromJson(json: String): LibraryCounts {
            val o = JSONObject(json)
            return LibraryCounts(
                all = o.optInt("all", 0),
                uncategorized = o.optInt("uncategorized", 0),
                untagged = o.optInt("untagged", 0),
                trash = o.optInt("trash", 0),
            )
        }
    }
}

/**
 * What the core tells the app while it works.
 * A2 parses [ViewsInvalidated] view ids so browse models can refresh windows.
 * A4 parses typed [ImportProgress] for the duplicate question and the banner.
 */
sealed class LibraryEvent {
    data class ImportProgress(val progress: app.eglviewer.core.ImportProgress) : LibraryEvent()
    data class ExportProgress(val raw: JSONObject) : LibraryEvent()
    data class ViewsInvalidated(val views: List<ViewOpened>) : LibraryEvent()
    data class Unknown(val type: String, val raw: JSONObject) : LibraryEvent()

    companion object {
        fun fromJson(json: String): LibraryEvent? =
            runCatching {
                val o = JSONObject(json)
                when (o.optString("type")) {
                    "importProgress" -> {
                        val progress = o.optJSONObject("progress")
                            ?: return@runCatching Unknown("importProgress", o)
                        ImportProgress(app.eglviewer.core.ImportProgress.fromJson(progress))
                    }
                    "exportProgress" -> ExportProgress(o)
                    "viewsInvalidated" -> {
                        val arr = o.optJSONArray("views") ?: JSONArray()
                        val views = (0 until arr.length()).map {
                            ViewOpened.fromJson(arr.getJSONObject(it))
                        }
                        ViewsInvalidated(views)
                    }
                    else -> Unknown(o.optString("type"), o)
                }
            }.getOrNull()
    }
}
