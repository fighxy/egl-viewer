package app.eglviewer.screens

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.automirrored.filled.ViewSidebar
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.MoreHoriz
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Tag
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.bridge.ItemCellDto
import app.eglviewer.bridge.LibraryBridge
import app.eglviewer.components.FilterPill
import app.eglviewer.components.PlaceChip
import app.eglviewer.theme.EglTokens


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

@Composable
fun BrowseScreen(
    bridge: LibraryBridge,
    onBackToLibraries: () -> Unit,
    onOpenPreview: (String) -> Unit,
) {
    var place by remember { mutableStateOf(Place.All) }
    var showFilters by remember { mutableStateOf(true) }
    var detailsPressed by remember { mutableStateOf(true) }
    var tab by remember { mutableStateOf(LibraryTab.Grid) }
    var search by remember { mutableStateOf("") }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        BrowseNavBar(
            libraryName = bridge.libraryName.ifBlank { "Library" },
            detailsPressed = detailsPressed,
            onBack = onBackToLibraries,
            onToggleDetails = { detailsPressed = !detailsPressed },
        )
        PlacesStrip(place = place, total = bridge.total, onSelect = { place = it })
        SearchToolbar(
            query = search,
            onQueryChange = { search = it },
            filtersPressed = showFilters,
            onToggleFilters = { showFilters = !showFilters },
        )
        if (showFilters) {
            FilterBar()
        }
        Box(modifier = Modifier.weight(1f)) {
            when (tab) {
                LibraryTab.Grid -> ItemGrid(bridge = bridge, items = bridge.items, onOpen = onOpenPreview)
                LibraryTab.Places -> SimpleList("Places", bridge.folders.map { it.name })
                LibraryTab.Tags -> SimpleList("Tags", bridge.tags.map { it.name })
                LibraryTab.More -> SimpleList(
                    "More",
                    listOf(
                        if (bridge.info?.readOnly == true) "Read-only" else "Writable",
                        if (bridge.ffiLinked) "FFI linked" else "FFI mock",
                        "Library info",
                        "Settings",
                    ),
                )
            }
        }
        if (bridge.usingMockData) {
            Text(
                text = if (bridge.ffiLinked) {
                    "Mock fallback — could not open via FFI"
                } else {
                    "Mock data — drop libegl_library.so to use a real .eglib"
                },
                color = EglTokens.MutedForeground,
                fontSize = EglTokens.ChromeSp,
                modifier = Modifier
                    .fillMaxWidth()
                    .background(EglTokens.Muted)
                    .padding(vertical = 6.dp, horizontal = 12.dp),
            )
        }
        TabBar(tab = tab, onSelect = { tab = it })
    }
}

@Composable
private fun BrowseNavBar(
    libraryName: String,
    detailsPressed: Boolean,
    onBack: () -> Unit,
    onToggleDetails: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(44.dp)
            .padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Libraries")
        }
        Text(
            text = libraryName,
            fontSize = 15.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        IconButton(onClick = {}) {
            Icon(Icons.Default.Add, contentDescription = "Add")
        }
        IconButton(onClick = onToggleDetails) {
            Icon(
                Icons.AutoMirrored.Filled.ViewSidebar,
                contentDescription = "Details",
                tint = if (detailsPressed) EglTokens.Foreground else EglTokens.MutedForeground,
                modifier = Modifier
                    .background(
                        if (detailsPressed) EglTokens.Muted else Color.Transparent,
                        RoundedCornerShape(EglTokens.RadiusSm),
                    )
                    .padding(4.dp),
            )
        }
    }
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(1.dp)
            .background(EglTokens.Border),
    )
}

@Composable
private fun PlacesStrip(place: Place, total: Int, onSelect: (Place) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(EglTokens.Sidebar)
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        PlaceChip(
            label = Place.All.label,
            count = "%,d".format(total),
            active = place == Place.All,
            onClick = { onSelect(Place.All) },
        )
        PlaceChip(
            label = Place.Uncategorized.label,
            count = "42",
            active = place == Place.Uncategorized,
            onClick = { onSelect(Place.Uncategorized) },
        )
        PlaceChip(
            label = Place.Untagged.label,
            count = "19",
            active = place == Place.Untagged,
            onClick = { onSelect(Place.Untagged) },
        )
        PlaceChip(label = "Clients ▾", active = false, onClick = {})
        PlaceChip(
            label = Place.Trash.label,
            count = "3",
            active = place == Place.Trash,
            onClick = { onSelect(Place.Trash) },
        )
    }
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(1.dp)
            .background(EglTokens.Border),
    )
}

