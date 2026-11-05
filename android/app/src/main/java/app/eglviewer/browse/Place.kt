package app.eglviewer.browse

import app.eglviewer.core.Scope
import app.eglviewer.core.SortKey
import app.eglviewer.core.TagMode
import org.json.JSONArray
import org.json.JSONObject

/** Where the user browses: a fixed place, a folder, a smart folder or a tag (D-094). */
sealed class Place {
    data object All : Place()
    data object Uncategorized : Place()
    data object Untagged : Place()
    data object Trash : Place()
    data class Folder(val id: String, val name: String) : Place()
    data class Smart(val id: String, val name: String) : Place()
    data class Tag(val name: String) : Place()

    val scope: Scope
        get() = when (this) {
            All -> Scope.All
            Uncategorized -> Scope.Unfiled
            Untagged -> Scope.Untagged
            Trash -> Scope.Trash
            is Folder -> Scope.FolderScope(id = id, includeDescendants = true)
            is Smart -> Scope.SmartScope(id = id)
            is Tag -> Scope.All
        }

    /** Tag place always filters by this name. */
    val fixedTag: String?
        get() = (this as? Tag)?.name

    /** Stable key for accessibility and the saved view. */
    val key: String
        get() = when (this) {
            All -> "all"
            Uncategorized -> "uncategorized"
            Untagged -> "untagged"
            Trash -> "trash"
            is Folder -> "folder.$id"
            is Smart -> "smart.$id"
            is Tag -> "tag.$name"
        }

    fun toJson(): JSONObject = JSONObject().also { o ->
        when (this) {
            All -> o.put("kind", "all")
            Uncategorized -> o.put("kind", "uncategorized")
            Untagged -> o.put("kind", "untagged")
            Trash -> o.put("kind", "trash")
            is Folder -> {
                o.put("kind", "folder")
                o.put("id", id)
                o.put("name", name)
            }
            is Smart -> {
                o.put("kind", "smart")
                o.put("id", id)
                o.put("name", name)
            }
            is Tag -> {
                o.put("kind", "tag")
                o.put("name", name)
            }
        }
    }

    companion object {
        fun fromJson(o: JSONObject?): Place? {
            if (o == null) return null
            return when (o.optString("kind")) {
                "uncategorized" -> Uncategorized
                "untagged" -> Untagged
                "trash" -> Trash
                "folder" -> Folder(id = o.getString("id"), name = o.optString("name"))
                "smart" -> Smart(id = o.getString("id"), name = o.optString("name"))
                "tag" -> Tag(name = o.getString("name"))
                "all" -> All
                else -> null
            }
        }
    }
}

enum class BrowseLayout { Grid, List }

enum class ThumbnailSize {
    Small, Medium, Large;

    /** Narrowest column width in dp. */
    val minimumWidthDp: Int
        get() = when (this) {
            Small -> 76
            Medium -> 110
            Large -> 170
        }
}

/** Filters of a view: tags, file type, rating and shape (desktop filter bar). */
data class BrowseFilters(
    val tags: List<String> = emptyList(),
    val tagMode: TagMode = TagMode.Any,
    val ext: String? = null,
    /** null: any; 0: unrated; 1…5: exactly that many stars. */
    val rating: Int? = null,
    val shape: Shape? = null,
) {
    val isActive: Boolean
        get() = tags.isNotEmpty() || ext != null || rating != null || shape != null

    enum class Shape(val wire: String) {
        Square("square"),
        Landscape("landscape"),
        Portrait("portrait"),
        PanoramicLandscape("panoramic-landscape"),
        PanoramicPortrait("panoramic-portrait"),
        ;

        companion object {
            fun fromWire(value: String): Shape? =
                entries.firstOrNull { it.wire == value }
        }
    }

    fun toJson(): JSONObject = JSONObject().also { o ->
        o.put("tags", JSONArray(tags))
        o.put("tagMode", tagMode.wire)
        if (ext != null) o.put("ext", ext) else o.put("ext", JSONObject.NULL)
        if (rating != null) o.put("rating", rating) else o.put("rating", JSONObject.NULL)
        if (shape != null) o.put("shape", shape.wire) else o.put("shape", JSONObject.NULL)
    }

    companion object {
        fun fromJson(o: JSONObject?): BrowseFilters {
            if (o == null) return BrowseFilters()
            val tagsArr = o.optJSONArray("tags") ?: JSONArray()
            val tags = (0 until tagsArr.length()).map { tagsArr.getString(it) }
            return BrowseFilters(
                tags = tags,
                tagMode = TagMode.fromWire(o.optString("tagMode", "any")),
                ext = o.optStringOrNull("ext"),
                rating = if (o.has("rating") && !o.isNull("rating")) o.getInt("rating") else null,
                shape = o.optStringOrNull("shape")?.let(Shape::fromWire),
            )
        }
    }
}

