package app.eglviewer.settings

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.adding.PickedFile
import app.eglviewer.core.DecryptedArchive
import app.eglviewer.core.Library
import app.eglviewer.details.ShareFiles
import app.eglviewer.details.ShareLauncher
import app.eglviewer.theme.EglTokens
import java.io.File
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Decrypt Archive (D-110 / D-111): opens an encrypted export (`name.zip.age`) with its
 * passphrase. The picked file is copied into the cache (a document has no path the core can
 * read), decrypted next to the copy, then saved where the user chooses or shared. The cache
 * folder goes when the screen closes; the passphrase is never stored.
 */
@Composable
fun DecryptArchiveScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var archive by remember { mutableStateOf<Uri?>(null) }
    var archiveName by remember { mutableStateOf<String?>(null) }
    var passphrase by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var failure by remember { mutableStateOf<String?>(null) }
    var result by remember { mutableStateOf<DecryptedArchive?>(null) }
    var folder by remember { mutableStateOf<File?>(null) }

    fun clean() {
        folder?.deleteRecursively()
        folder = null
    }
    DisposableEffect(Unit) { onDispose { clean() } }

    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            archive = uri
            archiveName = displayName(context, uri)
            failure = null
            result = null
        }
    }
    val save = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/zip"),
    ) { uri ->
        val decrypted = result ?: return@rememberLauncherForActivityResult
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            withContext(Dispatchers.IO) {
                runCatching {
                    context.contentResolver.openOutputStream(uri)?.use { out ->
                        decrypted.file.inputStream().use { it.copyTo(out) }
                    }
                }
            }
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        SettingsTopBar(title = stringResource(R.string.decrypt_title), onBack = onBack)
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = stringResource(R.string.decrypt_hint),
                color = EglTokens.MutedForeground,
                fontSize = 13.sp,
            )
            SettingsRow(
                icon = Icons.Default.Folder,
                title = stringResource(R.string.decrypt_archive),
                trailing = archiveName ?: stringResource(R.string.decrypt_choose),
                onClick = { pick.launch(arrayOf("*/*")) },
            )
            OutlinedTextField(
                value = passphrase,
                onValueChange = { passphrase = it },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                label = { Text(stringResource(R.string.export_passphrase)) },
                modifier = Modifier.fillMaxWidth(),
            )
            if (busy) {
                Text(stringResource(R.string.decrypt_decrypting))
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            } else {
                Button(
                    enabled = archive != null && passphrase.isNotEmpty(),
                    onClick = {
                        val source = archive ?: return@Button
                        val name = archiveName ?: "archive.zip.age"
                        val secret = passphrase
                        busy = true
                        failure = null
                        result = null
                        clean()
                        scope.launch {
                            val target = File(context.cacheDir, "decrypt-${UUID.randomUUID()}")
                            val outcome = withContext(Dispatchers.IO) {
                                runCatching {
                                    target.mkdirs()
                                    val copy = PickedFile.child(target, name, fallback = "archive.zip.age")
                                    context.contentResolver.openInputStream(source)?.use { input ->
                                        copy.outputStream().use { input.copyTo(it) }
                                    } ?: throw java.io.IOException("cannot read the archive")
                                    val out = File(target, "out").also { it.mkdirs() }
                                    try {
                                        Library.decryptArchive(copy, out, secret)
                                    } finally {
                                        copy.delete()
                                    }
                                }
                            }
                            busy = false
                            outcome
                                .onSuccess {
                                    folder = target
                                    result = it
                                    passphrase = ""
                                }
                                .onFailure {
                                    target.deleteRecursively()
                                    failure = it.message ?: it.toString()
                                }
                        }
                    },
                ) {
                    Text(stringResource(R.string.decrypt_start))
                }
            }
            failure?.let {
                Text(text = it, color = MaterialTheme.colorScheme.error, fontSize = 13.sp)
            }
            result?.let { decrypted ->
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(Icons.Default.CheckCircle, contentDescription = null, tint = Color(0xFF15803D))
                    Text("${decrypted.file.name} · ${formatBytes(decrypted.bytes)}")
                }
                TextButton(onClick = { save.launch(decrypted.file.name) }) {
                    Text(stringResource(R.string.export_save_to_files))
                }
                TextButton(
                    onClick = {
                        val share = ShareFiles(folder = decrypted.file.parentFile ?: decrypted.file, files = listOf(decrypted.file))
                        context.startActivity(ShareLauncher.intent(context, share))
                    },
                ) {
                    Text(stringResource(R.string.action_share))
                }
            }
        }
    }
}

private fun displayName(context: Context, uri: Uri): String? =
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        }
