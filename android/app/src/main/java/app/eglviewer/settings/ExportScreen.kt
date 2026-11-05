package app.eglviewer.settings

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.StringRes
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
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Error
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.OpenLibrary
import app.eglviewer.core.ExportCompression
import app.eglviewer.core.ExportFormat
import app.eglviewer.core.ExportPreview
import app.eglviewer.core.ExportProgress
import app.eglviewer.core.ExportRequest
import app.eglviewer.core.ExportSelection
import app.eglviewer.core.ExportState
import app.eglviewer.core.MIN_PASSPHRASE_CHARS
import app.eglviewer.core.Tag
import app.eglviewer.core.ZipEncryption
import app.eglviewer.core.passphraseLongEnough
import app.eglviewer.details.ShareFiles
import app.eglviewer.details.ShareLauncher
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.UUID

/**
 * Export (Android A7 ≈ D-099): the whole library, folders or tags, as a folder tree, a ZIP or a
 * 7z archive, optionally encrypted with a passphrase: age or AES-256 for a ZIP (D-112), AES-256
 * with hidden names for a 7z (D-113). Written into the app's temporary
 * folder, then shared or saved via SAF; the temporary copy goes when the screen closes.
 */
@Composable
fun ExportScreen(
    open: OpenLibrary,
    onBack: () -> Unit,
) {
    var whole by remember { mutableStateOf(true) }
    var folders by remember { mutableStateOf(setOf<String>()) }
    var tags by remember { mutableStateOf(setOf<String>()) }
    var includeSubfolders by remember { mutableStateOf(true) }
    var kind by remember { mutableStateOf(ExportKind.ZIP) }
    var zipEncryption by remember { mutableStateOf(ZipEncryption.AGE) }
    var compression by remember { mutableStateOf(ExportCompression.BALANCED) }
    var encrypt by remember { mutableStateOf(false) }
    var passphrase by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    var vocabulary by remember { mutableStateOf<List<Tag>>(emptyList()) }
    var preview by remember { mutableStateOf<ExportPreview?>(null) }
    var exportId by remember { mutableStateOf<Long?>(null) }
    var folder by remember { mutableStateOf<File?>(null) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val savedToast = stringResource(R.string.export_saved)

    val selection = ExportSelection(
        whole = whole,
        folders = if (whole) emptyList() else folders.toList(),
        tags = if (whole) emptyList() else tags.toList(),
    )
    val progress: ExportProgress? = exportId?.let { open.exports[it] }

    val saveLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("*/*"),
    ) { uri: Uri? ->
        val source = progress?.summary?.path?.let(::File) ?: return@rememberLauncherForActivityResult
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            val ok = withContext(Dispatchers.IO) {
                runCatching {
                    context.contentResolver.openOutputStream(uri)?.use { out ->
                        source.inputStream().use { input -> input.copyTo(out) }
                    } != null
                }.getOrDefault(false)
            }
            if (ok) {
                open.show(savedToast, undoable = false)
                finishExport(open, exportId, folder) { id, dir ->
                    exportId = id
                    folder = dir
                }
            }
        }
    }

    fun clean() {
        folder?.deleteRecursively()
        folder = null
    }

    fun finish() {
        clean()
        exportId?.let { open.forgetExport(it) }
        exportId = null
    }

    DisposableEffect(Unit) {
        onDispose { clean() }
    }

    LaunchedEffect(open) {
        vocabulary = withContext(Dispatchers.IO) {
            runCatching { open.library.tags() }.getOrDefault(emptyList())
        }
    }

    LaunchedEffect(whole, folders, tags, includeSubfolders) {
        if (selection.isEmpty) {
            preview = null
            return@LaunchedEffect
        }
        preview = withContext(Dispatchers.IO) {
            runCatching {
                open.library.exportPreview(selection, includeSubfolders)
            }.onFailure {
                open.report(it.message ?: it.toString())
            }.getOrNull()
        }
    }

    val ready = !selection.isEmpty && (preview?.items ?: 0) > 0 &&
        (!kind.archive || !encrypt || (passphraseLongEnough(passphrase) && passphrase == confirm))

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        SettingsTopBar(
            title = stringResource(R.string.export_title),
            onBack = {
                finish()
                onBack()
            },
        )
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            val running = progress
            if (running != null) {
                RunningExport(
                    progress = running,
                    onCancel = { open.cancelExport(running.id) },
                    onSave = {
                        val name = File(running.summary?.path ?: "export.zip").name
                        saveLauncher.launch(name)
                    },
                    onShare = {
                        val path = running.summary?.path ?: return@RunningExport
                        val file = File(path)
                        val share = ShareFiles(folder = file.parentFile ?: file, files = listOf(file))
                        context.startActivity(ShareLauncher.intent(context, share))
                    },
                    onAnother = { finish() },
                )
            } else {
                SwitchRow(
                    title = stringResource(R.string.export_whole),
                    checked = whole,
                    onCheckedChange = { whole = it },
                )
                Text(
                    text = stringResource(R.string.export_whole_hint),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                )

                if (!whole) {
                    SectionHeader(stringResource(R.string.export_folders))
                    for (entry in open.pickableFolders) {
                        CheckRow(
                            title = entry.folder.name,
                            depth = entry.depth,
                            checked = folders.contains(entry.folder.id),
                            onClick = {
                                folders = if (folders.contains(entry.folder.id)) {
                                    folders - entry.folder.id
                                } else {
                                    folders + entry.folder.id
                                }
                            },
                        )
                    }
                    SwitchRow(
                        title = stringResource(R.string.export_subfolders),
                        checked = includeSubfolders,
                        onCheckedChange = { includeSubfolders = it },
                    )

                    SectionHeader(stringResource(R.string.export_tags))
                    for (tag in vocabulary) {
                        CheckRow(
                            title = tag.name,
                            depth = 0,
                            checked = tags.contains(tag.name),
                            onClick = {
                                tags = if (tags.contains(tag.name)) {
                                    tags - tag.name
                                } else {
                                    tags + tag.name
                                }
                            },
                        )
                    }
                }

                SectionHeader(stringResource(R.string.export_format))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (choice in ExportKind.entries) {
                        TextButton(onClick = { kind = choice }) {
                            Text(
                                text = stringResource(choice.label),
                                color = if (kind == choice) EglTokens.Primary else EglTokens.Foreground,
                            )
                        }
                    }
                }
                if (kind.archive) {
                    SectionHeader(stringResource(R.string.export_compression))
                    for (level in ExportCompression.entries) {
                        CheckRow(
                            title = compressionLabel(level),
                            depth = 0,
                            checked = compression == level,
                            onClick = { compression = level },
                        )
                    }
                    SwitchRow(
                        title = stringResource(R.string.export_encrypt),
                        checked = encrypt,
                        onCheckedChange = { encrypt = it },
                    )
                    if (encrypt && kind == ExportKind.ZIP) {
                        SectionHeader(stringResource(R.string.export_protection))
                        CheckRow(
                            title = stringResource(R.string.export_protection_age),
                            depth = 0,
                            checked = zipEncryption == ZipEncryption.AGE,
                            onClick = { zipEncryption = ZipEncryption.AGE },
                        )
                        CheckRow(
                            title = stringResource(R.string.export_protection_aes),
                            depth = 0,
                            checked = zipEncryption == ZipEncryption.AES,
                            onClick = { zipEncryption = ZipEncryption.AES },
                        )
                    }
                    if (encrypt) {
                        OutlinedTextField(
                            value = passphrase,
                            onValueChange = { passphrase = it },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                            visualTransformation = PasswordVisualTransformation(),
                            label = { Text(stringResource(R.string.export_passphrase)) },
                        )
                        OutlinedTextField(
                            value = confirm,
                            onValueChange = { confirm = it },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                            visualTransformation = PasswordVisualTransformation(),
                            label = { Text(stringResource(R.string.export_confirm_passphrase)) },
                        )
                        val short = passphrase.isNotEmpty() && !passphraseLongEnough(passphrase)
                        Text(
                            text = if (short) {
                                stringResource(R.string.export_passphrase_too_short, MIN_PASSPHRASE_CHARS)
                            } else {
                                stringResource(
                                    when {
                                        kind == ExportKind.SEVEN_ZIP -> R.string.export_seven_zip_hint
                                        zipEncryption == ZipEncryption.AES -> R.string.export_aes_hint
                                        else -> R.string.export_encrypt_hint
                                    },
                                )
                            },
                            color = if (short) MaterialTheme.colorScheme.error else EglTokens.MutedForeground,
                            fontSize = 12.sp,
                        )
                    }
                }

                val currentPreview = preview
                if (currentPreview != null) {
                    val size = formatBytes(currentPreview.bytes)
                    Text(
                        text = pluralStringResource(
                            R.plurals.export_summary,
                            currentPreview.items,
                            currentPreview.items,
                        ) + " · $size",
                        color = EglTokens.Foreground,
                        fontSize = 14.sp,
                    )
                    if (currentPreview.hidden > 0) {
                        Text(
                            text = pluralStringResource(
                                R.plurals.export_hidden,
                                currentPreview.hidden,
                                currentPreview.hidden,
                            ),
                            color = EglTokens.MutedForeground,
                            fontSize = 12.sp,
                        )
                    }
                }

                TextButton(
                    onClick = {
                        scope.launch {
                            val dest = File(
                                context.cacheDir,
                                "export-${UUID.randomUUID()}",
                            ).also { it.mkdirs() }
                            folder = dest
                            val request = ExportRequest(
                                selection = selection,
                                includeSubfolders = includeSubfolders,
                                format = kind.format(compression, zipEncryption),
                                destination = dest.absolutePath,
                                encrypt = if (kind.archive && encrypt) passphrase else null,
                            )
                            passphrase = ""
                            confirm = ""
                            val started = open.startExport(request)
                            if (started != null) {
                                exportId = started.id
                            } else {
                                clean()
                            }
                        }
                    },
                    enabled = ready,
                ) {
                    Text(stringResource(R.string.export_start))
                }
            }
        }
    }
}