/**
 * What a view remembers between launches: sort/layout/size for the whole library, and the
 * last place with its filters and search (desktop D-078 / iOS D-094).
 */
data class SavedView(
    val place: Place? = null,
    val sort: SortKey = SortKey.Added,
    val ascending: Boolean = false,
    val layout: BrowseLayout = BrowseLayout.Grid,
    val size: ThumbnailSize = ThumbnailSize.Medium,
    val filters: BrowseFilters = BrowseFilters(),
    val search: String = "",
) {
    fun toJson(): JSONObject = JSONObject().also { o ->
        if (place != null) o.put("place", place.toJson()) else o.put("place", JSONObject.NULL)
        o.put("sort", sort.wire)
        o.put("ascending", ascending)
        o.put("layout", layout.name.lowercase())
        o.put("size", size.name.lowercase())
        o.put("filters", filters.toJson())
        o.put("search", search)
    }

    companion object {
        fun fromJson(o: JSONObject?): SavedView {
            if (o == null) return SavedView()
            return SavedView(
                place = Place.fromJson(o.optJSONObject("place")),
                sort = SortKey.fromWire(o.optString("sort", "added")),
                ascending = o.optBoolean("ascending", false),
                layout = when (o.optString("layout", "grid")) {
                    "list" -> BrowseLayout.List
                    else -> BrowseLayout.Grid
                },
                size = when (o.optString("size", "medium")) {
                    "small" -> ThumbnailSize.Small
                    "large" -> ThumbnailSize.Large
                    else -> ThumbnailSize.Medium
                },
                filters = BrowseFilters.fromJson(o.optJSONObject("filters")),
                search = o.optString("search", ""),
            )
        }
    }
}

/** Saved views, one per library path — SharedPreferences on Android (UserDefaults on iOS). */
class SavedViews(
    private val loadAll: () -> Map<String, SavedView>,
    private val storeAll: (Map<String, SavedView>) -> Unit,
) {
    fun load(forLibrary: String): SavedView = loadAll()[forLibrary] ?: SavedView()

    fun save(view: SavedView, forLibrary: String) {
        val next = loadAll().toMutableMap()
        next[forLibrary] = view
        storeAll(next)
    }

    fun clear() {
        storeAll(emptyMap())
    }

    companion object {
        const val PREFS_NAME = "egl.saved.views"
        const val KEY = "egl.saved.views.v1"

        fun sharedPreferences(prefs: android.content.SharedPreferences): SavedViews =
            SavedViews(
                loadAll = {
                    val raw = prefs.getString(KEY, null) ?: return@SavedViews emptyMap()
                    try {
                        val root = JSONObject(raw)
                        root.keys().asSequence().associateWith { SavedView.fromJson(root.optJSONObject(it)) }
                    } catch (_: Exception) {
                        emptyMap()
                    }
                },
                storeAll = { map ->
                    val root = JSONObject()
                    map.forEach { (path, view) -> root.put(path, view.toJson()) }
                    prefs.edit().putString(KEY, root.toString()).apply()
                },
            )

        fun memory(initial: MutableMap<String, SavedView> = mutableMapOf()): SavedViews =
            SavedViews(
                loadAll = { initial.toMap() },
                storeAll = {
                    initial.clear()
                    initial.putAll(it)
                },
            )
    }
}

private fun JSONObject.optStringOrNull(key: String): String? =
    if (has(key) && !isNull(key)) {
        val v = getString(key)
        v.ifEmpty { null }
    } else {
        null
    }
