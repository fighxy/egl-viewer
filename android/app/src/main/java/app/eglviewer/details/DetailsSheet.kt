package app.eglviewer.details

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.browse.BrowseHost
import app.eglviewer.browse.FolderPickerSheet
import app.eglviewer.browse.TagEntryField
import app.eglviewer.core.ItemCell
import app.eglviewer.theme.EglTokens
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DetailsSheet(
    open: BrowseHost,
    cell: ItemCell,
    onDismiss: () -> Unit,
) {
    val model = remember(open, cell.id) { ItemDetailsModel(open, cell.id) }
    val item by model.item.collectAsState()
    val vocabulary by model.vocabulary.collectAsState()
    val scope = rememberCoroutineScope()
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    var name by remember(cell.id) { mutableStateOf(cell.name) }
    var notes by remember(cell.id) { mutableStateOf("") }
    var link by remember(cell.id) { mutableStateOf("") }
    var tagText by remember { mutableStateOf("") }
    var pickingFolder by remember { mutableStateOf(false) }

    LaunchedEffect(cell.id) {
        model.load()
    }
    LaunchedEffect(item) {
        val current = item ?: return@LaunchedEffect
        name = current.name
        notes = current.annotation
        link = current.url
    }

    suspend fun save() {
        model.rename(name)
        model.setNotes(notes)
        model.setLink(link)
    }

    DisposableEffect(Unit) {
        onDispose {
            scope.launch { save() }
        }
    }

    ModalBottomSheet(
        onDismissRequest = {
            scope.launch {
                save()
                onDismiss()
            }
        },
        sheetState = sheetState,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 8.dp)
                .padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = stringResource(R.string.details_title),
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 18.sp,
                )
                TextButton(
                    onClick = {
                        scope.launch {
                            save()
                            onDismiss()
                        }
                    },
                ) { Text(stringResource(R.string.common_done)) }
            }

            FieldLabel(stringResource(R.string.details_name))
            BasicTextField(
                value = name,
                onValueChange = { name = it },
                enabled = !model.readOnly,
                singleLine = true,
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag("details.name"),
            )

            FieldLabel(stringResource(R.string.details_rating))
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                (1..5).forEach { stars ->
                    val selected = (item?.star ?: 0) >= stars
                    TextButton(
                        onClick = { scope.launch { model.rate(stars) } },
                        enabled = !model.readOnly,
                        modifier = Modifier.testTag("details.star.$stars"),
                    ) {
                        Text(
                            text = if (selected) "★" else "☆",
                            color = if (selected) EglTokens.Primary else EglTokens.MutedForeground,
                            fontSize = 22.sp,
                        )
                    }
                }
            }

            FieldLabel(stringResource(R.string.details_tags))
            item?.tags?.forEach { tag ->
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(tag)
                    if (!model.readOnly) {
                        TextButton(onClick = { scope.launch { model.removeTag(tag) } }) {
                            Text(stringResource(R.string.common_remove))
                        }
                    }
                }
            }
            if (!model.readOnly) {
                TagEntryField(
                    text = tagText,
                    onText = { tagText = it },
                    vocabulary = vocabulary.map { it.name },
                    taken = item?.tags?.toSet().orEmpty(),
                    onSubmit = { value ->
                        scope.launch {
                            model.addTag(value)
                            tagText = ""
                        }
                    },
                )
            }

            FieldLabel(stringResource(R.string.details_folders))
            model.folderNames.forEach { (id, folderName) ->
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text(folderName)
                    if (!model.readOnly) {
                        TextButton(onClick = { scope.launch { model.toggleFolder(id) } }) {
                            Text(stringResource(R.string.common_remove))
                        }
                    }
                }
            }
            if (!model.readOnly) {
                TextButton(onClick = { pickingFolder = true }) {
                    Text(stringResource(R.string.action_add_to_folder))
                }
            }

            FieldLabel(stringResource(R.string.details_notes))
            BasicTextField(
                value = notes,
                onValueChange = { notes = it },
                enabled = !model.readOnly,
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag("details.notes"),
            )

            FieldLabel(stringResource(R.string.details_link))
            BasicTextField(
                value = link,
                onValueChange = { link = it },
                enabled = !model.readOnly,
                singleLine = true,
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag("details.link"),
            )

            FieldLabel(stringResource(R.string.details_facts))
            val current = item
            if (current != null) {
                FactRow(stringResource(R.string.details_type), current.ext.uppercase())
                if (current.width != null && current.height != null) {
                    FactRow(
                        stringResource(R.string.details_dimensions),
                        "${current.width} × ${current.height}",
                    )
                }
                current.size?.let {
                    FactRow(stringResource(R.string.details_size), formatBytes(it))
                }
                current.duration?.let {
                    FactRow(stringResource(R.string.details_duration), formatDuration(it))
                }
                current.addedAt?.let {
                    FactRow(stringResource(R.string.details_added), formatDate(it))
                }
                current.fileCreatedAt?.let {
                    FactRow(stringResource(R.string.details_created), formatDate(it))
                }
                current.fileModifiedAt?.let {
                    FactRow(stringResource(R.string.details_modified), formatDate(it))
                }
            }
        }
    }

    if (pickingFolder) {
        FolderPickerSheet(
            folders = open.pickableFolders,
            title = stringResource(R.string.action_add_to_folder),
            chosen = item?.folders?.toSet().orEmpty(),
            onPick = { id ->
                pickingFolder = false
                scope.launch { model.toggleFolder(id) }
            },
            onDismiss = { pickingFolder = false },
        )
    }
}

@Composable
private fun FieldLabel(text: String) {
    Text(
        text = text,
        color = EglTokens.MutedForeground,
        fontSize = 12.sp,
        fontWeight = FontWeight.SemiBold,
    )
}

@Composable
private fun FactRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(label, color = EglTokens.MutedForeground, fontSize = 13.sp)
        Text(value, color = EglTokens.Foreground, fontSize = 13.sp)
    }
}

private fun formatBytes(bytes: Int): String {
    if (bytes < 1024) return "$bytes B"
    val kb = bytes / 1024.0
    if (kb < 1024) return String.format("%.1f KB", kb)
    return String.format("%.1f MB", kb / 1024.0)
}

private fun formatDuration(seconds: Double): String {
    val total = seconds.toInt()
    val m = total / 60
    val s = total % 60
    return "%d:%02d".format(m, s)
}

private fun formatDate(epochMs: Long): String =
    DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(epochMs))
