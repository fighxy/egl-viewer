package app.eglviewer.organize

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.CreateNewFolder
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.OpenLibrary
import app.eglviewer.browse.Place
import app.eglviewer.core.Tag
import app.eglviewer.core.TagGroup
import app.eglviewer.core.TagGroupChange
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

sealed class TagGroupEditing {
    data object Create : TagGroupEditing()
    data class Edit(val group: TagGroup) : TagGroupEditing()
}

/**
 * The library's tags in their groups (D-105 ≈ D-097): open one to see its items, rename,
 * merge, delete; make, edit and delete tag groups. Tags stay case-sensitive.
 */
@Composable
fun TagsScreen(
    open: OpenLibrary,
    onOpenPlace: (Place) -> Unit,
    onBack: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var tags by remember { mutableStateOf<List<Tag>>(emptyList()) }
    var search by remember { mutableStateOf("") }
    var renaming by remember { mutableStateOf<String?>(null) }
    var newName by remember { mutableStateOf("") }
    var merging by remember { mutableStateOf<String?>(null) }
    var deleting by remember { mutableStateOf<String?>(null) }
    var editingGroup by remember { mutableStateOf<TagGroupEditing?>(null) }
    var deletingGroup by remember { mutableStateOf<TagGroup?>(null) }

    suspend fun reload() {
        withContext(Dispatchers.IO) { open.refresh() }
        tags = withContext(Dispatchers.IO) {
            runCatching { open.library.tags() }.getOrDefault(emptyList())
                .sortedBy { it.name.lowercase() }
        }
    }

    LaunchedEffect(open, open.counts) { reload() }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onBack) {
                Icon(
                    Icons.AutoMirrored.Filled.ArrowBack,
                    contentDescription = stringResource(R.string.library_close),
                    tint = EglTokens.Foreground,
                )
            }
            Text(
                text = stringResource(R.string.library_tags),
                color = EglTokens.Foreground,
                fontSize = EglTokens.TitleSp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.weight(1f),
            )
            if (!open.readOnly) {
                IconButton(onClick = { editingGroup = TagGroupEditing.Create }) {
                    Icon(
                        Icons.Default.CreateNewFolder,
                        contentDescription = stringResource(R.string.tags_new_group),
                        tint = EglTokens.Foreground,
                    )
                }
            }
        }

        OutlinedTextField(
            value = search,
            onValueChange = { search = it },
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 4.dp),
            placeholder = { Text(stringResource(R.string.tags_find)) },
            singleLine = true,
        )

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
        ) {
            if (tags.isEmpty()) {
                Text(
                    text = stringResource(R.string.tags_none),
                    color = EglTokens.MutedForeground,
                    modifier = Modifier.padding(vertical = 24.dp),
                )
            }

            open.tagGroups.forEach { group ->
                val members = shown(group.tags, tags, search)
                if (members.isNotEmpty() || search.isBlank()) {
                    GroupHeader(
                        group = group,
                        readOnly = open.readOnly,
                        onEdit = { editingGroup = TagGroupEditing.Edit(group) },
                        onDelete = { deletingGroup = group },
                    )
                    members.forEach { tag ->
                        TagRow(
                            tag = tag,
                            group = group,
                            open = open,
                            onOpen = { onOpenPlace(Place.Tag(name = tag.name)) },
                            onRename = {
                                newName = tag.name
                                renaming = tag.name
                            },
                            onMerge = { merging = tag.name },
                            onDelete = { deleting = tag.name },
                            onReload = { scope.launch { reload() } },
                        )
                    }
                }
            }

            val grouped = open.tagGroups.flatMap { it.tags }.toSet()
            val loose = shown(tags.map { it.name }.filter { it !in grouped }, tags, search)
            if (loose.isNotEmpty()) {
                Text(
                    text = if (open.tagGroups.isEmpty()) {
                        stringResource(R.string.tags_all)
                    } else {
                        stringResource(R.string.tags_ungrouped)
                    },
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(top = 16.dp, bottom = 4.dp),
                )
                loose.forEach { tag ->
                    TagRow(
                        tag = tag,
                        group = null,
                        open = open,
                        onOpen = { onOpenPlace(Place.Tag(name = tag.name)) },
                        onRename = {
                            newName = tag.name
                            renaming = tag.name
                        },
                        onMerge = { merging = tag.name },
                        onDelete = { deleting = tag.name },
                        onReload = { scope.launch { reload() } },
                    )
                }
            }
        }
    }

    renaming?.let { from ->
        AlertDialog(
            onDismissRequest = { renaming = null },
            title = { Text(stringResource(R.string.tags_rename_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.tags_rename_hint), fontSize = 13.sp)
                    OutlinedTextField(
                        value = newName,
                        onValueChange = { newName = it },
                        label = { Text(stringResource(R.string.tags_new_name)) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        scope.launch {
                            open.renameTag(from, newName)
                            renaming = null
                            reload()
                        }
                    },
                ) { Text(stringResource(R.string.tags_rename)) }
            },
            dismissButton = {
                TextButton(onClick = { renaming = null }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }

    deleting?.let { name ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text(stringResource(R.string.tags_delete_title)) },
            text = { Text(name) },
            confirmButton = {
                TextButton(
                    onClick = {
                        scope.launch {
                            open.deleteTag(name)
                            deleting = null
                            reload()
                        }
                    },
                ) { Text(stringResource(R.string.tags_delete)) }
            },
            dismissButton = {
                TextButton(onClick = { deleting = null }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }

    deletingGroup?.let { group ->
        AlertDialog(
            onDismissRequest = { deletingGroup = null },
            title = { Text(stringResource(R.string.tags_delete_group_title)) },
            text = { Text(stringResource(R.string.tags_delete_group_hint)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        scope.launch {
                            open.deleteTagGroup(group.id)
                            deletingGroup = null
                        }
                    },
                ) { Text(stringResource(R.string.tags_delete_group)) }
            },
            dismissButton = {
                TextButton(onClick = { deletingGroup = null }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }

    merging?.let { source ->
        MergeTagDialog(
            source = source,
            tags = tags.map { it.name },
            onDismiss = { merging = null },
            onPick = { target ->
                scope.launch {
                    if (open.mergeTag(source, target)) {
                        merging = null
                        reload()
                    }
                }
            },
        )
    }

    editingGroup?.let { editing ->
        TagGroupEditorDialog(
            open = open,
            editing = editing,
            vocabulary = tags.map { it.name },
            onDismiss = { editingGroup = null },
        )
    }
}

private fun shown(names: List<String>, tags: List<Tag>, search: String): List<Tag> {
    val counts = tags.associate { it.name to it.imageCount }
    val needle = search.trim()
    return names
        .filter { needle.isEmpty() || it.contains(needle, ignoreCase = true) }
        .map { Tag(name = it, imageCount = counts[it]) }
}

@Composable
private fun GroupHeader(
    group: TagGroup,
    readOnly: Boolean,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 16.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        group.color?.let { c ->
            androidx.compose.foundation.layout.Box(
                modifier = Modifier
                    .padding(end = 8.dp)
                    .size(9.dp)
                    .clip(CircleShape)
                    .background(labelColor(c) ?: EglTokens.MutedForeground),
            )
        }
        Text(
            text = group.name,
            color = EglTokens.MutedForeground,
            fontSize = 12.sp,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.weight(1f),
        )
        if (!readOnly) {
            IconButton(onClick = { menu = true }) {
                Icon(Icons.Default.MoreVert, contentDescription = stringResource(R.string.tags_group_actions))
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.tags_edit_group)) },
                    onClick = {
                        menu = false
                        onEdit()
                    },
                )
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.tags_delete_group)) },
                    onClick = {
                        menu = false
                        onDelete()
                    },
                )
            }
        }
    }
}

