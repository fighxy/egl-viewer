package app.eglviewer.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.MoreHoriz
import androidx.compose.material.icons.filled.Tag
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.bridge.ItemCellDto
import app.eglviewer.bridge.LibraryBridge
import app.eglviewer.ui.theme.EglTokens

private enum class Place(val label: String) {
    All("All"),
    Uncategorized("Uncategorized"),
    Untagged("Untagged"),
    Trash("Trash"),
}

private enum class LibraryTab(val label: String, val icon: ImageVector) {
    Grid("Grid", Icons.Default.GridView),
    Places("Places", Icons.Default.Folder),
    Tags("Tags", Icons.Default.Tag),
    More("More", Icons.Default.MoreHoriz),
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BrowseScreen(
    bridge: LibraryBridge,
    onBackToLibraries: () -> Unit,
    onOpenPreview: (String) -> Unit,
) {
    var place by remember { mutableStateOf(Place.All) }
    var showFilters by remember { mutableStateOf(false) }
    var tab by remember { mutableStateOf(LibraryTab.Grid) }

    Scaffold(
        containerColor = EglTokens.Background,
        topBar = {
            TopAppBar(
                title = {
                    Text(bridge.libraryName, fontSize = EglTokens.TitleSp, fontWeight = FontWeight.SemiBold)
                },
                navigationIcon = {
                    IconButton(onClick = onBackToLibraries) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Libraries")
                    }
                },
                actions = {
                    IconButton(onClick = { showFilters = !showFilters }) {
                        Icon(Icons.Default.FilterList, contentDescription = "Filters")
                    }
                    IconButton(onClick = {}) {
                        Icon(Icons.Default.Add, contentDescription = "Add")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = EglTokens.Background),
            )
        },
        bottomBar = {
            Column {
                if (bridge.usingMockData) {
                    Text(
                        text = "Mock data — drop libegl_library.so to use a real .eglib",
                        color = EglTokens.MutedForeground,
                        fontSize = EglTokens.ChromeSp,
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(EglTokens.Muted)
                            .padding(vertical = 6.dp, horizontal = 12.dp),
                    )
                }
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(EglTokens.Sidebar)
                        .border(width = 1.dp, color = EglTokens.Border)
                        .padding(vertical = 8.dp),
                ) {
                    LibraryTab.entries.forEach { entry ->
                        Column(
                            modifier = Modifier
                                .weight(1f)
                                .clickable { tab = entry },
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            Icon(
                                entry.icon,
                                contentDescription = entry.label,
                                tint = if (tab == entry) EglTokens.Foreground else EglTokens.MutedForeground,
                            )
                            Text(
                                entry.label,
                                fontSize = 11.sp,
                                color = if (tab == entry) EglTokens.Foreground else EglTokens.MutedForeground,
                            )
                        }
                    }
                }
            }
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            PlacesStrip(place = place, total = bridge.total, onSelect = { place = it })
            if (showFilters) {
                FilterBar()
            }
            when (tab) {
                LibraryTab.Grid -> ItemGrid(items = bridge.items, onOpen = onOpenPreview)
                LibraryTab.Places -> SimpleList("Places", bridge.folders.map { it.name })
                LibraryTab.Tags -> SimpleList("Tags", bridge.tags.map { it.name })
                LibraryTab.More -> SimpleList(
                    "More",
                    listOf(
                        if (bridge.info?.readOnly == true) "Read-only" else "Writable",
                        "Library info",
                        "Settings",
                    ),
                )
            }
        }
    }
}

@Composable
private fun PlacesStrip(place: Place, total: Int, onSelect: (Place) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(EglTokens.Sidebar)
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Place.entries.forEach { chip ->
            val label = if (chip == Place.All) "All · $total" else chip.label
            PlaceChip(label = label, active = place == chip) { onSelect(chip) }
        }
        PlaceChip(label = "Clients ▾", active = false) {}
    }
}

@Composable
private fun PlaceChip(label: String, active: Boolean, onClick: () -> Unit) {
    Text(
        text = label,
        color = if (active) Color.White else EglTokens.Foreground,
        fontSize = EglTokens.ChromeSp,
        modifier = Modifier
            .background(
                color = if (active) EglTokens.Foreground else Color.Transparent,
                shape = RoundedCornerShape(6.dp),
            )
            .border(
                width = if (active) 0.dp else 1.dp,
                color = EglTokens.Border,
                shape = RoundedCornerShape(6.dp),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 6.dp),
    )
}

@Composable
private fun FilterBar() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        listOf("Tags" to true, "Folders" to false, "Ext" to false, "Rating" to false, "Shape" to false)
            .forEach { (title, active) ->
                Text(
                    text = title,
                    color = if (active) Color.White else EglTokens.Foreground,
                    fontSize = EglTokens.ChromeSp,
                    modifier = Modifier
                        .background(
                            if (active) EglTokens.Foreground else Color.Transparent,
                            RoundedCornerShape(50),
                        )
                        .border(1.dp, EglTokens.Border, RoundedCornerShape(50))
                        .padding(horizontal = 10.dp, vertical = 6.dp),
                )
            }
    }
}

@Composable
private fun ItemGrid(items: List<ItemCellDto>, onOpen: (String) -> Unit) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(96.dp),
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Grid),
        contentPadding = PaddingValues(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        items(items, key = { it.id }) { item ->
            val ratio = run {
                val w = (item.width ?: 1).toFloat()
                val h = (item.height ?: 1).toFloat().coerceAtLeast(1f)
                (w / h).coerceAtLeast(0.55f)
            }
            Box(
                modifier = Modifier
                    .aspectRatio(ratio)
                    .background(EglTokens.Muted)
                    .clickable { onOpen(item.id) },
                contentAlignment = Alignment.BottomStart,
            ) {
                Text(
                    text = item.ext.uppercase(),
                    color = EglTokens.MutedForeground,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(6.dp),
                )
            }
        }
    }
}

@Composable
private fun SimpleList(title: String, rows: List<String>) {
    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, fontSize = EglTokens.ChromeSp, color = EglTokens.MutedForeground)
        rows.forEach { row ->
            Text(row, fontSize = EglTokens.ChromeSp, color = EglTokens.Foreground)
        }
    }
}
