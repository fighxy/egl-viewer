package app.eglviewer.adding

import android.content.Intent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.PhotoLibrary
import androidx.compose.material.icons.filled.StopCircle
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.LibraryLocations
import app.eglviewer.app.OpenLibrary
import app.eglviewer.core.DuplicateAnswer
import app.eglviewer.core.ImportKind
import app.eglviewer.core.ImportProgress
import app.eglviewer.theme.EglTokens
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * + menu: Gallery, Files, Folder, Paste (D-104 ≈ D-096). Trash places omit it.
 * [folder] is the destination folder id when adding from a folder place.
 */
@Composable
fun AddMenuButton(
    open: OpenLibrary,
    folder: String?,
    cacheDir: File = LocalContext.current.cacheDir,
) {
    if (open.readOnly) return
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var menuOpen by remember { mutableStateOf(false) }
    var preparing by remember { mutableStateOf(false) }
    val locations = remember { LibraryLocations(context) }

    val gallery = rememberLauncherForActivityResult(
        ActivityResultContracts.PickMultipleVisualMedia(),
    ) { uris ->
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        scope.launch {
            preparing = true
            val batch = withContext(Dispatchers.IO) {
                ImportPaths.fromContentUris(
                    context = context,
                    uris = uris,
                    cacheDir = cacheDir,
                    resolvePath = { null },
                )
            }
            preparing = false
            val unreadable = uris.size - batch.paths.size
            open.addPaths(batch.paths, into = folder, cleanup = batch.cleanup)
            if (unreadable > 0) {
                open.showUnreadable(unreadable)
            }
        }
    }

    val filesPicker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenMultipleDocuments(),
    ) { uris ->
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        scope.launch {
            preparing = true
            val batch = withContext(Dispatchers.IO) {
                ImportPaths.fromContentUris(
                    context = context,
                    uris = uris,
                    cacheDir = cacheDir,
                    resolvePath = { locations.resolveFilesystemPath(it) },
                    takeGrant = { uri ->
                        runCatching {
                            context.contentResolver.takePersistableUriPermission(
                                uri,
                                Intent.FLAG_GRANT_READ_URI_PERMISSION,
                            )
                        }
                    },
                    releaseGrant = { uri ->
                        runCatching {
                            context.contentResolver.releasePersistableUriPermission(
                                uri,
                                Intent.FLAG_GRANT_READ_URI_PERMISSION,
                            )
                        }
                    },
                )
            }
            preparing = false
            open.addPaths(batch.paths, into = folder, cleanup = batch.cleanup)
        }
    }

    val folderPicker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocumentTree(),
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            preparing = true
            val batch = withContext(Dispatchers.IO) {
                ImportPaths.fromTreeUri(
                    context = context,
                    tree = uri,
                    cacheDir = cacheDir,
                    takeGrant = { tree ->
                        runCatching {
                            context.contentResolver.takePersistableUriPermission(
                                tree,
                                Intent.FLAG_GRANT_READ_URI_PERMISSION,
                            )
                        }
                    },
                    releaseGrant = { tree ->
                        runCatching {
                            context.contentResolver.releasePersistableUriPermission(
                                tree,
                                Intent.FLAG_GRANT_READ_URI_PERMISSION,
                            )
                        }
                    },
                    resolvePath = { locations.resolveFilesystemPath(it) },
                    canReadPath = { path -> locations.canAccess(path) },
                )
            }
            preparing = false
            open.addPaths(batch.paths, into = folder, cleanup = batch.cleanup)
        }
    }

    Row(verticalAlignment = Alignment.CenterVertically) {
        if (preparing) {
            CircularProgressIndicator(
                modifier = Modifier.padding(8.dp),
                strokeWidth = 2.dp,
            )
        }
        IconButton(onClick = { menuOpen = true }) {
            Icon(
                Icons.Default.Add,
                contentDescription = stringResource(R.string.add_title),
                tint = EglTokens.Foreground,
            )
        }
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.add_photos)) },
                leadingIcon = { Icon(Icons.Default.PhotoLibrary, contentDescription = null) },
                onClick = {
                    menuOpen = false
                    gallery.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageAndVideo))
                },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.add_files)) },
                leadingIcon = { Icon(Icons.Default.Folder, contentDescription = null) },
                onClick = {
                    menuOpen = false
                    filesPicker.launch(arrayOf("*/*"))
                },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.add_folder)) },
                leadingIcon = { Icon(Icons.Default.Folder, contentDescription = null) },
                onClick = {
                    menuOpen = false
                    folderPicker.launch(null)
                },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.add_paste)) },
                leadingIcon = { Icon(Icons.Default.ContentPaste, contentDescription = null) },
                enabled = ImportPaths.clipboardHasSomething(context),
                onClick = {
                    menuOpen = false
                    scope.launch {
                        preparing = true
                        val files = withContext(Dispatchers.IO) {
                            ImportPaths.clipboardFiles(context, cacheDir)
                        }
                        preparing = false
                        open.addPicked(files, into = folder)
                    }
                },
            )
        }
    }
}