@Composable
private fun TagRow(
    tag: Tag,
    group: TagGroup?,
    open: OpenLibrary,
    onOpen: () -> Unit,
    onRename: () -> Unit,
    onMerge: () -> Unit,
    onDelete: () -> Unit,
    onReload: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var menu by remember { mutableStateOf(false) }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpen)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = tag.name,
            color = EglTokens.Foreground,
            fontSize = 15.sp,
            fontWeight = FontWeight.Medium,
            modifier = Modifier.weight(1f),
        )
        tag.imageCount?.let {
            Text(text = it.toString(), color = EglTokens.MutedForeground, fontSize = 14.sp)
        }
        if (!open.readOnly) {
            IconButton(onClick = { menu = true }) {
                Icon(Icons.Default.MoreVert, contentDescription = null, tint = EglTokens.MutedForeground)
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.tags_rename)) },
                    onClick = {
                        menu = false
                        onRename()
                    },
                )
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.tags_merge)) },
                    onClick = {
                        menu = false
                        onMerge()
                    },
                )
                open.tagGroups.filter { tag.name !in it.tags }.forEach { target ->
                    DropdownMenuItem(
                        text = { Text("${stringResource(R.string.tags_add_to_group)}: ${target.name}") },
                        onClick = {
                            menu = false
                            scope.launch {
                                open.saveTagGroup(
                                    TagGroupChange.Update(
                                        id = target.id,
                                        name = target.name,
                                        color = target.color,
                                        tags = target.tags + tag.name,
                                    ),
                                )
                                onReload()
                            }
                        },
                    )
                }
                if (group != null) {
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.tags_remove_from_group)) },
                        onClick = {
                            menu = false
                            scope.launch {
                                open.saveTagGroup(
                                    TagGroupChange.Update(
                                        id = group.id,
                                        name = group.name,
                                        color = group.color,
                                        tags = group.tags.filter { it != tag.name },
                                    ),
                                )
                                onReload()
                            }
                        },
                    )
                }
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.tags_delete)) },
                    onClick = {
                        menu = false
                        onDelete()
                    },
                )
            }
        }
    }
    HorizontalDivider(color = EglTokens.Grid)
}

