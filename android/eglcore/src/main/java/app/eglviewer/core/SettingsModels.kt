package app.eglviewer.core

import org.json.JSONArray
import org.json.JSONObject

/** Kind of a `backups/` database copy (Android A7 ≈ D-099). */
enum class BackupKind {
    DAILY,
    MANUAL,
    BEFORE_MIGRATION,
    BEFORE_RESTORE,
    ;

    companion object {
        fun fromWire(raw: String?): BackupKind? =
            when (raw) {
                "daily" -> DAILY
                "manual" -> MANUAL
                "beforeMigration" -> BEFORE_MIGRATION
                "beforeRestore" -> BEFORE_RESTORE
                else -> null
            }
    }
}

/** A copy of `library.db` in `backups/`; restoring names it by [name]. */
data class Backup(
    val name: String,
    val kind: BackupKind?,
    val createdAt: Long,
    val size: Long,
) {
    companion object {
        fun fromJson(o: JSONObject): Backup =
            Backup(
                name = o.getString("name"),
                kind = BackupKind.fromWire(
                    if (o.has("kind") && !o.isNull("kind")) o.getString("kind") else null,
                ),
                createdAt = o.optLong("createdAt", 0L),
                size = o.optLong("size", 0L),
            )

        fun listFromJson(json: String): List<Backup> {
            val arr = JSONArray(json)
            return (0 until arr.length()).map { fromJson(arr.getJSONObject(it)) }
        }
    }
}

/** Bytes on disk, for Library Settings. */
data class LibraryStats(
    val originals: Long,
    val thumbnails: Long,
    val database: Long,
    val backups: Long,
) {
    val total: Long get() = originals + thumbnails + database + backups

    companion object {
        fun fromJson(json: String): LibraryStats {
            val o = JSONObject(json)
            return LibraryStats(
                originals = o.optLong("originals", 0L),
                thumbnails = o.optLong("thumbnails", 0L),
                database = o.optLong("database", 0L),
                backups = o.optLong("backups", 0L),
            )
        }
    }
}

data class MissingOriginal(
    val id: String,
    val name: String,
    val inTrash: Boolean,
) {
    companion object {
        fun fromJson(o: JSONObject): MissingOriginal =
            MissingOriginal(
                id = o.optString("id"),
                name = o.optString("name"),
                inTrash = o.optBoolean("inTrash", false),
            )
    }
}

/** What Check Library found. */
data class LibraryCheck(
    val items: Int,
    val databaseOk: Boolean,
    val databaseProblems: List<String>,
    val missingCount: Int,
    val missing: List<MissingOriginal>,
    val orphanCount: Int,
    val orphanBytes: Long,
    val orphans: List<String>,
    val orphanThumbnails: Int,
) {
    val isClean: Boolean
        get() = databaseOk && missingCount == 0 && orphanCount == 0 && orphanThumbnails == 0

    companion object {
        fun fromJson(json: String): LibraryCheck {
            val o = JSONObject(json)
            val problems = o.optJSONArray("databaseProblems") ?: JSONArray()
            val missingArr = o.optJSONArray("missing") ?: JSONArray()
            val orphansArr = o.optJSONArray("orphans") ?: JSONArray()
            return LibraryCheck(
                items = o.optInt("items", 0),
                databaseOk = o.optBoolean("databaseOk", false),
                databaseProblems = (0 until problems.length()).map { problems.getString(it) },
                missingCount = o.optInt("missingCount", 0),
                missing = (0 until missingArr.length()).map {
                    MissingOriginal.fromJson(missingArr.getJSONObject(it))
                },
                orphanCount = o.optInt("orphanCount", 0),
                orphanBytes = o.optLong("orphanBytes", 0L),
                orphans = (0 until orphansArr.length()).map { orphansArr.getString(it) },
                orphanThumbnails = o.optInt("orphanThumbnails", 0),
            )
        }
    }
}

/** What Repair removes after a check. */
data class RepairOptions(
    val orphans: Boolean,
    val missing: Boolean,
) {
    fun toJson(): JSONObject =
        JSONObject()
            .put("orphans", orphans)
            .put("missing", missing)
}

/** A background task making thumbnails; [id] is null when one already runs. */
data class PreviewsTicket(
    val id: Long?,
    val waiting: Int,
) {
    companion object {
        fun fromJson(json: String): PreviewsTicket {
            val o = JSONObject(json)
            val id = if (o.has("id") && !o.isNull("id")) o.getLong("id") else null
            return PreviewsTicket(id = id, waiting = o.optInt("waiting", 0))
        }
    }
}

