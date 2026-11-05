package app.eglviewer.core

import org.json.JSONArray
import org.json.JSONObject

/** One position of a view: what a grid cell shows (Android A2 ≈ D-094). */
data class ItemCell(
    val id: String,
    val name: String,
    val ext: String = "",
    val width: Int? = null,
    val height: Int? = null,
    val size: Int? = null,
    val star: Int = 0,
    val palette0: List<Int>? = null,
    val hasThumbnail: Boolean = false,
    val thumbVersion: Long = 0,
    val tags: List<String> = emptyList(),
) {
    companion object {
        fun fromJson(o: JSONObject): ItemCell {
            val tags = o.optJSONArray("tags") ?: JSONArray()
            val palette = o.optJSONArray("palette0")
            return ItemCell(
                id = o.getString("id"),
                name = o.getString("name"),
                ext = o.optString("ext"),
                width = o.optIntOrNull("width"),
                height = o.optIntOrNull("height"),
                size = o.optIntOrNull("size"),
                star = o.optInt("star", 0),
                palette0 = palette?.let { p -> (0 until p.length()).map { p.getInt(it) } },
                hasThumbnail = o.optBoolean("hasThumbnail", false),
                thumbVersion = o.optLong("thumbVersion", 0),
                tags = (0 until tags.length()).map { tags.getString(it) },
            )
        }
    }
}

data class ViewOpened(val viewId: Long, val total: Int) {
    companion object {
        fun fromJson(o: JSONObject): ViewOpened =
            ViewOpened(viewId = o.getLong("viewId"), total = o.getInt("total"))
    }
}

data class Folder(
    val id: String,
    val name: String,
    val description: String = "",
    val iconColor: String? = null,
    val tags: List<String> = emptyList(),
    val children: List<Folder> = emptyList(),
    val imageCount: Int? = null,
    val descendantImageCount: Int? = null,
    val isLocked: Boolean = false,
    val isUnlocked: Boolean = false,
    val hasPassword: Boolean = false,
    val lockLabel: String? = null,
    val lockColor: String? = null,
    /** The sort the folder opens with; null follows the library's sort. */
    val orderBy: SortKey? = null,
    val sortAscending: Boolean? = null,
) {
    companion object {
        fun fromJson(o: JSONObject): Folder {
            val children = o.optJSONArray("children") ?: JSONArray()
            val tags = o.optJSONArray("tags") ?: JSONArray()
            val order = o.optString("orderBy").ifEmpty { null }
            return Folder(
                id = o.getString("id"),
                name = o.getString("name"),
                description = o.optString("description"),
                iconColor = o.optString("iconColor").ifEmpty { null },
                tags = (0 until tags.length()).map { tags.getString(it) },
                children = (0 until children.length()).map { fromJson(children.getJSONObject(it)) },
                imageCount = o.optIntOrNull("imageCount"),
                descendantImageCount = o.optIntOrNull("descendantImageCount"),
                isLocked = o.optBoolean("isLocked", false),
                isUnlocked = o.optBoolean("isUnlocked", false),
                hasPassword = o.optBoolean("hasPassword", false),
                lockLabel = o.optString("lockLabel").ifEmpty { null },
                lockColor = o.optString("lockColor").ifEmpty { null },
                orderBy = order?.let(SortKey::fromWire),
                sortAscending = if (o.has("sortAscending") && !o.isNull("sortAscending")) {
                    o.getBoolean("sortAscending")
                } else {
                    null
                },
            )
        }

        fun listFromJson(json: String): List<Folder> {
            val arr = JSONArray(json)
            return (0 until arr.length()).map { fromJson(arr.getJSONObject(it)) }
        }
    }
}

data class SmartFolder(
    val id: String,
    val name: String,
    val children: List<SmartFolder> = emptyList(),
    /** Eagle's rule language, as stored. */
    val conditions: JSONValue? = null,
    /** Rules this build cannot evaluate: they never match. */
    val unsupported: List<String> = emptyList(),
) {
    companion object {
        fun fromJson(o: JSONObject): SmartFolder {
            val children = o.optJSONArray("children") ?: JSONArray()
            val unsupported = o.optJSONArray("unsupported") ?: JSONArray()
            return SmartFolder(
                id = o.getString("id"),
                name = o.getString("name"),
                children = (0 until children.length()).map { fromJson(children.getJSONObject(it)) },
                conditions = if (o.has("conditions") && !o.isNull("conditions")) {
                    JSONValue.from(o.get("conditions"))
                } else {
                    null
                },
                unsupported = (0 until unsupported.length()).map { unsupported.getString(it) },
            )
        }

        fun listFromJson(json: String): List<SmartFolder> {
            val arr = JSONArray(json)
            return (0 until arr.length()).map { fromJson(arr.getJSONObject(it)) }
        }
    }
}

