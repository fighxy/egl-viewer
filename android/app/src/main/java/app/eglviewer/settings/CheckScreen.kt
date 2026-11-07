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
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.HealthAndSafety
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.OpenLibrary
import app.eglviewer.core.LibraryCheck
import app.eglviewer.core.RepairOptions
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Check Library and Repair: the database, missing originals, files nothing points at. */
@Composable
fun CheckScreen(
    open: OpenLibrary,
    onBack: () -> Unit,
) {
    var result by remember { mutableStateOf<LibraryCheck?>(null) }
    var working by remember { mutableStateOf(false) }
    var removeOrphans by remember { mutableStateOf(true) }
    var removeMissing by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    fun run(work: suspend () -> LibraryCheck?) {
        scope.launch {
            working = true
            result = withContext(Dispatchers.IO) {
                runCatching { work() }.onFailure {
                    open.report(it.message ?: it.toString())
                }.getOrNull()
            }
            working = false
            withContext(Dispatchers.IO) { runCatching { open.refresh() } }
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        SettingsTopBar(
            title = stringResource(R.string.settings_check),
            onBack = onBack,
        )
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            TextButton(
                onClick = { run { open.library.check() } },
                enabled = !working,
            ) {
                Icon(Icons.Default.HealthAndSafety, contentDescription = null)
                Text(
                    text = stringResource(R.string.check_run),
                    modifier = Modifier.padding(start = 8.dp),
                )
            }
            Text(
                text = stringResource(R.string.check_hint),
                color = EglTokens.MutedForeground,
                fontSize = 12.sp,
            )

            if (working) {
                CircularProgressIndicator(color = EglTokens.Primary)
            }

            val check = result
            if (check != null) {
                SectionHeader(stringResource(R.string.check_result))
                LabeledValue(stringResource(R.string.check_items), check.items.toString())
                LabeledValue(
                    stringResource(R.string.check_database),
                    stringResource(if (check.databaseOk) R.string.check_ok else R.string.check_problems),
                )
                for (problem in check.databaseProblems) {
                    Text(text = problem, color = EglTokens.Destructive, fontSize = 12.sp)
                }
                LabeledValue(stringResource(R.string.check_missing), check.missingCount.toString())
                LabeledValue(
                    stringResource(R.string.check_orphans),
                    "${check.orphanCount} · ${formatBytes(check.orphanBytes)}",
                )
                LabeledValue(
                    stringResource(R.string.check_orphan_thumbnails),
                    check.orphanThumbnails.toString(),
                )
                if (check.isClean) {
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.padding(top = 8.dp),
                    ) {
                        Icon(
                            Icons.Default.CheckCircle,
                            contentDescription = null,
                            tint = EglTokens.Success,
                        )
                        Text(
                            text = stringResource(R.string.check_clean),
                            color = EglTokens.Success,
                        )
                    }
                } else if (!open.readOnly) {
                    SectionHeader(stringResource(R.string.check_repair))
                    SwitchRow(
                        title = stringResource(R.string.check_remove_orphans),
                        checked = removeOrphans,
                        onCheckedChange = { removeOrphans = it },
                    )
                    SwitchRow(
                        title = stringResource(R.string.check_remove_missing),
                        checked = removeMissing,
                        onCheckedChange = { removeMissing = it },
                    )
                    TextButton(
                        onClick = {
                            val options = RepairOptions(orphans = removeOrphans, missing = removeMissing)
                            run { open.library.repair(options) }
                        },
                        enabled = !working && (removeOrphans || removeMissing),
                    ) {
                        Text(stringResource(R.string.check_repair))
                    }
                    Text(
                        text = stringResource(R.string.check_repair_hint),
                        color = EglTokens.MutedForeground,
                        fontSize = 12.sp,
                    )
                }
            }
        }
    }
}

@Composable
private fun SwitchRow(
    title: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = title,
            color = EglTokens.Foreground,
            fontSize = 14.sp,
            modifier = Modifier
                .weight(1f)
                .padding(end = 12.dp),
        )
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}
