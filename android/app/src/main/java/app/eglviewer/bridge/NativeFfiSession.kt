package app.eglviewer.bridge

import org.json.JSONArray
import org.json.JSONObject

/**
 * Live session over `crates/library-ffi` via the tiny JNI bridge in
 * `app/src/main/cpp/egl_jni.cpp`. Only used when [EglNative.isAvailable] is true.
 */
class NativeFfiSession {
    private var handle: Long = 0L

    fun open(path: String, syncFolder: Boolean) {
        close()
        handle = EglNative.open(path, APP_VERSION, syncFolder)
        if (handle == 0L) {
            throw IllegalStateException(EglNative.lastErrorGlobal().ifEmpty { "open failed" })
        }
    }

    fun close() {
        if (handle != 0L) {
            EglNative.close(handle)
            handle = 0L
        }
    }

    fun libraryInfo(): LibraryInfoDto = parseLibraryInfo(requireJson { EglNative.libraryInfo(handle) })

    fun viewWindow(scopeJson: String, start: Int, len: Int): ViewWindowDto {
        val raw = requireJson { EglNative.viewWindow(handle, scopeJson, start, len) }
        val obj = JSONObject(raw)
        val items = obj.getJSONArray("items")
        return ViewWindowDto(
            total = obj.getInt("total"),
            start = obj.getInt("start"),
            items = (0 until items.length()).map { parseItemCell(items.getJSONObject(it)) },
        )
    }

    fun item(id: String): ItemDto = parseItem(JSONObject(requireJson { EglNative.item(handle, id) }))

    fun folderTree(): List<FolderDto> {
        val arr = JSONArray(requireJson { EglNative.folderTree(handle) })
        return (0 until arr.length()).map { parseFolder(arr.getJSONObject(it)) }
    }

    fun tags(): List<TagDto> {
        val arr = JSONArray(requireJson { EglNative.tags(handle) })
        return (0 until arr.length()).map {
            val o = arr.getJSONObject(it)
            TagDto(
                name = o.getString("name"),
                imageCount = if (o.has("imageCount") && !o.isNull("imageCount")) o.getInt("imageCount") else null,
            )
        }
    }

    fun thumbnailBytes(id: String): ByteArray? = EglNative.thumbnailBytes(handle, id)

    fun applyAddTag(itemId: String, tag: String) {
        val escaped = tag.replace("\\", "\\\\").replace("\"", "\\\"")
        val edit = """{"type":"addTags","value":["$escaped"]}"""
        requireJson { EglNative.applyItemEdit(handle, itemId, edit) }
    }

    private fun requireJson(call: () -> String?): String {
        val json = call()
        if (json.isNullOrEmpty()) {
            throw IllegalStateException(EglNative.lastError(handle).ifEmpty { "FFI call failed" })
        }
        return json
    }

    companion object {
        const val APP_VERSION = "0.2.1-android"

        fun createLibrary(parent: String, name: String): String {
            val path = EglNative.createLibrary(parent, name, APP_VERSION)
            if (path.isNullOrEmpty()) {
                throw IllegalStateException(EglNative.lastErrorGlobal().ifEmpty { "create failed" })
            }
            return path
        }
    }
}

/** JNI façades for `egl_*` (see egl_jni.cpp). */
object EglNative {
    var isAvailable: Boolean = false
        private set

    init {
        try {
            System.loadLibrary("egl_jni")
            isAvailable = true
        } catch (_: UnsatisfiedLinkError) {
            isAvailable = false
        }
    }

    external fun createLibrary(parent: String, name: String, appVersion: String): String?
    external fun open(path: String, appVersion: String, syncFolder: Boolean): Long
    external fun close(handle: Long)
    external fun libraryInfo(handle: Long): String?
    external fun viewWindow(handle: Long, scopeJson: String, start: Int, len: Int): String?
    external fun item(handle: Long, id: String): String?
    external fun folderTree(handle: Long): String?
    external fun tags(handle: Long): String?
    external fun applyItemEdit(handle: Long, id: String, editJson: String): String?
    external fun thumbnailBytes(handle: Long, id: String): ByteArray?
    external fun lastError(handle: Long): String
    external fun lastErrorGlobal(): String
}

internal fun parseLibraryInfo(json: String): LibraryInfoDto {
    val o = JSONObject(json)
    return LibraryInfoDto(
        kind = o.optString("kind", "native"),
        id = o.optString("id").ifEmpty { null },
        name = o.getString("name"),
        path = o.optString("path").ifEmpty { null },
        readOnly = o.optBoolean("readOnly", false),
    )
}

internal fun parseItemCell(o: JSONObject): ItemCellDto {
    val tags = o.optJSONArray("tags") ?: JSONArray()
    return ItemCellDto(
        id = o.getString("id"),
        name = o.getString("name"),
        ext = o.optString("ext"),
        width = o.optIntOrNull("width"),
        height = o.optIntOrNull("height"),
        star = o.optInt("star", 0),
        hasThumbnail = o.optBoolean("hasThumbnail", false),
        thumbVersion = o.optLong("thumbVersion", 0),
        tags = (0 until tags.length()).map { tags.getString(it) },
        pending = o.optBoolean("pending", false),
    )
}

internal fun parseItem(o: JSONObject): ItemDto {
    val tags = o.optJSONArray("tags") ?: JSONArray()
    val folders = o.optJSONArray("folders") ?: JSONArray()
    return ItemDto(
        id = o.getString("id"),
        name = o.getString("name"),
        ext = o.optString("ext"),
        star = o.optInt("star", 0),
        tags = (0 until tags.length()).map { tags.getString(it) },
        folders = (0 until folders.length()).map { folders.getString(it) },
        annotation = o.optString("annotation"),
        isDeleted = o.optBoolean("isDeleted", false),
    )
}

internal fun parseFolder(o: JSONObject): FolderDto {
    val children = o.optJSONArray("children") ?: JSONArray()
    return FolderDto(
        id = o.getString("id"),
        name = o.getString("name"),
        children = (0 until children.length()).map { parseFolder(children.getJSONObject(it)) },
        imageCount = o.optIntOrNull("imageCount"),
        isLocked = o.optBoolean("isLocked", false),
    )
}

private fun JSONObject.optIntOrNull(key: String): Int? =
    if (has(key) && !isNull(key)) getInt(key) else null
