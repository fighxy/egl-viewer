package app.eglviewer.bridge

/** Wire shapes mirrored from library-core ItemCell / LibraryInfo JSON (camelCase). */
data class LibraryInfoDto(
    val kind: String,
    val id: String?,
    val name: String,
    val path: String?,
    val readOnly: Boolean,
)

data class ItemCellDto(
    val id: String,
    val name: String,
    val ext: String,
    val width: Int?,
    val height: Int?,
    val star: Int,
    val hasThumbnail: Boolean,
    val thumbVersion: Long,
    val tags: List<String>,
    val pending: Boolean = false,
)

data class ViewWindowDto(
    val total: Int,
    val start: Int,
    val items: List<ItemCellDto>,
)

data class ItemDto(
    val id: String,
    val name: String,
    val ext: String,
    val star: Int,
    val tags: List<String>,
    val folders: List<String>,
    val annotation: String,
    val isDeleted: Boolean,
)

data class FolderDto(
    val id: String,
    val name: String,
    val children: List<FolderDto> = emptyList(),
    val imageCount: Int? = null,
    val isLocked: Boolean = false,
)

data class TagDto(
    val name: String,
    val imageCount: Int? = null,
)

data class RecentLibrary(
    val name: String,
    val path: String,
    val locationHint: String,
    val missing: Boolean,
    val uri: String? = null,
)