@Composable
private fun SearchToolbar(
    query: String,
    onQueryChange: (String) -> Unit,
    filtersPressed: Boolean,
    onToggleFilters: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(40.dp)
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(
            modifier = Modifier
                .weight(1f)
                .height(30.dp)
                .background(EglTokens.Muted, RoundedCornerShape(EglTokens.RadiusSm))
                .padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Icon(
                Icons.Default.Search,
                contentDescription = null,
                tint = EglTokens.MutedForeground,
                modifier = Modifier.size(14.dp),
            )
            BasicTextField(
                value = query,
                onValueChange = onQueryChange,
                singleLine = true,
                textStyle = TextStyle(
                    fontSize = EglTokens.ChromeSp,
                    color = EglTokens.Foreground,
                ),
                decorationBox = { inner ->
                    if (query.isEmpty()) {
                        Text("Search", color = EglTokens.MutedForeground, fontSize = EglTokens.ChromeSp)
                    }
                    inner()
                },
                modifier = Modifier.fillMaxWidth(),
            )
        }
        ChromeIconButton(pressed = filtersPressed, onClick = onToggleFilters) {
            Icon(Icons.Default.FilterList, contentDescription = "Filters", modifier = Modifier.size(16.dp))
        }
        ChromeIconButton(pressed = false, onClick = {}) {
            Icon(Icons.AutoMirrored.Filled.Sort, contentDescription = "Sort", modifier = Modifier.size(16.dp))
        }
    }
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(1.dp)
            .background(EglTokens.Border),
    )
}

@Composable
private fun ChromeIconButton(pressed: Boolean, onClick: () -> Unit, content: @Composable () -> Unit) {
    Box(
        modifier = Modifier
            .size(32.dp)
            .background(
                if (pressed) EglTokens.Muted else Color.Transparent,
                RoundedCornerShape(EglTokens.RadiusSm),
            )
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}

@Composable
private fun FilterBar() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        FilterPill(label = "Tags: poster · logo", active = true)
        FilterPill(label = "Folders", active = false)
        FilterPill(label = "Ext", active = false)
        FilterPill(label = "★ Rating", active = false)
        FilterPill(label = "Shape", active = false)
    }
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(1.dp)
            .background(EglTokens.Border),
    )
}

@Composable
private fun ItemGrid(
    bridge: LibraryBridge,
    items: List<ItemCellDto>,
    onOpen: (String) -> Unit,
) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(96.dp),
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Grid),
        contentPadding = PaddingValues(6.dp),
        horizontalArrangement = Arrangement.spacedBy(EglTokens.GridGutter),
        verticalArrangement = Arrangement.spacedBy(EglTokens.GridGutter),
    ) {
        items(items, key = { it.id }) { item ->
            val ratio = run {
                val w = (item.width ?: 1).toFloat()
                val h = (item.height ?: 1).toFloat().coerceAtLeast(1f)
                (w / h).coerceAtLeast(0.55f)
            }
            val bitmap = remember(item.id, item.thumbVersion, bridge.usingMockData) {
                bridge.thumbnailBitmap(item.id)
            }
            Box(
                modifier = Modifier
                    .aspectRatio(ratio)
                    .background(EglTokens.Muted, RoundedCornerShape(2.dp))
                    .clickable { onOpen(item.id) },
                contentAlignment = Alignment.BottomStart,
            ) {
                if (bitmap != null) {
                    Image(
                        bitmap = bitmap.asImageBitmap(),
                        contentDescription = item.name,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
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
}

@Composable
private fun SimpleList(title: String, rows: List<String>) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(title, fontSize = EglTokens.ChromeSp, color = EglTokens.MutedForeground)
        rows.forEach { row ->
            Text(row, fontSize = EglTokens.ChromeSp, color = EglTokens.Foreground)
        }
    }
}

@Composable
private fun TabBar(tab: LibraryTab, onSelect: (LibraryTab) -> Unit) {
    Column {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(EglTokens.Border),
        )
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(EglTokens.Background)
                .padding(top = 6.dp, bottom = 18.dp),
        ) {
            LibraryTab.entries.forEach { entry ->
                val on = tab == entry
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .clickable { onSelect(entry) },
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    Icon(
                        entry.icon,
                        contentDescription = entry.label,
                        tint = if (on) EglTokens.Foreground else EglTokens.MutedForeground,
                        modifier = Modifier.size(20.dp),
                    )
                    Text(
                        entry.label,
                        fontSize = EglTokens.TabLabelSp,
                        fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal,
                        color = if (on) EglTokens.Foreground else EglTokens.MutedForeground,
                    )
                }
            }
        }
    }
}