data class Tag(val name: String, val imageCount: Int? = null) {
    companion object {
        fun listFromJson(json: String): List<Tag> {
            val arr = JSONArray(json)
            return (0 until arr.length()).map {
                val o = arr.getJSONObject(it)
                Tag(name = o.getString("name"), imageCount = o.optIntOrNull("imageCount"))
            }
        }
    }
}

data class ExtensionCount(val ext: String, val count: Int) {
    companion object {
        fun listFromJson(json: String): List<ExtensionCount> {
            val arr = JSONArray(json)
            return (0 until arr.length()).map {
                val o = arr.getJSONObject(it)
                ExtensionCount(ext = o.getString("ext"), count = o.getInt("count"))
            }
        }
    }
}

sealed class Scope {
    data object All : Scope()
    data object Unfiled : Scope()
    data object Untagged : Scope()
    data object Trash : Scope()
    data class FolderScope(val id: String, val includeDescendants: Boolean = true) : Scope()
    data class SmartScope(val id: String) : Scope()

    fun toJson(): JSONObject = JSONObject().also { o ->
        when (this) {
            All -> o.put("kind", "all")
            Unfiled -> o.put("kind", "unfiled")
            Untagged -> o.put("kind", "untagged")
            Trash -> o.put("kind", "trash")
            is FolderScope -> {
                o.put("kind", "folder")
                o.put("id", id)
                o.put("includeDescendants", includeDescendants)
            }
            is SmartScope -> {
                o.put("kind", "smart")
                o.put("id", id)
            }
        }
    }
}

enum class SortKey(val wire: String) {
    Added("added"),
    Btime("btime"),
    Mtime("mtime"),
    Name("name"),
    Ext("ext"),
    Size("size"),
    Resolution("resolution"),
    Star("star"),
    Random("random"),
    ;

    companion object {
        val offered: List<SortKey> = entries
        fun fromWire(value: String): SortKey =
            entries.firstOrNull { it.wire == value } ?: Added
    }
}

enum class TagMode(val wire: String) {
    Any("any"),
    All("all"),
    None("none"),
    ;

    companion object {
        fun fromWire(value: String): TagMode =
            entries.firstOrNull { it.wire == value } ?: Any
    }
}

data class LibraryQuery(
    val scope: Scope? = null,
    val tags: List<String>? = null,
    val tagMode: TagMode? = null,
    val ext: String? = null,
    val rating: Int? = null,
    val shape: String? = null,
    val text: String? = null,
    val sort: SortKey? = null,
    val ascending: Boolean? = null,
) {
    fun toJson(): JSONObject = JSONObject().also { o ->
        scope?.let { o.put("scope", it.toJson()) }
        tags?.takeIf { it.isNotEmpty() }?.let { o.put("tags", JSONArray(it)) }
        tagMode?.let { o.put("tagMode", it.wire) }
        ext?.let { o.put("ext", it) }
        rating?.let { o.put("rating", it) }
        shape?.let { o.put("shape", it) }
        text?.takeIf { it.isNotEmpty() }?.let { o.put("text", it) }
        sort?.let { o.put("sort", it.wire) }
        ascending?.let { o.put("ascending", it) }
    }
}

/** Selection of a view: listed ids, or with [all] every item except [ids]. */
data class ItemsTarget(
    val viewId: Long? = null,
    val all: Boolean = false,
    val ids: List<String> = emptyList(),
) {
    fun toJson(): JSONObject = JSONObject().also { o ->
        viewId?.let { o.put("viewId", it) }
        o.put("all", all)
        o.put("ids", JSONArray(ids))
    }
}

internal fun JSONObject.optIntOrNull(key: String): Int? =
    if (has(key) && !isNull(key)) getInt(key) else null
