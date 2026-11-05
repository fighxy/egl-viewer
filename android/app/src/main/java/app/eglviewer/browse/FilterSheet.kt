package app.eglviewer.browse

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.core.TagMode
import app.eglviewer.theme.EglTokens

/** Filters of a place: tags (any / all / none), file type, rating and shape. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FilterSheet(
    model: BrowseModel,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var tagSearch by remember { mutableStateOf("") }

    LaunchedEffect(model) {
        model.wantsVocabulary = true
        model.loadVocabulary()
    }

    ModalBottomSheet(
        onDismissRequest = {
            model.wantsVocabulary = false
            onDismiss()
        },
        sheetState = sheetState,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp)
                .padding(bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(
                    onClick = { model.resetFilters() },
                    enabled = model.settings.filters.isActive,
                ) {
                    Text("Reset")
                }
                Text("Filters", fontWeight = FontWeight.SemiBold, fontSize = 18.sp)
                TextButton(onClick = {
                    model.wantsVocabulary = false
                    onDismiss()
                }) {
                    Text("Done")
                }
            }

            if (model.place.fixedTag == null) {
                Text("Tags", color = EglTokens.MutedForeground, fontSize = 13.sp)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TagMode.entries.forEach { mode ->
                        FilterChip(
                            selected = model.settings.filters.tagMode == mode,
                            onClick = {
                                model.change { it.copy(filters = it.filters.copy(tagMode = mode)) }
                            },
                            label = { Text(mode.name.lowercase()) },
                        )
                    }
                }
                val needle = tagSearch.trim()
                val shown = if (needle.isEmpty()) {
                    model.vocabulary
                } else {
                    model.vocabulary.filter {
                        it.name.contains(needle, ignoreCase = true) ||
                            it.name in model.settings.filters.tags
                    }
                }
                if (shown.isEmpty()) {
                    Text("No tags", color = EglTokens.MutedForeground)
                } else {
                    shown.forEach { tag ->
                        val selected = tag.name in model.settings.filters.tags
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { model.toggleTag(tag.name) }
                                .padding(vertical = 8.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Text(tag.name, color = EglTokens.Foreground)
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                tag.imageCount?.let {
                                    Text(it.toString(), color = EglTokens.MutedForeground)
                                }
                                if (selected) {
                                    Icon(Icons.Default.Check, contentDescription = "Selected")
                                }
                            }
                        }
                    }
                }
            }

            Text("Type", color = EglTokens.MutedForeground, fontSize = 13.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = model.settings.filters.ext == null,
                    onClick = { model.change { it.copy(filters = it.filters.copy(ext = null)) } },
                    label = { Text("Any") },
                )
                model.extensions.forEach { type ->
                    FilterChip(
                        selected = model.settings.filters.ext == type.ext,
                        onClick = {
                            model.change { it.copy(filters = it.filters.copy(ext = type.ext)) }
                        },
                        label = { Text("${type.ext.uppercase()} (${type.count})") },
                    )
                }
            }

            Text("Rating", color = EglTokens.MutedForeground, fontSize = 13.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = model.settings.filters.rating == null,
                    onClick = { model.change { it.copy(filters = it.filters.copy(rating = null)) } },
                    label = { Text("Any") },
                )
                FilterChip(
                    selected = model.settings.filters.rating == 0,
                    onClick = { model.change { it.copy(filters = it.filters.copy(rating = 0)) } },
                    label = { Text("Unrated") },
                )
                (1..5).forEach { stars ->
                    FilterChip(
                        selected = model.settings.filters.rating == stars,
                        onClick = {
                            model.change { it.copy(filters = it.filters.copy(rating = stars)) }
                        },
                        label = { Text("$stars★") },
                    )
                }
            }

            Text("Shape", color = EglTokens.MutedForeground, fontSize = 13.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = model.settings.filters.shape == null,
                    onClick = { model.change { it.copy(filters = it.filters.copy(shape = null)) } },
                    label = { Text("Any") },
                )
                BrowseFilters.Shape.entries.forEach { shape ->
                    FilterChip(
                        selected = model.settings.filters.shape == shape,
                        onClick = {
                            model.change { it.copy(filters = it.filters.copy(shape = shape)) }
                        },
                        label = { Text(shape.name) },
                    )
                }
            }
        }
    }
}
