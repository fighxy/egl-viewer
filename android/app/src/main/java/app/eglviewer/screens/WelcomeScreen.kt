package app.eglviewer.screens

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.bridge.LibraryBridge
import app.eglviewer.bridge.RecentLibrary
import app.eglviewer.theme.EglTokens

@Composable
fun WelcomeScreen(
    bridge: LibraryBridge,
    onOpenBrowse: () -> Unit,
) {
    val context = LocalContext.current
    LaunchedEffect(Unit) {
        bridge.attach(context)
    }

    val openTree = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree(),
    ) { uri: Uri? ->
        if (uri != null) {
            bridge.openUri(uri)
            onOpenBrowse()
        }
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
                text = "egl-viewer",
                color = EglTokens.Foreground,
                fontSize = EglTokens.HeroSp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = (-0.5).sp,
            )
            Text(
                text = buildAnnotatedString {
                    append("Open a native library on this phone. Libraries live as ")
                    withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = EglTokens.Foreground)) {
                        append(".eglib")
                    }
                    append(" packages in Files or storage.")
                },
                color = EglTokens.MutedForeground,
                fontSize = 14.sp,
                lineHeight = 20.sp,
            )
        }

        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Button(
                onClick = {
                    bridge.createLibrary("Demo")
                    onOpenBrowse()
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .height(EglTokens.ButtonHeight),
                shape = RoundedCornerShape(EglTokens.RadiusLg),
                colors = ButtonDefaults.buttonColors(
                    containerColor = EglTokens.Primary,
                    contentColor = EglTokens.PrimaryForeground,
                ),
            ) {
                Icon(Icons.Default.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(modifier = Modifier.size(8.dp))
                Text("Create library", fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
            }
            OutlinedButton(
                onClick = { openTree.launch(null) },
                modifier = Modifier
                    .fillMaxWidth()
                    .height(EglTokens.ButtonHeight),
                shape = RoundedCornerShape(EglTokens.RadiusLg),
                colors = ButtonDefaults.outlinedButtonColors(contentColor = EglTokens.Foreground),
                border = BorderStroke(1.dp, EglTokens.Border),
            ) {
                Icon(Icons.Default.FolderOpen, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(modifier = Modifier.size(8.dp))
                Text("Open .eglib…", fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
            }
        }

        Column {
            Text(
                text = "Recent",
                color = EglTokens.MutedForeground,
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(bottom = 8.dp, top = 4.dp),
            )
            if (bridge.recents.isEmpty()) {
                Text("No recent libraries yet.", color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
            }
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                bridge.recents.forEachIndexed { index, recent ->
                    RecentRow(
                        recent = recent,
                        active = index == 0 && !recent.missing,
                        onClick = {
                            bridge.openRecent(recent)
                            onOpenBrowse()
                        },
                    )
                }
            }
        }

        bridge.lastError?.let { error ->
            Text(error, color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
        }

        Spacer(modifier = Modifier.height(24.dp))
        Text(
            text = "Native .eglib only · same product as desktop",
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
                .background(EglTokens.Sidebar, RoundedCornerShape(EglTokens.RadiusSm))
                .border(1.dp, EglTokens.Border, RoundedCornerShape(EglTokens.RadiusSm)),
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
        if (recent.missing) {
            Text("Missing", color = EglTokens.MutedForeground, fontSize = EglTokens.CaptionSp)
        }
    }
}
