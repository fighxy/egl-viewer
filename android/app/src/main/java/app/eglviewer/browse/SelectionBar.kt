package app.eglviewer.browse

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Undo
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.DeleteForever
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.Tag
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.a11y.StarsText
import app.eglviewer.core.ItemsTarget
import app.eglviewer.core.SelectionTags
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.launch
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics

/** Bottom bar under a selection: rate / tags / folder / share / trash (or restore / delete). */
@Composable
fun SelectionBar(model: BrowseModel) {
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val none = model.selectedCount == 0
    val readOnly = model.open.readOnly
    var tagging by remember { mutableStateOf(false) }
    var filing by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(EglTokens.Sidebar)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            text = stringResource(R.string.browse_selected, model.selectedCount),
            color = EglTokens.Foreground,
            fontSize = 13.sp,
            fontWeight = FontWeight.Medium,
            modifier = Modifier.testTag("browse.selectedCount"),
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (model.place == Place.Trash) {
                IconButton(
                    onClick = { scope.launch { model.restore(model.target) } },
                    enabled = !none && !readOnly,
                    modifier = Modifier.testTag("selection.restore"),
                ) {
                    Icon(Icons.AutoMirrored.Filled.Undo, contentDescription = stringResource(R.string.action_restore))
                }
                IconButton(
                    onClick = { deleting = true },
                    enabled = !none && !readOnly,
                    modifier = Modifier.testTag("selection.delete"),
                ) {
                    Icon(Icons.Default.DeleteForever, contentDescription = stringResource(R.string.action_delete_for_good))
                }
            } else {
                RatingMenu(
                    enabled = !none && !readOnly,
                    modifier = Modifier.testTag("selection.rate"),
                ) { stars ->
                    scope.launch { model.rate(model.target, stars) }
                }
                IconButton(
                    onClick = { tagging = true },
                    enabled = !none && !readOnly,
                    modifier = Modifier.testTag("selection.tags"),
                ) {
                    Icon(Icons.Default.Tag, contentDescription = stringResource(R.string.action_tags))
                }
                IconButton(
                    onClick = { filing = true },
                    enabled = !none && !readOnly,
                    modifier = Modifier.testTag("selection.folder"),
                ) {
                    Icon(Icons.Default.Folder, contentDescription = stringResource(R.string.action_add_to_folder))
                }
                IconButton(
                    onClick = {
                        scope.launch { model.share(model.target, context.cacheDir) }
                    },
                    enabled = !none,
                    modifier = Modifier.testTag("selection.share"),
                ) {
                    Icon(Icons.Default.Share, contentDescription = stringResource(R.string.action_share))
                }
                IconButton(
                    onClick = { scope.launch { model.trash(model.target) } },
                    enabled = !none && !readOnly,
                    modifier = Modifier.testTag("selection.trash"),
                ) {
                    Icon(Icons.Default.Delete, contentDescription = stringResource(R.string.action_trash))
                }
            }
        }
    }

    if (tagging) {
        SelectionTagsSheet(
            model = model,
            target = model.target,
            onDismiss = { tagging = false },
        )
    }
    if (filing) {
        FolderPickerSheet(
            folders = model.open.pickableFolders,
            title = stringResource(R.string.action_add_to_folder),
            onPick = { id ->
                filing = false
                scope.launch { model.addToFolder(model.target, id) }
            },
            onDismiss = { filing = false },
        )
    }
    if (deleting) {
        AlertDialog(
            onDismissRequest = { deleting = false },
            title = { Text(stringResource(R.string.trash_delete_confirm, model.selectedCount)) },
            text = { Text(stringResource(R.string.trash_no_undo)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        deleting = false
                        scope.launch { model.deleteForGood(model.target) }
                    },
                    modifier = Modifier.testTag("selection.deleteConfirmed"),
                ) {
                    Text(stringResource(R.string.action_delete_for_good))
                }
            },
            dismissButton = {
                TextButton(onClick = { deleting = false }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }
}

@Composable
fun RatingMenu(
    enabled: Boolean = true,
    modifier: Modifier = Modifier,
    onRate: (Int) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    IconButton(onClick = { open = true }, enabled = enabled, modifier = modifier) {
        Icon(Icons.Default.Star, contentDescription = stringResource(R.string.action_rate))
    }
    DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
        listOf(5, 4, 3, 2, 1, 0).forEach { stars ->
            DropdownMenuItem(
                text = {
                    if (stars == 0) {
                        Text(stringResource(R.string.rating_none))
                    } else {
                        StarsText(stars = stars)
                    }
                },
                onClick = {
                    open = false
                    onRate(stars)
                },
                modifier = Modifier.testTag("rating.$stars"),
            )
        }
    }
}

