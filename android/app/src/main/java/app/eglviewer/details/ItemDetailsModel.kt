package app.eglviewer.details

import app.eglviewer.browse.BrowseHost
import app.eglviewer.core.Item
import app.eglviewer.core.ItemEdit
import app.eglviewer.core.Tag
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * One item's details and its edits (A3 ≈ D-095): name, rating, tags, folders, notes and link.
 * Every edit goes through the core as one undoable step.
 */
class ItemDetailsModel(
    val open: BrowseHost,
    val id: String,
) {
    private val _item = MutableStateFlow<Item?>(null)
    val item: StateFlow<Item?> = _item.asStateFlow()

    private val _vocabulary = MutableStateFlow<List<Tag>>(emptyList())
    val vocabulary: StateFlow<List<Tag>> = _vocabulary.asStateFlow()

    val readOnly: Boolean get() = open.readOnly

    suspend fun load() {
        _item.value = open.perform { it.item(id) }
        _vocabulary.value = open.perform { it.tags() }
            ?.sortedBy { it.name.lowercase() }
            .orEmpty()
    }

    suspend fun rename(name: String) {
        val trimmed = name.trim()
        if (trimmed.isEmpty() || trimmed == _item.value?.name) return
        apply(ItemEdit.Rename(trimmed))
    }

    /** Tapping the current rating again clears it. */
    suspend fun rate(stars: Int) {
        val current = _item.value?.star ?: 0
        apply(ItemEdit.SetStar(if (current == stars) 0 else stars))
    }

    suspend fun addTag(name: String) {
        val item = _item.value ?: return
        if (name in item.tags) return
        apply(ItemEdit.AddTags(listOf(name)))
    }

    suspend fun removeTag(name: String) {
        apply(ItemEdit.RemoveTags(listOf(name)))
    }

    suspend fun toggleFolder(folder: String) {
        val item = _item.value ?: return
        apply(
            if (folder in item.folders) {
                ItemEdit.RemoveFromFolders(listOf(folder))
            } else {
                ItemEdit.AddToFolders(listOf(folder))
            },
        )
    }

    suspend fun setNotes(text: String) {
        if (text == _item.value?.annotation) return
        apply(ItemEdit.SetAnnotation(text))
    }

    suspend fun setLink(text: String) {
        val trimmed = text.trim()
        if (trimmed == _item.value?.url) return
        apply(ItemEdit.SetUrl(trimmed))
    }

    val folderNames: List<Pair<String, String>>
        get() {
            val item = _item.value ?: return emptyList()
            return open.pickableFolders
                .filter { it.folder.id in item.folders }
                .map { it.folder.id to it.folder.name }
        }

    private suspend fun apply(edit: ItemEdit) {
        if (readOnly) return
        val updated = open.perform { it.applyItemEdit(id, edit) } ?: return
        _item.value = updated
    }
}