/** What an export covers: the whole library, some folders, some tags, or a mix. */
data class ExportSelection(
    val whole: Boolean = false,
    val folders: List<String> = emptyList(),
    val tags: List<String> = emptyList(),
) {
    val isEmpty: Boolean get() = !whole && folders.isEmpty() && tags.isEmpty()

    fun toJson(): JSONObject =
        JSONObject()
            .put("whole", whole)
            .put("folders", JSONArray(folders))
            .put("tags", JSONArray(tags))
}

enum class ExportCompression(val wire: String) {
    NONE("none"),
    FAST("fast"),
    BALANCED("balanced"),
    SMALLEST("smallest"),
    ;

    companion object {
        fun fromWire(raw: String): ExportCompression =
            entries.firstOrNull { it.wire == raw } ?: BALANCED
    }
}

/** A folder tree that mirrors the library's folders, or one ZIP archive of the same tree. */
sealed class ExportFormat {
    data object Mirror : ExportFormat()
    data class Zip(val compression: ExportCompression) : ExportFormat()

    fun toJson(): JSONObject =
        when (this) {
            Mirror -> JSONObject().put("kind", "mirror")
            is Zip -> JSONObject()
                .put("kind", "zip")
                .put("compression", compression.wire)
        }
}

/**
 * One export into a folder or an archive; Eagle is not a destination on the phone.
 * [encrypt] is an age passphrase protecting a ZIP only — never stored.
 */
data class ExportRequest(
    val selection: ExportSelection,
    val includeSubfolders: Boolean,
    val format: ExportFormat,
    val destination: String,
    val encrypt: String? = null,
) {
    fun toJson(): JSONObject {
        val o = JSONObject()
            .put("selection", selection.toJson())
            .put("includeSubfolders", includeSubfolders)
            .put("format", format.toJson())
            .put("destination", destination)
        if (encrypt != null) {
            o.put("encrypt", encrypt)
        } else {
            o.put("encrypt", JSONObject.NULL)
        }
        return o
    }
}

/** What an export would write, before it starts. */
data class ExportPreview(
    val items: Int,
    val bytes: Long,
    /** Items a closed locked folder hides; they are not exported. */
    val hidden: Int,
) {
    companion object {
        fun fromJson(json: String): ExportPreview {
            val o = JSONObject(json)
            return ExportPreview(
                items = o.optInt("items", 0),
                bytes = o.optLong("bytes", 0L),
                hidden = o.optInt("hidden", 0),
            )
        }
    }
}

enum class ExportState {
    PREPARING,
    COPYING,
    ENCRYPTING,
    DONE,
    FAILED,
    CANCELLED,
    ;

    companion object {
        fun fromWire(raw: String): ExportState =
            when (raw) {
                "preparing" -> PREPARING
                "encrypting" -> ENCRYPTING
                "done" -> DONE
                "failed" -> FAILED
                "cancelled" -> CANCELLED
                else -> COPYING
            }
    }
}

data class ExportSummary(
    val items: Int,
    val files: Int,
    val bytes: Long,
    val missing: List<String>,
    val path: String,
) {
    companion object {
        fun fromJson(o: JSONObject): ExportSummary {
            val missingArr = o.optJSONArray("missing") ?: JSONArray()
            return ExportSummary(
                items = o.optInt("items", 0),
                files = o.optInt("files", 0),
                bytes = o.optLong("bytes", 0L),
                missing = (0 until missingArr.length()).map { missingArr.getString(it) },
                path = o.optString("path"),
            )
        }
    }
}

data class ExportProgress(
    val id: Long,
    val state: ExportState,
    val total: Int,
    val done: Int,
    val bytesTotal: Long,
    val bytesDone: Long,
    val current: String?,
    val summary: ExportSummary?,
    val error: String?,
) {
    val isFinished: Boolean
        get() = state == ExportState.DONE ||
            state == ExportState.FAILED ||
            state == ExportState.CANCELLED

    companion object {
        fun fromJson(json: String): ExportProgress = fromJson(JSONObject(json))

        fun fromJson(o: JSONObject): ExportProgress =
            ExportProgress(
                id = o.getLong("id"),
                state = ExportState.fromWire(o.optString("state", "copying")),
                total = o.optInt("total", 0),
                done = o.optInt("done", 0),
                bytesTotal = o.optLong("bytesTotal", 0L),
                bytesDone = o.optLong("bytesDone", 0L),
                current = o.optString("current").ifEmpty { null },
                summary = o.optJSONObject("summary")?.let { ExportSummary.fromJson(it) },
                error = o.optString("error").ifEmpty { null },
            )

        fun listFromJson(json: String): List<ExportProgress> {
            val arr = JSONArray(json)
            return (0 until arr.length()).map { fromJson(arr.getJSONObject(it)) }
        }
    }
}