@Composable
private fun MergeTagDialog(
    source: String,
    tags: List<String>,
    onDismiss: () -> Unit,
    onPick: (String) -> Unit,
) {
    var search by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.tags_merge_title)) },
        text = {
            Column {
                OutlinedTextField(
                    value = search,
                    onValueChange = { search = it },
                    placeholder = { Text(stringResource(R.string.tags_find)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState())
                        .padding(top = 8.dp),
                ) {
                    tags.filter { it != source && (search.isBlank() || it.contains(search, ignoreCase = true)) }
                        .forEach { target ->
                            Text(
                                text = target,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clickable { onPick(target) }
                                    .padding(vertical = 12.dp),
                                color = EglTokens.Foreground,
                            )
                        }
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
fun TagGroupEditorDialog(
    open: OpenLibrary,
    editing: TagGroupEditing,
    vocabulary: List<String>,
    onDismiss: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    var color by remember { mutableStateOf<String?>(null) }
    var chosen by remember { mutableStateOf<List<String>>(emptyList()) }

    LaunchedEffect(editing) {
        when (editing) {
            TagGroupEditing.Create -> {
                name = ""
                color = null
                chosen = emptyList()
            }
            is TagGroupEditing.Edit -> {
                name = editing.group.name
                color = editing.group.color
                chosen = editing.group.tags
            }
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                if (editing is TagGroupEditing.Create) {
                    stringResource(R.string.tags_new_group)
                } else {
                    stringResource(R.string.tags_edit_group)
                },
            )
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.tags_group_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(stringResource(R.string.folder_color), fontSize = 13.sp, color = EglTokens.MutedForeground)
                ColorChoice(color = color, onColor = { color = it })
                Text(stringResource(R.string.tags_group_tags), fontSize = 13.sp, color = EglTokens.MutedForeground)
                vocabulary.forEach { tag ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable {
                                chosen = if (tag in chosen) chosen - tag else chosen + tag
                            },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Checkbox(
                            checked = tag in chosen,
                            onCheckedChange = {
                                chosen = if (it) chosen + tag else chosen - tag
                            },
                        )
                        Text(tag, color = EglTokens.Foreground)
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = name.trim().isNotEmpty(),
                onClick = {
                    val trimmed = name.trim()
                    val change = when (editing) {
                        TagGroupEditing.Create -> TagGroupChange.Create(
                            name = trimmed,
                            color = color,
                            tags = chosen,
                        )
                        is TagGroupEditing.Edit -> TagGroupChange.Update(
                            id = editing.group.id,
                            name = trimmed,
                            color = color,
                            tags = chosen,
                        )
                    }
                    scope.launch {
                        if (open.saveTagGroup(change)) onDismiss()
                    }
                },
            ) { Text(stringResource(R.string.common_save)) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}
