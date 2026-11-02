package app.eglviewer.screens

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.eglviewer.bridge.LibraryBridge
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
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 24.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = "egl-viewer",
                color = EglTokens.Foreground,
                fontSize = EglTokens.HeroSp,
                fontWeight = FontWeight.SemiBold,
            )
            Text(
                text = "Open a native .eglib library from Files or storage.",
                color = EglTokens.MutedForeground,
                fontSize = EglTokens.ChromeSp,
            )
        }

        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Button(
                onClick = {
                    bridge.createLibrary("Demo")
                    onOpenBrowse()
                },
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.buttonColors(
                    containerColor = EglTokens.Foreground,
                    contentColor = EglTokens.Background,
                ),
            ) {
                Text("Create library", fontSize = EglTokens.TitleSp, fontWeight = FontWeight.SemiBold)
            }
            OutlinedButton(
                onClick = { openTree.launch(null) },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Open .eglib…", fontSize = EglTokens.TitleSp, fontWeight = FontWeight.SemiBold)
            }
        }

        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Recent", color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
            if (bridge.recents.isEmpty()) {
                Text(
                    "No recent libraries yet.",
                    color = EglTokens.MutedForeground,
                    fontSize = EglTokens.ChromeSp,
                )
            }
            bridge.recents.forEach { recent ->
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .border(1.dp, EglTokens.Border)
                        .clickable(enabled = !recent.missing) {
                            bridge.openRecent(recent)
                            onOpenBrowse()
                        }
                        .padding(12.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column {
                        Text(
                            text = recent.name,
                            color = if (recent.missing) EglTokens.MutedForeground else EglTokens.Foreground,
                            fontSize = EglTokens.TitleSp,
                            fontWeight = FontWeight.SemiBold,
                        )
                        Text(recent.locationHint, color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
                    }
                    if (recent.missing) {
                        Text("Missing", color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
                    }
                }
            }
        }

        bridge.lastError?.let { error ->
            Text(error, color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
        }

        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Native .eglib only - no Eagle on this client.",
            color = EglTokens.MutedForeground,
            fontSize = EglTokens.ChromeSp,
        )
    }
}
