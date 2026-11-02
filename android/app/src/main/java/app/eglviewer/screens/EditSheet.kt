package app.eglviewer.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.StarOutline
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.bridge.FolderDto
import app.eglviewer.bridge.LibraryBridge
import app.eglviewer.components.SegmentedControl
import app.eglviewer.theme.EglTokens

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun EditSheet(
    bridge: LibraryBridge,
    itemId: String,
    onDismiss: () -> Unit,
) {
    val detail = bridge.itemDetail(itemId)
    val cell = bridge.items.firstOrNull { it.id == itemId }
    var segment by remember { mutableIntStateOf(0) }
    var draftName by remember(detail?.name) { mutableStateOf(detail?.name.orEmpty()) }
    var addingTag by remember { mutableStateOf(false) }
    var newTag by remember { mutableStateOf("") }
    val segments = listOf("Details", "Tags sheet", "Folders")
    val displayName = run {
        val base = detail?.name ?: cell?.name ?: "Item"
        val ext = (detail?.ext ?: cell?.ext).orEmpty()
        if (ext.isBlank()) base else "$base.$ext"
    }
    val metaHint = buildString {
        val w = cell?.width
        val h = cell?.height
        if (w != null && h != null) append("${w}×${h}")
        val ext = (detail?.ext ?: cell?.ext)?.uppercase().orEmpty()
        if (ext.isNotEmpty()) {
            if (isNotEmpty()) append(" · ")
            append(ext)
        }
        if (isEmpty()) append("Details")
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp)
            .padding(bottom = 28.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(
            modifier = Modifier
                .align(Alignment.CenterHorizontally)
                .padding(top = 2.dp, bottom = 4.dp)
                .width(36.dp)
                .height(5.dp)
                .background(Color(0xFFD4D4D4), RoundedCornerShape(99.dp)),
        )

        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                text = displayName,
                fontSize = 16.sp,
                fontWeight = FontWeight.SemiBold,
                color = EglTokens.Foreground,
            )
            Text(
                text = metaHint,
                fontSize = 12.sp,
                color = EglTokens.MutedForeground,
            )
        }

        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            when (segment) {
                0 -> {
                    FieldLabel("Name")
                    BasicTextField(
                        value = draftName,
                        onValueChange = { draftName = it },
                        singleLine = true,
                        textStyle = TextStyle(fontSize = 14.sp, color = EglTokens.Foreground),
                        modifier = Modifier
                            .fillMaxWidth()
                            .border(1.dp, EglTokens.Border, RoundedCornerShape(EglTokens.RadiusSm))
                            .padding(horizontal = 10.dp, vertical = 8.dp),
                    )

                    FieldLabel("Rating")
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        val star = detail?.star ?: 0
                        (1..5).forEach { n ->
                            Icon(
                                imageVector = if (n <= star) Icons.Default.Star else Icons.Outlined.StarOutline,
                                contentDescription = null,
                                tint = if (n <= star) EglTokens.Foreground else Color(0xFFD4D4D4),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }

                    FieldLabel("Tags")
                    FlowRow(
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        (detail?.tags.orEmpty()).forEach { tag ->
                            Text(
                                text = "$tag  ×",
                                fontSize = 12.sp,
                                modifier = Modifier
                                    .background(EglTokens.Muted, RoundedCornerShape(7.dp))
                                    .padding(horizontal = 10.dp, vertical = 5.dp),
                            )
                        }
                        Text(
                            text = "+ Add tag",
                            fontSize = 12.sp,
                            color = EglTokens.MutedForeground,
                            modifier = Modifier
                                .border(1.dp, EglTokens.Border, RoundedCornerShape(7.dp))
                                .clickable { addingTag = true }
                                .padding(horizontal = 10.dp, vertical = 5.dp),
                        )
                    }
                    if (addingTag) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            BasicTextField(
                                value = newTag,
                                onValueChange = { newTag = it },
                                singleLine = true,
                                textStyle = TextStyle(fontSize = EglTokens.ChromeSp, color = EglTokens.Foreground),
                                modifier = Modifier
                                    .weight(1f)
                                    .border(1.dp, EglTokens.Border, RoundedCornerShape(EglTokens.RadiusSm))
                                    .padding(horizontal = 10.dp, vertical = 8.dp),
                            )
                            TextButton(onClick = {
                                val tag = newTag.trim()
                                if (tag.isNotEmpty()) {
                                    bridge.applyAddTag(itemId, tag)
                                    newTag = ""
                                    addingTag = false
                                }
                            }) {
                                Text("Add", fontSize = EglTokens.ChromeSp)
                            }
                        }
                    }

                    FieldLabel("Folders")
                    FolderRows(
                        folders = bridge.folders,
                        selected = detail?.folders.orEmpty().toSet(),
                        depth = 0,
                        pathPrefix = "",
                    )
                }
                1 -> {
                    FieldLabel("Match mode")
                    Text(
                        "Any · All · None (stub)",
                        color = EglTokens.MutedForeground,
                        fontSize = EglTokens.ChromeSp,
                    )
                    FieldLabel("Vocabulary")
                    bridge.tags.forEach { tag ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { bridge.applyAddTag(itemId, tag.name) }
                                .padding(vertical = 6.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Text(tag.name, fontSize = EglTokens.ChromeSp)
                            tag.imageCount?.let {
                                Text("$it", color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
                            }
                        }
                    }
                }
                else -> {
                    FolderRows(
                        folders = bridge.folders,
                        selected = detail?.folders.orEmpty().toSet(),
                        depth = 0,
                        pathPrefix = "",
                    )
                }
            }
        }

        SegmentedControl(
            options = segments,
            selectedIndex = segment,
            onSelect = { segment = it },
        )

        // Dismiss affordance for TalkBack / non-gesture users (sheet also swipes down).
        TextButton(
            onClick = onDismiss,
            modifier = Modifier.align(Alignment.End),
        ) {
            Text("Done", fontSize = EglTokens.ChromeSp)
        }
    }
}

@Composable
private fun FieldLabel(text: String) {
    Text(text, fontSize = EglTokens.CaptionSp, color = EglTokens.MutedForeground)
}

@Composable
private fun FolderRows(
    folders: List<FolderDto>,
    selected: Set<String>,
    depth: Int,
    pathPrefix: String,
) {
    folders.forEach { folder ->
        val path = if (pathPrefix.isEmpty()) folder.name else "$pathPrefix / ${folder.name}"
        val on = folder.id in selected
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(
                    if (on) EglTokens.Muted else Color.Transparent,
                    RoundedCornerShape(EglTokens.RadiusSm),
                )
                .padding(start = (depth * 8).dp)
                .padding(horizontal = 8.dp)
                .height(34.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(
                Icons.Default.Folder,
                contentDescription = null,
                tint = EglTokens.Foreground,
                modifier = Modifier.size(14.dp),
            )
            Text(
                text = if (depth == 0) folder.name else path,
                fontSize = EglTokens.ChromeSp,
                fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal,
                modifier = Modifier.weight(1f),
            )
            Box(
                modifier = Modifier
                    .size(16.dp)
                    .background(
                        if (on) EglTokens.Primary else Color.Transparent,
                        RoundedCornerShape(4.dp),
                    )
                    .border(
                        width = 1.5.dp,
                        color = if (on) EglTokens.Primary else EglTokens.Ring,
                        shape = RoundedCornerShape(4.dp),
                    ),
            )
        }
        FolderRows(folder.children, selected, depth + 1, path)
    }
}
