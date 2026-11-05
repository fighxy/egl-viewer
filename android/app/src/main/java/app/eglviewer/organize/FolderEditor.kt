package app.eglviewer.organize

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.OpenLibrary
import app.eglviewer.core.Folder
import app.eglviewer.core.FolderChange
import app.eglviewer.core.LabelColor
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.launch

/** What the folder editor works on: a new folder (under [parent]) or an existing one. */
sealed class FolderEditing {
    data class Create(val parent: String?) : FolderEditing()
    data class Edit(val folder: Folder) : FolderEditing()
}

@Composable
fun FolderEditorDialog(
    open: OpenLibrary,
    editing: FolderEditing,
    onDismiss: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    var notes by remember { mutableStateOf("") }
    var color by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(editing) {
        when (editing) {
            is FolderEditing.Create -> {
                name = ""
                notes = ""
                color = null
            }
            is FolderEditing.Edit -> {
                name = editing.folder.name
                notes = editing.folder.description
                color = editing.folder.iconColor
            }
        }
    }

    val title = when (editing) {
        is FolderEditing.Create -> stringResource(R.string.folder_new_title)
        is FolderEditing.Edit -> stringResource(R.string.folder_edit_title)
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.folder_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = notes,
                    onValueChange = { notes = it },
                    label = { Text(stringResource(R.string.folder_description)) },
                    modifier = Modifier.fillMaxWidth(),
                    minLines = 2,
                )
                Text(stringResource(R.string.folder_color), fontSize = 13.sp, color = EglTokens.MutedForeground)
                ColorChoice(color = color, onColor = { color = it })
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    val trimmed = name.trim()
                    if (trimmed.isEmpty()) return@TextButton
                    val change = when (editing) {
                        is FolderEditing.Create -> FolderChange.Create(
                            name = trimmed,
                            parent = editing.parent,
                            description = notes,
                            iconColor = color,
                        )
                        is FolderEditing.Edit -> FolderChange.Update(
                            id = editing.folder.id,
                            name = trimmed,
                            description = notes,
                            tags = editing.folder.tags,
                            iconColor = color,
                            orderBy = editing.folder.orderBy,
                            sortAscending = editing.folder.sortAscending,
                        )
                    }
                    scope.launch {
                        if (open.saveFolder(change)) onDismiss()
                    }
                },
                enabled = name.trim().isNotEmpty(),
            ) {
                Text(stringResource(R.string.common_save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}

/** No colour or one of the eight label colours. */
@Composable
fun ColorChoice(color: String?, onColor: (String?) -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
        ColorSwatch(value = null, selected = color == null, onClick = { onColor(null) })
        LabelColor.all.forEach { label ->
            ColorSwatch(
                value = label.wire,
                selected = color == label.wire,
                onClick = { onColor(label.wire) },
            )
        }
    }
}

@Composable
private fun ColorSwatch(value: String?, selected: Boolean, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .size(28.dp)
            .then(
                if (selected) {
                    Modifier.border(2.dp, EglTokens.Foreground, CircleShape).padding(3.dp)
                } else {
                    Modifier
                },
            )
            .background(labelColor(value) ?: EglTokens.Grid, CircleShape)
            .clickable(onClick = onClick),
    )
}

fun labelColor(name: String?): Color? = when (LabelColor.fromWire(name)) {
    LabelColor.RED -> Color(0xFFE53935)
    LabelColor.ORANGE -> Color(0xFFFB8C00)
    LabelColor.YELLOW -> Color(0xFFFDD835)
    LabelColor.GREEN -> Color(0xFF43A047)
    LabelColor.AQUA -> Color(0xFF00ACC1)
    LabelColor.BLUE -> Color(0xFF1E88E5)
    LabelColor.PURPLE -> Color(0xFF8E24AA)
    LabelColor.PINK -> Color(0xFFD81B60)
    null -> null
}

/** Where a folder moves: the top level or another folder, never into itself. */
@Composable
fun FolderMoveDialog(
    open: OpenLibrary,
    folder: Folder,
    onDismiss: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    val blocked = remember(folder.id, open.folders) { open.subtree(folder.id) }
    val current = remember(folder.id, open.folders) { open.parentOf(folder.id) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.folder_move_title)) },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(320.dp)
                    .verticalScroll(rememberScrollState()),
            ) {
                MoveRow(
                    title = stringResource(R.string.folder_top_level),
                    depth = 0,
                    selected = current == null,
                    onClick = {
                        scope.launch {
                            moveFolder(open, folder, null, current)
                            onDismiss()
                        }
                    },
                )
                open.pickableFolders.filter { it.folder.id !in blocked }.forEach { entry ->
                    MoveRow(
                        title = entry.folder.name,
                        depth = entry.depth,
                        selected = current == entry.folder.id,
                        onClick = {
                            scope.launch {
                                moveFolder(open, folder, entry.folder.id, current)
                                onDismiss()
                            }
                        },
                    )
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}

@Composable
private fun MoveRow(title: String, depth: Int, selected: Boolean, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(start = (depth * 16).dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = title,
            color = EglTokens.Foreground,
            modifier = Modifier.weight(1f),
        )
        if (selected) {
            Text("✓", color = EglTokens.Primary)
        }
    }
}

private suspend fun moveFolder(
    open: OpenLibrary,
    folder: Folder,
    parent: String?,
    current: String?,
) {
    if (parent == current) return
    open.saveFolder(
        FolderChange.Move(
            id = folder.id,
            parent = parent,
            index = open.endIndex(parent),
        ),
    )
}
