package app.eglviewer.core

import org.json.JSONArray
import org.json.JSONObject

/** The colours a folder or a tag group may be given (`FOLDER_COLORS`); stored as raw names. */
enum class LabelColor(val wire: String) {
    RED("red"),
    ORANGE("orange"),
    YELLOW("yellow"),
    GREEN("green"),
    AQUA("aqua"),
    BLUE("blue"),
    PURPLE("purple"),
    PINK("pink"),
    ;

    companion object {
        val all: List<LabelColor> = entries
        fun fromWire(value: String?): LabelColor? =
            entries.firstOrNull { it.wire == value }
    }
}

/**
 * One edit of the folder tree; the answer is the new tree.
 * Mirrors iOS `FolderChange` / `egl_save_folder` (D-097 / D-105).
 */
sealed class FolderChange {
    /** After the last child of [parent] (`null`: the top level). */
    data class Create(
        val name: String,
        val parent: String? = null,
        val description: String = "",
        val iconColor: String? = null,
    ) : FolderChange()

    /** The folder's own properties; folder tags are labels on the folder, not item tags. */
    data class Update(
        val id: String,
        val name: String,
        val description: String,
        val tags: List<String>,
        val iconColor: String?,
        val orderBy: SortKey? = null,
        val sortAscending: Boolean? = null,
    ) : FolderChange()

    /** Under [parent] (`null`: the top level) at [index] among its other children. */
    data class Move(
        val id: String,
        val parent: String?,
        val index: Int,
    ) : FolderChange()

    fun toJson(): JSONObject = JSONObject().also { o ->
        when (this) {
            is Create -> {
                o.put("type", "create")
                o.put("name", name)
                o.put("parent", parent ?: JSONObject.NULL)
                o.put("description", description)
                o.put("iconColor", iconColor ?: JSONObject.NULL)
            }
            is Update -> {
                o.put("type", "update")
                o.put("id", id)
                o.put("name", name)
                o.put("description", description)
                o.put("tags", JSONArray(tags))
                o.put("iconColor", iconColor ?: JSONObject.NULL)
                o.put("orderBy", orderBy?.wire ?: JSONObject.NULL)
                o.put("sortAscending", sortAscending ?: JSONObject.NULL)
            }
            is Move -> {
                o.put("type", "move")
                o.put("id", id)
                o.put("parent", parent ?: JSONObject.NULL)
                o.put("index", index)
            }
        }
    }
}

/** A named set of tags, for the tag list. */
data class TagGroup(
    val id: String,
    val name: String,
    /** One of [LabelColor], or what an imported library holds. */
    val color: String? = null,
    val tags: List<String> = emptyList(),
) {
    companion object {
        fun fromJson(o: JSONObject): TagGroup {
            val tags = o.optJSONArray("tags") ?: JSONArray()
            return TagGroup(
                id = o.getString("id"),
                name = o.getString("name"),
                color = o.optString("color").ifEmpty { null },
                tags = (0 until tags.length()).map { tags.getString(it) },
            )
        }

        fun listFromJson(json: String): List<TagGroup> {
            val arr = JSONArray(json)
            return (0 until arr.length()).map { fromJson(arr.getJSONObject(it)) }
        }
    }
}

sealed class TagGroupChange {
    data class Create(
        val name: String,
        val color: String? = null,
        val tags: List<String> = emptyList(),
    ) : TagGroupChange()

    data class Update(
        val id: String,
        val name: String,
        val color: String? = null,
        val tags: List<String> = emptyList(),
    ) : TagGroupChange()

    fun toJson(): JSONObject = JSONObject().also { o ->
        when (this) {
            is Create -> {
                o.put("type", "create")
                o.put("name", name)
                o.put("color", color ?: JSONObject.NULL)
                o.put("tags", JSONArray(tags))
            }
            is Update -> {
                o.put("type", "update")
                o.put("id", id)
                o.put("name", name)
                o.put("color", color ?: JSONObject.NULL)
                o.put("tags", JSONArray(tags))
            }
        }
    }
}

/** A smart folder edit; [conditions] is Eagle's array, stored unchanged. */
sealed class SmartFolderChange {
    data class Create(
        val name: String,
        val parent: String? = null,
        val conditions: JSONValue,
    ) : SmartFolderChange()

    data class Update(
        val id: String,
        val name: String,
        val conditions: JSONValue,
    ) : SmartFolderChange()

    fun toJson(): JSONObject = JSONObject().also { o ->
        when (this) {
            is Create -> {
                o.put("type", "create")
                o.put("name", name)
                o.put("parent", parent ?: JSONObject.NULL)
                o.put("conditions", conditions.toJson())
            }
            is Update -> {
                o.put("type", "update")
                o.put("id", id)
                o.put("name", name)
                o.put("conditions", conditions.toJson())
            }
        }
    }
}

/** "Save as smart folder": the rules of a view's query. */
data class SmartRulesDraft(
    val parent: String? = null,
    val conditions: JSONValue,
) {
    companion object {
        fun fromJson(json: String): SmartRulesDraft {
            val o = JSONObject(json)
            return SmartRulesDraft(
                parent = o.optString("parent").ifEmpty { null },
                conditions = JSONValue.from(o.get("conditions")),
            )
        }
    }
}