@Composable
private fun RunningExport(
    progress: ExportProgress,
    onCancel: () -> Unit,
    onSave: () -> Unit,
    onShare: () -> Unit,
    onAnother: () -> Unit,
) {
    when (progress.state) {
        ExportState.PREPARING, ExportState.COPYING, ExportState.ENCRYPTING -> {
            Text(
                text = when (progress.state) {
                    ExportState.PREPARING -> stringResource(R.string.export_preparing)
                    ExportState.ENCRYPTING -> stringResource(R.string.export_encrypting)
                    else -> stringResource(R.string.export_copying)
                },
                color = EglTokens.Foreground,
            )
            val total = progress.total.coerceAtLeast(1)
            LinearProgressIndicator(
                progress = { progress.done.toFloat() / total.toFloat() },
                modifier = Modifier.fillMaxWidth(),
            )
            Text(
                text = "${progress.done} / ${progress.total}",
                color = EglTokens.MutedForeground,
                fontSize = 12.sp,
            )
            TextButton(onClick = onCancel) {
                Text(stringResource(R.string.export_cancel))
            }
        }
        ExportState.DONE -> {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(Icons.Default.CheckCircle, contentDescription = null, tint = Color(0xFF15803D))
                Text(stringResource(R.string.export_done), color = Color(0xFF15803D))
            }
            progress.summary?.let { summary ->
                val size = formatBytes(summary.bytes)
                Text(
                    text = pluralStringResource(R.plurals.export_written, summary.items, summary.items) +
                        " · $size",
                )
                if (summary.missing.isNotEmpty()) {
                    Text(
                        text = pluralStringResource(
                            R.plurals.export_missing,
                            summary.missing.size,
                            summary.missing.size,
                        ),
                        color = Color(0xFFB45309),
                        fontSize = 12.sp,
                    )
                }
                TextButton(onClick = onSave) {
                    Icon(Icons.Default.Folder, contentDescription = null)
                    Text(
                        text = stringResource(R.string.export_save_to_files),
                        modifier = Modifier.padding(start = 8.dp),
                    )
                }
                TextButton(onClick = onShare) {
                    Text(stringResource(R.string.action_share))
                }
            }
            TextButton(onClick = onAnother) {
                Text(stringResource(R.string.export_another))
            }
        }
        ExportState.FAILED -> {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(Icons.Default.Error, contentDescription = null, tint = Color(0xFFB91C1C))
                Text(stringResource(R.string.export_failed), color = Color(0xFFB91C1C))
            }
            if (!progress.error.isNullOrEmpty()) {
                Text(text = progress.error!!, color = EglTokens.MutedForeground, fontSize = 12.sp)
            }
            TextButton(onClick = onAnother) {
                Text(stringResource(R.string.export_another))
            }
        }
        ExportState.CANCELLED -> {
            Text(stringResource(R.string.export_cancelled))
            TextButton(onClick = onAnother) {
                Text(stringResource(R.string.export_another))
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
        verticalAlignment = Alignment.CenterVertically,
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

@Composable
private fun CheckRow(
    title: String,
    depth: Int,
    checked: Boolean,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(start = (depth * 16).dp, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(text = title, color = EglTokens.Foreground, fontSize = 14.sp, modifier = Modifier.weight(1f))
        if (checked) {
            Icon(Icons.Default.Check, contentDescription = null, tint = EglTokens.Primary)
        }
    }
}

/** What the export writes. */
internal enum class ExportKind(@StringRes val label: Int, val archive: Boolean) {
    ZIP(R.string.export_zip, true),
    SEVEN_ZIP(R.string.export_seven_zip, true),
    FOLDER(R.string.export_folder, false),
    ;

    fun format(compression: ExportCompression, encryption: ZipEncryption): ExportFormat =
        when (this) {
            ZIP -> ExportFormat.Zip(compression, encryption)
            SEVEN_ZIP -> ExportFormat.SevenZip(compression)
            FOLDER -> ExportFormat.Mirror
        }
}

@Composable
private fun compressionLabel(level: ExportCompression): String =
    stringResource(
        when (level) {
            ExportCompression.NONE -> R.string.export_compression_none
            ExportCompression.FAST -> R.string.export_compression_fast
            ExportCompression.BALANCED -> R.string.export_compression_balanced
            ExportCompression.SMALLEST -> R.string.export_compression_smallest
        },
    )

private fun finishExport(
    open: OpenLibrary,
    exportId: Long?,
    folder: File?,
    apply: (Long?, File?) -> Unit,
) {
    folder?.deleteRecursively()
    exportId?.let { open.forgetExport(it) }
    apply(null, null)
}