/** Progress over the place with Stop; one row per running import. */
@Composable
fun ImportBanner(open: OpenLibrary) {
    val imports = open.imports.values.sortedBy { it.id }
    if (imports.isEmpty()) return
    val scope = rememberCoroutineScope()
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
    ) {
        for (progress in imports) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = stringResource(
                            if (progress.kind == ImportKind.EAGLE) R.string.eagle_importing else R.string.library_adding,
                            progress.processed,
                            progress.total.coerceAtLeast(1),
                        ),
                        color = EglTokens.Foreground,
                        fontSize = 12.sp,
                    )
                    LinearProgressIndicator(
                        progress = {
                            val total = progress.total.coerceAtLeast(1).toFloat()
                            progress.processed.toFloat() / total
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 4.dp),
                    )
                }
                IconButton(
                    onClick = {
                        val library = open.library
                        val id = progress.id
                        scope.launch(Dispatchers.IO) {
                            runCatching { library.cancelImport(id) }
                        }
                    },
                ) {
                    Icon(
                        Icons.Default.StopCircle,
                        contentDescription = stringResource(R.string.add_stop),
                        tint = EglTokens.MutedForeground,
                    )
                }
            }
        }
    }
}

/** Duplicate question for the batch; the answer holds for the rest. */
@Composable
fun DuplicateQuestionDialog(open: OpenLibrary) {
    val question = open.duplicateQuestion ?: return
    val duplicate = question.duplicate
    val name = duplicate?.path?.let { File(it).name }.orEmpty()
    val message = buildString {
        if (name.isNotEmpty()) append(name).append('\n')
        append(
            stringResource(
                if (duplicate?.inTrash == true) R.string.duplicate_in_trash else R.string.duplicate_message,
            ),
        )
    }
    val scope = rememberCoroutineScope()
    AlertDialog(
        onDismissRequest = { /* must answer */ },
        title = { Text(stringResource(R.string.duplicate_title)) },
        text = { Text(message) },
        confirmButton = {
            TextButton(onClick = { scope.launch { open.answer(DuplicateAnswer.SKIP) } }) {
                Text(stringResource(R.string.duplicate_skip))
            }
        },
        dismissButton = {
            Column {
                TextButton(onClick = { scope.launch { open.answer(DuplicateAnswer.ADD_ANYWAY) } }) {
                    Text(stringResource(R.string.duplicate_add_anyway))
                }
                if (duplicate?.targetFolder != null) {
                    TextButton(onClick = { scope.launch { open.answer(DuplicateAnswer.ADD_TO_FOLDER) } }) {
                        Text(stringResource(R.string.duplicate_add_to_folder))
                    }
                }
                TextButton(onClick = { scope.launch { open.answer(DuplicateAnswer.CANCEL) } }) {
                    Text(stringResource(R.string.duplicate_stop))
                }
            }
        },
    )
}

/** Summarises a finished import for the toast (same wording as iOS D-096). */
fun importSummary(
    progress: ImportProgress,
    addedLabel: (Int) -> String,
    duplicatesLabel: (Int) -> String,
    failedLabel: (Int) -> String,
    cancelledLabel: String,
): String {
    val parts = mutableListOf(addedLabel(progress.added))
    if (progress.duplicates > 0) parts += duplicatesLabel(progress.duplicates)
    if (progress.failed > 0) parts += failedLabel(progress.failed)
    if (progress.state == app.eglviewer.core.ImportState.CANCELLED) parts += cancelledLabel
    return parts.joinToString(" · ")
}

/** How an Eagle import ended: what came in, what did not, and the locked folders (D-116). */
@Composable
fun EagleOutcomeDialog(open: OpenLibrary) {
    val outcome = open.eagleOutcome ?: return
    val report = outcome.report
    val lines = buildList {
        add(stringResource(R.string.eagle_imported, report.imported))
        if (report.duplicates > 0) add(stringResource(R.string.eagle_duplicates, report.duplicates))
        if (report.skipped.isNotEmpty()) add(stringResource(R.string.eagle_skipped, report.skipped.size))
        if (report.trashSkipped > 0) add(stringResource(R.string.eagle_trash_left, report.trashSkipped))
        if (report.lockedFolders.isNotEmpty()) {
            add(stringResource(R.string.eagle_locked, report.lockedFolders.joinToString(", ")))
        }
        if (outcome.cancelled) add(stringResource(R.string.eagle_resume))
    }
    AlertDialog(
        onDismissRequest = { open.eagleOutcome = null },
        title = { Text(stringResource(if (outcome.cancelled) R.string.eagle_stopped else R.string.eagle_done)) },
        text = { Text(lines.joinToString("\n")) },
        confirmButton = {
            TextButton(onClick = { open.eagleOutcome = null }) {
                Text(stringResource(android.R.string.ok))
            }
        },
    )
}
