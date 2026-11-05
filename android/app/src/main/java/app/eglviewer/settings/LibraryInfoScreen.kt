package app.eglviewer.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.OpenLibrary
import app.eglviewer.core.LibraryStats
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** The library's name, where it lives and what it takes on disk; rebuilding its thumbnails. */
@Composable
fun LibraryInfoScreen(
    open: OpenLibrary,
    onBack: () -> Unit,
) {
    var name by remember(open.info.name) { mutableStateOf(open.info.name) }
    var stats by remember { mutableStateOf<LibraryStats?>(null) }
    var confirmRebuild by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val rebuildingToast = stringResource(R.string.toast_rebuilding)

    LaunchedEffect(open) {
        stats = withContext(Dispatchers.IO) {
            runCatching { open.library.stats() }.onFailure {
                open.report(it.message ?: it.toString())
            }.getOrNull()
        }
    }

    DisposableEffect(Unit) {
        onDispose {
            val trimmed = name.trim()
            if (!open.readOnly && trimmed.isNotEmpty() && trimmed != open.info.name) {
                scope.launch { open.rename(trimmed) }
            }
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        SettingsTopBar(
            title = stringResource(R.string.settings_library_title),
            onBack = onBack,
        )
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            SectionHeader(stringResource(R.string.settings_name))
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                enabled = !open.readOnly,
                label = { Text(stringResource(R.string.settings_name)) },
            )

            SectionHeader(stringResource(R.string.settings_location))
            Text(
                text = open.path.absolutePath,
                color = EglTokens.MutedForeground,
                fontSize = 13.sp,
            )
            if (open.readOnly) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(vertical = 8.dp),
                ) {
                    Icon(Icons.Default.Lock, contentDescription = null, tint = EglTokens.MutedForeground)
                    Text(
                        text = stringResource(R.string.library_read_only),
                        color = EglTokens.MutedForeground,
                        fontSize = 13.sp,
                    )
                }
            }

            SectionHeader(stringResource(R.string.settings_storage))
            val current = stats
            if (current == null) {
                CircularProgressIndicator(
                    modifier = Modifier.padding(16.dp),
                    color = EglTokens.Primary,
                )
            } else {
                LabeledValue(stringResource(R.string.settings_originals), formatBytes(current.originals))
                LabeledValue(stringResource(R.string.settings_thumbnails), formatBytes(current.thumbnails))
                LabeledValue(stringResource(R.string.settings_database), formatBytes(current.database))
                LabeledValue(stringResource(R.string.settings_backups_size), formatBytes(current.backups))
                LabeledValue(stringResource(R.string.settings_total), formatBytes(current.total))
            }

            if (!open.readOnly) {
                SectionHeader(stringResource(R.string.settings_rebuild))
                TextButton(onClick = { confirmRebuild = true }) {
                    Text(stringResource(R.string.settings_rebuild))
                }
                Text(
                    text = stringResource(R.string.settings_rebuild_hint),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                )
            }
        }
    }

    if (confirmRebuild) {
        AlertDialog(
            onDismissRequest = { confirmRebuild = false },
            title = { Text(stringResource(R.string.settings_rebuild_confirm)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmRebuild = false
                        scope.launch {
                            val ok = withContext(Dispatchers.IO) {
                                runCatching { open.library.generatePreviews(rebuild = true) }
                                    .onFailure { open.report(it.message ?: it.toString()) }
                                    .isSuccess
                            }
                            if (ok) {
                                open.show(rebuildingToast, undoable = false)
                            }
                        }
                    },
                ) {
                    Text(stringResource(R.string.settings_rebuild))
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmRebuild = false }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }
}
