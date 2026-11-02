package app.eglviewer.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckBox
import androidx.compose.material.icons.filled.CheckBoxOutlineBlank
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.StarOutline
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.eglviewer.bridge.FolderDto
import app.eglviewer.bridge.LibraryBridge
import app.eglviewer.ui.theme.EglTokens

@Composable
fun EditSheet(
    bridge: LibraryBridge,
    itemId: String,
    onDismiss: () -> Unit,
) {
    val detail = bridge.itemDetail(itemId)
    var segment by remember { mutableIntStateOf(0) }
    var draftName by remember(detail?.name) { mutableStateOf(detail?.name.orEmpty()) }
    var newTag by remember { mutableStateOf("") }
    val segments = listOf("Details", "Tags sheet", "Folders")

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(bottom = 24.dp),
    ) {
        Text(
            text = "Edit",
            fontSize = EglTokens.TitleSp,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
        )
        TabRow(
            selectedTabIndex = segment,
            containerColor = EglTokens.Background,
            contentColor = EglTokens.Foreground,
        ) {
            segments.forEachIndexed { index, title ->
                Tab(
                    selected = segment == index,
                    onClick = { segment = index },
                    text = { Text(title, fontSize = EglTokens.ChromeSp) },
                )
            }
        }

        when (segment) {
            0 -> {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    OutlinedTextField(
                        value = draftName,
                        onValueChange = { draftName = it },
                        label = { Text("Name") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        val star = detail?.star ?: 0
                        (1..5).forEach { n ->
                            Icon(
                                imageVector = if (n <= star) Icons.Default.Star else Icons.Outlined.StarOutline,
                                contentDescription = null,
                                tint = EglTokens.Foreground,
                            )
                        }
                    }
                    Text("Tags", color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
                    (detail?.tags.orEmpty()).forEach { tag ->
                        Text(
                            text = tag,
                            fontSize = EglTokens.ChromeSp,
                            modifier = Modifier
                                .background(EglTokens.Muted, RoundedCornerShape(4.dp))
                                .border(1.dp, EglTokens.Border, RoundedCornerShape(4.dp))
                                .padding(horizontal = 8.dp, vertical = 4.dp),
                        )
                    }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        OutlinedTextField(
                            value = newTag,
                            onValueChange = { newTag = it },
                            label = { Text("Add tag") },
                            modifier = Modifier.weight(1f),
                        )
                        TextButton(onClick = {
                            val tag = newTag.trim()
                            if (tag.isNotEmpty()) {
                                bridge.applyAddTag(itemId, tag)
                                newTag = ""
                            }
                        }) {
                            Text("Add", fontSize = EglTokens.ChromeSp)
                        }
                    }
                    Text("Folders", color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
                    (detail?.folders.orEmpty()).forEach { folderId ->
                        Text(folderName(bridge.folders, folderId), fontSize = EglTokens.ChromeSp)
                    }
                }
            }
            1 -> {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        "Match mode: Any · All · None (stub)",
                        color = EglTokens.MutedForeground,
                        fontSize = EglTokens.ChromeSp,
                    )
                    Text("Vocabulary", color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
                    bridge.tags.forEach { tag ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(tag.name, fontSize = EglTokens.ChromeSp)
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                tag.imageCount?.let {
                                    Text(
                                        "$it",
                                        color = EglTokens.MutedForeground,
                                        fontSize = EglTokens.ChromeSp,
                                        modifier = Modifier.padding(end = 8.dp),
                                    )
                                }
                                TextButton(onClick = { bridge.applyAddTag(itemId, tag.name) }) {
                                    Text("Add", fontSize = EglTokens.ChromeSp)
                                }
                            }
                        }
                    }
                }
            }
            else -> {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    FolderRows(
                        folders = bridge.folders,
                        selected = detail?.folders.orEmpty().toSet(),
                        depth = 0,
                    )
                }
            }
        }

        TextButton(
            onClick = onDismiss,
            modifier = Modifier.align(Alignment.End).padding(horizontal = 8.dp),
        ) {
            Text("Close", fontSize = EglTokens.ChromeSp)
        }
    }
}

@Composable
private fun FolderRows(folders: List<FolderDto>, selected: Set<String>, depth: Int) {
    folders.forEach { folder ->
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = (depth * 12).dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(folder.name, fontSize = EglTokens.ChromeSp)
            Icon(
                imageVector = if (folder.id in selected) {
                    Icons.Default.CheckBox
                } else {
                    Icons.Default.CheckBoxOutlineBlank
                },
                contentDescription = null,
                tint = EglTokens.Foreground,
            )
        }
        FolderRows(folder.children, selected, depth + 1)
    }
}

private fun folderName(nodes: List<FolderDto>, id: String): String {
    fun search(list: List<FolderDto>): String? {
        list.forEach { node ->
            if (node.id == id) return node.name
            search(node.children)?.let { return it }
        }
        return null
    }
    return search(nodes) ?: id
}
