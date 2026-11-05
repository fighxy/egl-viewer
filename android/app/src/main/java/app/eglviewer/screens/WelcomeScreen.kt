package app.eglviewer.screens

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.AppModel
import app.eglviewer.app.RecentLibrary
import app.eglviewer.theme.EglTokens
import java.io.File

@Composable
fun WelcomeScreen(model: AppModel) {
    var showCreate by remember { mutableStateOf(false) }
    var createName by remember { mutableStateOf("") }

    val openTree = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree(),
    ) { uri: Uri? ->
        if (uri != null) {
            model.openUri(uri)
        }
    }

    model.error?.let { message ->
        AlertDialog(
            onDismissRequest = model::dismissError,
            title = { Text(stringResource(R.string.error_title)) },
            text = { Text(message) },
            confirmButton = {
                TextButton(onClick = model::dismissError) {
                    Text(stringResource(android.R.string.ok))
                }
            },
        )
    }

    if (showCreate) {
        AlertDialog(
            onDismissRequest = { showCreate = false },
            title = { Text(stringResource(R.string.create_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.create_hint), color = EglTokens.MutedForeground)
                    OutlinedTextField(
                        value = createName,
                        onValueChange = { createName = it },
                        singleLine = true,
                        label = { Text(stringResource(R.string.create_name)) },
                    )
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        showCreate = false
                        model.create(createName)
                        createName = ""
                    },
                ) {
                    Text(stringResource(R.string.create_confirm))
                }
            },
            dismissButton = {
                TextButton(onClick = { showCreate = false }) {
                    Text(stringResource(android.R.string.cancel))
                }
            },
        )
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp)
            .padding(top = 36.dp, bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.app_name),
                color = EglTokens.Foreground,
                fontSize = EglTokens.HeroSp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = (-0.5).sp,
            )
            Text(
                text = buildAnnotatedString {
                    append(stringResource(R.string.welcome_hint_before))
                    withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = EglTokens.Foreground)) {
                        append(".eglib")
                    }
                    append(stringResource(R.string.welcome_hint_after))
                },
                color = EglTokens.MutedForeground,
                fontSize = 14.sp,
                lineHeight = 20.sp,
            )
            if (!model.nativeLinked) {
                Text(
                    text = stringResource(R.string.error_core_missing),
                    color = EglTokens.MutedForeground,
                    fontSize = EglTokens.ChromeSp,
                )
            }
        }

        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Button(
                onClick = {
                    createName = ""
                    showCreate = true
                },
                enabled = !model.busy,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(EglTokens.ButtonHeight),
                shape = RoundedCornerShape(EglTokens.RadiusLg),
                colors = ButtonDefaults.buttonColors(
                    containerColor = EglTokens.Primary,
                    contentColor = EglTokens.PrimaryForeground,
                ),
            ) {
                if (model.busy) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(18.dp),
                        color = EglTokens.PrimaryForeground,
                        strokeWidth = 2.dp,
                    )
                } else {
                    Icon(Icons.Default.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                }
                Spacer(modifier = Modifier.size(8.dp))
                Text(stringResource(R.string.welcome_create), fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
            }
            OutlinedButton(
                onClick = { openTree.launch(null) },
                enabled = !model.busy,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(EglTokens.ButtonHeight),
                shape = RoundedCornerShape(EglTokens.RadiusLg),
                colors = ButtonDefaults.outlinedButtonColors(contentColor = EglTokens.Foreground),
                border = BorderStroke(1.dp, EglTokens.Border),
            ) {
                Icon(Icons.Default.FolderOpen, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(modifier = Modifier.size(8.dp))
                Text(stringResource(R.string.welcome_open), fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
            }
        }

        if (model.localLibraries.isNotEmpty()) {
            Column {
                Text(
                    text = stringResource(R.string.welcome_on_this_phone),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(bottom = 8.dp, top = 4.dp),
                )
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    model.localLibraries.forEach { file: File ->
                        RecentRow(
                            recent = RecentLibrary(
                                name = file.name.removeSuffix(".eglib"),
                                path = file.absolutePath,
                                locationHint = stringResource(R.string.hint_app_storage),
                            ),
                            active = false,
                            onClick = { model.openLocal(file) },
                            onForget = null,
                        )
                    }
                }
            }
        }

        Column {
            Text(
                text = stringResource(R.string.welcome_recent),
                color = EglTokens.MutedForeground,
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(bottom = 8.dp, top = 4.dp),
            )
            if (model.recents.isEmpty()) {
                Text(
                    text = stringResource(R.string.welcome_recent_empty),
                    color = EglTokens.MutedForeground,
                    fontSize = EglTokens.ChromeSp,
                )
            }
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                model.recents.forEachIndexed { index, recent ->
                    RecentRow(
                        recent = recent,
                        active = index == 0 && !recent.missing,
                        onClick = { model.openRecent(recent) },
                        onForget = { model.forget(recent) },
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(24.dp))
        Text(
            text = stringResource(R.string.welcome_footer),
            color = EglTokens.MutedForeground,
            fontSize = EglTokens.CaptionSp,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

@Composable
private fun RecentRow(
    recent: RecentLibrary,
    active: Boolean,
    onClick: () -> Unit,
    onForget: (() -> Unit)?,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(
                color = if (active) EglTokens.Muted else Color.Transparent,
                shape = RoundedCornerShape(EglTokens.RadiusMd),
            )
            .clickable(enabled = !recent.missing, onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(
            modifier = Modifier
                .size(36.dp)
                .background(EglTokens.Sidebar, RoundedCornerShape(EglTokens.RadiusSm)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                Icons.Default.GridView,
                contentDescription = null,
                tint = EglTokens.MutedForeground.copy(alpha = 0.7f),
                modifier = Modifier.size(16.dp),
            )
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = recent.name,
                color = if (recent.missing) EglTokens.MutedForeground else EglTokens.Foreground,
                fontSize = 14.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = recent.locationHint,
                color = EglTokens.MutedForeground,
                fontSize = EglTokens.CaptionSp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        when {
            recent.missing -> {
                Text(
                    text = stringResource(R.string.welcome_missing),
                    color = EglTokens.MutedForeground,
                    fontSize = EglTokens.CaptionSp,
                )
            }
            onForget != null -> {
                TextButton(onClick = onForget) {
                    Text(stringResource(R.string.welcome_forget), fontSize = EglTokens.CaptionSp)
                }
            }
        }
    }
}
