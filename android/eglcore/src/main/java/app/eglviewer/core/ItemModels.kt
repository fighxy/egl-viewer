package app.eglviewer.core

import org.json.JSONArray
import org.json.JSONObject

/** Full item record for details / share (Android A3 ≈ D-095). */
data class Item(
    val id: String,
    val name: String,
    val ext: String = "",
    val width: Int? = null,
    val height: Int? = null,
    val size: Int? = null,
    val star: Int = 0,
    val tags: List<String> = emptyList(),
    val folders: List<String> = emptyList(),
    val annotation: String = "",
    val url: String = "",
    val addedAt: Long? = null,
    val fileCreatedAt: Long? = null,
    val fileModifiedAt: Long? = null,
    val duration: Double? = null,
    val noThumbnail: Boolean = false,
    val thumbVersion: Long = 0,
    val isDeleted: Boolean = false,
    val deletedAt: Long? = null,
) {
    companion object {
        fun fromJson(json: String): Item = fromJson(JSONObject(json))

        fun fromJson(o: JSONObject): Item {
            val tags = o.optJSONArray("tags") ?: JSONArray()
            val folders = o.optJSONArray("folders") ?: JSONArray()
            return Item(
                id = o.getString("id"),
                name = o.getString("name"),
                ext = o.optString("ext"),
                width = o.optIntOrNull("width"),
                height = o.optIntOrNull("height"),
                size = o.optIntOrNull("size"),
                star = o.optInt("star", 0),
                tags = (0 until tags.length()).map { tags.getString(it) },
                folders = (0 until folders.length()).map { folders.getString(it) },
                annotation = o.optString("annotation"),
                url = o.optString("url"),
                addedAt = o.optLongOrNull("addedAt"),
                fileCreatedAt = o.optLongOrNull("fileCreatedAt"),
                fileModifiedAt = o.optLongOrNull("fileModifiedAt"),
                duration = if (o.has("duration") && !o.isNull("duration")) o.getDouble("duration") else null,
                noThumbnail = o.optBoolean("noThumbnail", false),
                thumbVersion = o.optLong("thumbVersion", 0),
                isDeleted = o.optBoolean("isDeleted", false),
                deletedAt = o.optLongOrNull("deletedAt"),
            )
        }
    }
}

/**
 * One undoable edit of an item or selection. Wire JSON matches the desktop / iOS
 * `ItemEdit` (`{"type":…,"value":…}`).
 */
sealed class ItemEdit {
    data class AddTags(val value: List<String>) : ItemEdit()
    data class RemoveTags(val value: List<String>) : ItemEdit()
    data class AddToFolders(val value: List<String>) : ItemEdit()
    data class RemoveFromFolders(val value: List<String>) : ItemEdit()
    data class SetFolders(val value: List<String>) : ItemEdit()
    data class SetStar(val value: Int) : ItemEdit()
    data class SetAnnotation(val value: String) : ItemEdit()
    data class SetUrl(val value: String) : ItemEdit()
    data class Rename(val value: String) : ItemEdit()
    data object Trash : ItemEdit()
    data object Restore : ItemEdit()

    fun toJson(): JSONObject = JSONObject().also { o ->
        when (this) {
            is AddTags -> {
                o.put("type", "addTags")
                o.put("value", JSONArray(value))
            }
            is RemoveTags -> {
                o.put("type", "removeTags")
                o.put("value", JSONArray(value))
            }
            is AddToFolders -> {
                o.put("type", "addToFolders")
                o.put("value", JSONArray(value))
            }
            is RemoveFromFolders -> {
                o.put("type", "removeFromFolders")
                o.put("value", JSONArray(value))
            }
            is SetFolders -> {
                o.put("type", "setFolders")
                o.put("value", JSONArray(value))
            }
            is SetStar -> {
                o.put("type", "setStar")
                o.put("value", value)
            }
            is SetAnnotation -> {
                o.put("type", "setAnnotation")
                o.put("value", value)
            }
            is SetUrl -> {
                o.put("type", "setUrl")
                o.put("value", value)
            }
            is Rename -> {
                o.put("type", "rename")
                o.put("value", value)
            }
            Trash -> o.put("type", "trash")
            Restore -> o.put("type", "restore")
        }
    }
}

/** Tags of a selection with how many selected items carry each. */
data class SelectionTags(
    val items: Int,
    val tags: List<Tag>,
    val size: Int,
) {
    companion object {
        fun fromJson(json: String): SelectionTags {
            val o = JSONObject(json)
            val arr = o.optJSONArray("tags") ?: JSONArray()
            return SelectionTags(
                items = o.optInt("items", 0),
                tags = (0 until arr.length()).map {
                    val t = arr.getJSONObject(it)
                    Tag(name = t.getString("name"), imageCount = t.optIntOrNull("imageCount"))
                },
                size = o.optInt("size", 0),
            )
        }
    }
}

data class UndoStatus(val kind: String, val items: Int) {
    companion object {
        fun fromJson(json: String?): UndoStatus? {
            if (json.isNullOrBlank() || json == "null") return null
            val o = JSONObject(json)
            if (o == JSONObject.NULL || !o.has("kind")) return null
            return UndoStatus(kind = o.getString("kind"), items = o.optInt("items", 0))
        }
    }
}

data class TrashInfo(val items: Int, val bytes: Int) {
    companion object {
        fun fromJson(json: String): TrashInfo {
            val o = JSONObject(json)
            return TrashInfo(items = o.optInt("items", 0), bytes = o.optInt("bytes", 0))
        }
    }
}

internal fun JSONObject.optLongOrNull(key: String): Long? =
    if (has(key) && !isNull(key)) getLong(key) else null
