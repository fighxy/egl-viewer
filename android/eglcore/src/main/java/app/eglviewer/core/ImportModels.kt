package app.eglviewer.core

import org.json.JSONArray
import org.json.JSONObject

/** Ticket from `egl_add_files`: progress arrives as events / [Library.importStatus]. */
data class ImportTicket(val id: Long) {
    companion object {
        fun fromJson(json: String): ImportTicket {
            val o = JSONObject(json)
            return ImportTicket(o.getLong("id"))
        }
    }
}

enum class ImportKind {
    FILES,
    PREVIEWS,
    EAGLE,
    ;

    companion object {
        fun fromWire(raw: String): ImportKind =
            when (raw) {
                "previews" -> PREVIEWS
                "eagle" -> EAGLE
                else -> FILES
            }
    }
}

enum class ImportState {
    SCANNING,
    ADDING,
    WAITING_FOR_DECISION,
    PREVIEWS,
    DONE,
    CANCELLED,
    ;

    companion object {
        fun fromWire(raw: String): ImportState =
            when (raw) {
                "scanning" -> SCANNING
                "waitingForDecision" -> WAITING_FOR_DECISION
                "previews" -> PREVIEWS
                "done" -> DONE
                "cancelled" -> CANCELLED
                else -> ADDING
            }
    }
}

/** A file whose content the library already has; the import waits for an answer. */
data class DuplicateNotice(
    val path: String,
    val existing: String,
    val inTrash: Boolean,
    val targetFolder: String?,
) {
    companion object {
        fun fromJson(o: JSONObject): DuplicateNotice =
            DuplicateNotice(
                path = o.optString("path"),
                existing = o.optString("existing"),
                inTrash = o.optBoolean("inTrash", false),
                targetFolder = o.optString("targetFolder").ifEmpty { null },
            )
    }
}

/** First answer holds for the rest of the batch (`egl_answer_import`). */
enum class DuplicateAnswer(val wire: String) {
    SKIP("skip"),
    ADD_ANYWAY("addAnyway"),
    ADD_TO_FOLDER("addToFolder"),
    CANCEL("cancel"),
}

data class ImportProgress(
    val id: Long,
    val kind: ImportKind,
    val state: ImportState,
    val total: Int,
    val processed: Int,
    val added: Int,
    val duplicates: Int,
    val failed: Int,
    val previewsTotal: Int,
    val previewsDone: Int,
    val current: String?,
    val duplicate: DuplicateNotice?,
) {
    val isFinished: Boolean
        get() = state == ImportState.DONE || state == ImportState.CANCELLED

    companion object {
        fun fromJson(json: String): ImportProgress = fromJson(JSONObject(json))

        fun fromJson(o: JSONObject): ImportProgress =
            ImportProgress(
                id = o.getLong("id"),
                kind = ImportKind.fromWire(o.optString("kind", "files")),
                state = ImportState.fromWire(o.optString("state", "adding")),
                total = o.optInt("total", 0),
                processed = o.optInt("processed", 0),
                added = o.optInt("added", 0),
                duplicates = o.optInt("duplicates", 0),
                failed = o.optInt("failed", 0),
                previewsTotal = o.optInt("previewsTotal", 0),
                previewsDone = o.optInt("previewsDone", 0),
                current = o.optString("current").ifEmpty { null },
                duplicate = o.optJSONObject("duplicate")?.let { DuplicateNotice.fromJson(it) },
            )

        fun listFromJson(json: String): List<ImportProgress> {
            val arr = JSONArray(json)
            return (0 until arr.length()).map { fromJson(arr.getJSONObject(it)) }
        }
    }
}
