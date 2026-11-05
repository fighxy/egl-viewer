package app.eglviewer.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AddCircleOutline
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.OpenLibrary
import app.eglviewer.core.Backup
import app.eglviewer.core.BackupKind
import app.eglviewer.theme.EglTokens
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** The library's database backups: back up now, or put one back. */
@Composable
fun BackupsScreen(
    open: OpenLibrary,
    onBack: () -> Unit,
) {
    var backups by remember { mutableStateOf<List<Backup>>(emptyList()) }
    var restoring by remember { mutableStateOf<Backup?>(null) }
    var working by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val backedUpToast = stringResource(R.string.toast_backed_up)

    suspend fun reload() {
        backups = withContext(Dispatchers.IO) {
            runCatching { open.library.backups() }.onFailure {
                open.report(it.message ?: it.toString())
            }.getOrDefault(emptyList())
        }
    }

    LaunchedEffect(open) { reload() }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        SettingsTopBar(
            title = stringResource(R.string.settings_backups),
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
            if (!open.readOnly) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable(enabled = !working) {
                            scope.launch {
                                working = true
                                val ok = withContext(Dispatchers.IO) {
                                    runCatching { open.library.backUpNow() }
                                        .onFailure { open.report(it.message ?: it.toString()) }
                                        .isSuccess
                                }
                                if (ok) open.show(backedUpToast, undoable = false)
                                reload()
                                working = false
                            }
                        }
                        .padding(vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Icon(
                        Icons.Default.AddCircleOutline,
                        contentDescription = null,
                        tint = EglTokens.Primary,
                    )
                    Text(
                        text = stringResource(R.string.backups_now),
                        color = EglTokens.Foreground,
                        fontSize = 15.sp,
                    )
                }
                Text(
                    text = stringResource(R.string.backups_hint),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                    modifier = Modifier.padding(bottom = 12.dp),
                )
            }

            if (backups.isEmpty() && !working) {
                Text(
                    text = stringResource(R.string.backups_none),
                    color = EglTokens.MutedForeground,
                    fontSize = 14.sp,
                    modifier = Modifier.padding(vertical = 24.dp),
                )
            } else {
                for (backup in backups) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable(enabled = !open.readOnly && !working) {
                                restoring = backup
                            }
                            .padding(vertical = 12.dp),
                    ) {
                        Text(
                            text = DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT)
                                .format(Date(backup.createdAt)),
                            color = EglTokens.Foreground,
                            fontSize = 15.sp,
                        )
                        Text(
                            text = "${kindLabel(backup.kind)} · ${formatBytes(backup.size)}",
                            color = EglTokens.MutedForeground,
                            fontSize = 12.sp,
                        )
                    }
                }
            }
        }
    }

    restoring?.let { backup ->
        AlertDialog(
            onDismissRequest = { restoring = null },
            title = { Text(stringResource(R.string.backups_restore_title)) },
            text = { Text(stringResource(R.string.backups_restore_hint)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        restoring = null
                        scope.launch {
                            working = true
                            open.restoreBackup(backup.name)
                            reload()
                            working = false
                        }
                    },
                ) {
                    Text(stringResource(R.string.backups_restore))
                }
            },
            dismissButton = {
                TextButton(onClick = { restoring = null }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }
}

@Composable
private fun kindLabel(kind: BackupKind?): String =
    stringResource(
        when (kind) {
            BackupKind.DAILY -> R.string.backups_daily
            BackupKind.MANUAL -> R.string.backups_manual
            BackupKind.BEFORE_MIGRATION -> R.string.backups_before_migration
            BackupKind.BEFORE_RESTORE -> R.string.backups_before_restore
            null -> R.string.backups_other
        },
    )