@Composable
fun SelectionTagsSheet(
    model: BrowseModel,
    target: ItemsTarget,
    onDismiss: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var tags by remember { mutableStateOf<SelectionTags?>(null) }
    var text by remember { mutableStateOf("") }

    androidx.compose.runtime.LaunchedEffect(target) {
        tags = model.open.perform { it.selectionTags(target) }
        model.loadVocabulary()
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.action_tags)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                TagEntryField(
                    text = text,
                    onText = { text = it },
                    vocabulary = model.vocabulary.map { it.name },
                    taken = tags?.tags?.map { it.name }?.toSet().orEmpty(),
                    onSubmit = { name ->
                        scope.launch {
                            model.addTags(target, listOf(name))
                            tags = model.open.perform { it.selectionTags(target) }
                            text = ""
                        }
                    },
                )
                tags?.tags?.forEach { tag ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text("${tag.name} (${tag.imageCount ?: 0})")
                        TextButton(
                            onClick = {
                                scope.launch {
                                    model.removeTags(target, listOf(tag.name))
                                    tags = model.open.perform { it.selectionTags(target) }
                                }
                            },
                        ) { Text(stringResource(R.string.common_remove)) }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.common_done)) }
        },
    )
}

@Composable
fun FolderPickerSheet(
    folders: List<PickableFolder>,
    title: String,
    chosen: Set<String> = emptySet(),
    onPick: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column {
                if (folders.isEmpty()) {
                    Text(stringResource(R.string.folders_empty), color = EglTokens.MutedForeground)
                } else {
                    folders.forEach { entry ->
                        val isChosen = entry.folder.id in chosen
                        TextButton(
                            onClick = { onPick(entry.folder.id) },
                            modifier = Modifier
                                .padding(start = (entry.depth * 12).dp)
                                .semantics { selected = isChosen },
                        ) {
                            val mark = if (isChosen) " ✓" else ""
                            Text(entry.folder.name + mark)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.common_cancel)) }
        },
    )
}

@Composable
fun TagEntryField(
    text: String,
    onText: (String) -> Unit,
    vocabulary: List<String>,
    taken: Set<String>,
    onSubmit: (String) -> Unit,
) {
    val suggestions = TagSuggestions.matching(text, vocabulary, taken, limit = 6)
    Column {
        androidx.compose.foundation.text.BasicTextField(
            value = text,
            onValueChange = onText,
            singleLine = true,
            modifier = Modifier
                .fillMaxWidth()
                .background(EglTokens.Muted)
                .padding(8.dp),
            decorationBox = { inner ->
                if (text.isEmpty()) {
                    Text(stringResource(R.string.details_add_tag), color = EglTokens.MutedForeground)
                }
                inner()
            },
        )
        suggestions.forEach { name ->
            TextButton(onClick = { onSubmit(name) }) { Text(name) }
        }
        if (text.isNotBlank() && suggestions.none { it.equals(text, ignoreCase = false) }) {
            TextButton(onClick = { onSubmit(text.trim()) }) {
                Text(stringResource(R.string.details_add_tag_named, text.trim()))
            }
        }
    }
}

/** Case-insensitive prefix match; keeps the vocabulary's exact casing. */
object TagSuggestions {
    fun matching(
        query: String,
        vocabulary: List<String>,
        taken: Set<String>,
        limit: Int,
    ): List<String> {
        val q = query.trim()
        if (q.isEmpty()) return emptyList()
        val takenLower = taken.map { it.lowercase() }.toSet()
        return vocabulary
            .asSequence()
            .filter { it.lowercase().startsWith(q.lowercase()) }
            .filter { it.lowercase() !in takenLower }
            .distinctBy { it.lowercase() }
            .take(limit)
            .toList()
    }
}
