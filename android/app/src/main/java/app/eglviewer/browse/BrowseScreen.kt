package app.eglviewer.browse

import android.graphics.Bitmap
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DeleteSweep
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.RadioButtonUnchecked
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.ViewList
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import android.content.ClipData
import android.net.Uri
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.toAndroidDragEvent
import app.eglviewer.R
import app.eglviewer.adding.AddMenuButton
import app.eglviewer.adding.DuplicateQuestionDialog
import app.eglviewer.adding.ImportBanner
import app.eglviewer.adding.ImportPaths
import app.eglviewer.app.LibraryLocations
import app.eglviewer.app.OpenLibrary
import app.eglviewer.components.UndoSnackbar
import app.eglviewer.core.ItemCell
import app.eglviewer.core.SortKey
import app.eglviewer.details.DetailsSheet
import app.eglviewer.details.ShareLauncher
import app.eglviewer.preview.PreviewScreen
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun BrowseScreen(
    open: BrowseHost,
    place: Place,
    onBack: () -> Unit,
) {
    val model = remember(open, place) { BrowseModel(open = open, place = place) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    var tick by remember { mutableIntStateOf(0) }
    var showFilters by remember { mutableStateOf(false) }
    var showSort by remember { mutableStateOf(false) }
    var emptyTrashConfirm by remember { mutableStateOf(false) }
    val openLibrary = open as? OpenLibrary
    val addFolder = (place as? Place.Folder)?.id
    val canAdd = openLibrary != null && !open.readOnly && place != Place.Trash
    val dropTarget = remember(openLibrary, addFolder, canAdd) {
        val host = openLibrary.takeIf { canAdd } ?: return@remember null
        object : DragAndDropTarget {
            override fun onDrop(event: DragAndDropEvent): Boolean {
                val clip = event.toAndroidDragEvent().clipData ?: return false
                val uris = clipUris(clip)
                if (uris.isEmpty()) return false
                val locations = LibraryLocations(context)
                scope.launch {
                    val batch = withContext(Dispatchers.IO) {
                        ImportPaths.fromContentUris(
                            context = context,
                            uris = uris,
                            cacheDir = context.cacheDir,
                            resolvePath = { locations.resolveFilesystemPath(it) },
                        )
                    }
                    host.addPaths(batch.paths, into = addFolder, cleanup = batch.cleanup)
                }
                return true
            }
        }
    }

    DisposableEffect(model) {
        val listener: () -> Unit = { tick += 1 }
        model.addListener(listener)
        model.start()
        onDispose {
            model.removeListener(listener)
            // Preview is hosted in this screen; stop always closes the view when leaving the place.
            model.stop()
        }
    }
    @Suppress("UNUSED_EXPRESSION")
    tick

    Box(
        modifier = Modifier
            .fillMaxSize()
            .then(
                if (dropTarget != null) {
                    Modifier.dragAndDropTarget(
                        shouldStartDragAndDrop = { true },
                        target = dropTarget,
                    )
                } else {
                    Modifier
                },
            ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(EglTokens.Background),
        ) {
            BrowseTopBar(
                title = placeTitle(place),
                selecting = model.selecting,
                selectedCount = model.selectedCount,
                total = model.total,
                filtersActive = model.settings.filters.isActive,
                isTrash = place == Place.Trash,
                addContent = openLibrary?.takeIf { canAdd && !model.selecting }?.let { host ->
                    { AddMenuButton(open = host, folder = addFolder) }
                },
                onBack = {
                    if (model.selecting) {
                        model.setSelecting(false)
                    } else {
                        open.forgetPlace()
                        onBack()
                    }
                },
                onSelect = { model.setSelecting(true) },
                onSelectAll = { model.selectAll() },
                onDeselectAll = { model.deselectAll() },
                onDone = { model.setSelecting(false) },
                onFilter = { showFilters = true },
                onEmptyTrash = { emptyTrashConfirm = true },
                onToggleLayout = {
                    model.change {
                        it.copy(
                            layout = if (it.layout == BrowseLayout.Grid) {
                                BrowseLayout.List
                            } else {
                                BrowseLayout.Grid
                            },
                        )
                    }
                },
                layout = model.settings.layout,
                showSort = showSort,
                onShowSort = { showSort = it },
                sort = model.settings.sort,
                ascending = model.settings.ascending,
                size = model.settings.size,
                onSort = { key -> model.change { it.copy(sort = key) } },
                onAscending = { model.change { it.copy(ascending = !it.ascending) } },
                onSize = { size -> model.change { it.copy(size = size) } },
            )

            if (openLibrary != null) {
                ImportBanner(open = openLibrary)
            }

            SearchField(
                query = model.settings.search,
                onQuery = { q -> model.change { it.copy(search = q) } },
            )

            if (!model.loading) {
                Text(
                    text = stringResource(R.string.browse_item_count, model.total),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
                )
            }

            Box(modifier = Modifier.weight(1f)) {
                when (model.settings.layout) {
                    BrowseLayout.Grid -> BrowseGrid(model = model)
                    BrowseLayout.List -> BrowseList(model = model)
                }
            }

            if (model.selecting) {
                SelectionBar(model = model)
            }
        }

        if (model.previewing == null) {
            UndoSnackbar(
                toast = open.toast,
                onUndo = { scope.launch { open.undo() } },
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = if (model.selecting) 96.dp else 16.dp),
            )
        }
    }

    model.previewing?.let { start ->
        val originals = (open as? OpenLibrary)?.originals
            ?: OriginalCache(open.session)
        PreviewScreen(
            model = model,
            start = start,
            originals = originals,
            onClose = { model.closePreview() },
            onShare = { files ->
                context.startActivity(ShareLauncher.intent(context, files))
                files.clean()
            },
        )
    }

    model.details?.let { cell ->
        DetailsSheet(
            open = open,
            cell = cell,
            onDismiss = { model.closeDetails() },
        )
    }

    model.sharing?.let { files ->
        LaunchedEffect(files) {
            context.startActivity(ShareLauncher.intent(context, files))
            model.clearSharing()
        }
    }

    if (showFilters) {
        FilterSheet(model = model, onDismiss = { showFilters = false })
    }

    if (emptyTrashConfirm) {
        AlertDialog(
            onDismissRequest = { emptyTrashConfirm = false },
            title = { Text(stringResource(R.string.trash_empty_confirm)) },
            text = { Text(stringResource(R.string.trash_no_undo)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        emptyTrashConfirm = false
                        scope.launch { model.emptyTrash() }
                    },
                ) { Text(stringResource(R.string.trash_empty)) }
            },
            dismissButton = {
                TextButton(onClick = { emptyTrashConfirm = false }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }

    if (openLibrary != null) {
        DuplicateQuestionDialog(open = openLibrary)
    }
}

private fun clipUris(clip: ClipData): List<Uri> {
    val uris = mutableListOf<Uri>()
    for (i in 0 until clip.itemCount) {
        clip.getItemAt(i).uri?.let { uris += it }
    }
    return uris
}

@Composable
private fun BrowseTopBar(
    title: String,
    selecting: Boolean,
    selectedCount: Int,
    total: Int,
    filtersActive: Boolean,
    isTrash: Boolean,
    addContent: (@Composable () -> Unit)? = null,
    onBack: () -> Unit,
    onSelect: () -> Unit,
    onSelectAll: () -> Unit,
    onDeselectAll: () -> Unit,
    onDone: () -> Unit,
    onFilter: () -> Unit,
    onEmptyTrash: () -> Unit,
    onToggleLayout: () -> Unit,
    layout: BrowseLayout,
    showSort: Boolean,
    onShowSort: (Boolean) -> Unit,
    sort: SortKey,
    ascending: Boolean,
    size: ThumbnailSize,
    onSort: (SortKey) -> Unit,
    onAscending: () -> Unit,
    onSize: (ThumbnailSize) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack) {
            Icon(
                if (selecting) Icons.Default.Close else Icons.AutoMirrored.Filled.ArrowBack,
                contentDescription = if (selecting) "Done" else "Back",
            )
        }
        Text(
            text = if (selecting) "$selectedCount selected" else title,
            fontWeight = FontWeight.SemiBold,
            fontSize = 17.sp,
            color = EglTokens.Foreground,
            modifier = Modifier.weight(1f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (selecting) {
            TextButton(
                onClick = {
                    if (selectedCount == total && total > 0) onDeselectAll() else onSelectAll()
                },
            ) {
                Text(if (selectedCount == total && total > 0) "Deselect All" else "Select All")
            }
            TextButton(onClick = onDone) { Text("Done") }
        } else {
            IconButton(onClick = onFilter) {
                Icon(
                    Icons.Default.FilterList,
                    contentDescription = "Filters",
                    tint = if (filtersActive) EglTokens.Primary else EglTokens.Foreground,
                )
            }
            IconButton(onClick = onToggleLayout) {
                Icon(
                    if (layout == BrowseLayout.Grid) Icons.Default.ViewList else Icons.Default.GridView,
                    contentDescription = "Layout",
                )
            }
            Box {
                IconButton(onClick = { onShowSort(true) }) {
                    Icon(Icons.AutoMirrored.Filled.Sort, contentDescription = "Sort")
                }
                DropdownMenu(expanded = showSort, onDismissRequest = { onShowSort(false) }) {
                    SortKey.offered.forEach { key ->
                        DropdownMenuItem(
                            text = { Text(sortLabel(key) + if (key == sort) " ✓" else "") },
                            onClick = {
                                onSort(key)
                                onShowSort(false)
                            },
                        )
                    }
                    if (sort != SortKey.Random) {
                        DropdownMenuItem(
                            text = { Text(if (ascending) "Ascending ✓" else "Ascending") },
                            onClick = {
                                onAscending()
                                onShowSort(false)
                            },
                        )
                    }
                    if (layout == BrowseLayout.Grid) {
                        ThumbnailSize.entries.forEach { s ->
                            DropdownMenuItem(
                                text = { Text(s.name.lowercase() + if (s == size) " ✓" else "") },
                                onClick = {
                                    onSize(s)
                                    onShowSort(false)
                                },
                            )
                        }
                    }
                }
            }
            if (isTrash && total > 0) {
                IconButton(onClick = onEmptyTrash) {
                    Icon(Icons.Default.DeleteSweep, contentDescription = stringResource(R.string.trash_empty))
                }
            }
            addContent?.invoke()
            TextButton(onClick = onSelect, enabled = total > 0) { Text(stringResource(R.string.browse_select)) }
        }
    }
}

@Composable
private fun SearchField(query: String, onQuery: (String) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 6.dp)
            .background(EglTokens.Muted, RoundedCornerShape(10.dp))
            .padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(Icons.Default.Search, contentDescription = null, tint = EglTokens.MutedForeground)
        BasicTextField(
            value = query,
            onValueChange = onQuery,
            singleLine = true,
            textStyle = TextStyle(color = EglTokens.Foreground, fontSize = 15.sp),
            modifier = Modifier.weight(1f),
            decorationBox = { inner ->
                if (query.isEmpty()) {
                    Text("Search", color = EglTokens.MutedForeground, fontSize = 15.sp)
                }
                inner()
            },
        )
    }
}

@Composable
private fun BrowseGrid(model: BrowseModel) {
    val minWidth = model.settings.size.minimumWidthDp.dp
    val density = LocalDensity.current
    val pixels = with(density) { (minWidth * 1.6f).roundToPx() }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(minSize = minWidth),
        contentPadding = PaddingValues(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        items(model.total) { index ->
            model.need(index)
            val cell = model.cell(at = index)
            GridTile(
                cell = cell,
                selected = cell?.let { model.isSelected(it.id) } == true,
                selecting = model.selecting,
                pixels = pixels,
                cache = model.open.thumbnails,
                onClick = { cell?.let { model.tap(it, index) } },
                onLongClick = { cell?.let { model.startSelecting(it) } },
            )
        }
    }
}

@Composable
private fun BrowseList(model: BrowseModel) {
    val density = LocalDensity.current
    val pixels = with(density) { 48.dp.roundToPx() }
    LazyColumn(modifier = Modifier.fillMaxSize()) {
        items(model.total) { index ->
            model.need(index)
            val cell = model.cell(at = index)
            ListRow(
                cell = cell,
                selected = cell?.let { model.isSelected(it.id) } == true,
                selecting = model.selecting,
                pixels = pixels,
                cache = model.open.thumbnails,
                onClick = { cell?.let { model.tap(it, index) } },
                onLongClick = { cell?.let { model.startSelecting(it) } },
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun GridTile(
    cell: ItemCell?,
    selected: Boolean,
    selecting: Boolean,
    pixels: Int,
    cache: ThumbnailCache,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
) {
    Column(
        modifier = Modifier.combinedClickable(onClick = onClick, onLongClick = onLongClick),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(1f)
                .background(EglTokens.Muted, RoundedCornerShape(6.dp))
                .then(
                    if (selected) {
                        Modifier.border(3.dp, EglTokens.Primary, RoundedCornerShape(6.dp))
                    } else {
                        Modifier
                    },
                ),
            contentAlignment = Alignment.Center,
        ) {
            ItemThumbnail(cell = cell, pixels = pixels, cache = cache)
            if (selecting && cell != null) {
                Icon(
                    imageVector = if (selected) Icons.Default.CheckCircle else Icons.Default.RadioButtonUnchecked,
                    contentDescription = null,
                    tint = if (selected) EglTokens.Primary else EglTokens.Foreground,
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .padding(6.dp),
                )
            }
        }
        Text(
            text = cell?.name ?: " ",
            color = EglTokens.MutedForeground,
            fontSize = 11.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 3.dp),
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ListRow(
    cell: ItemCell?,
    selected: Boolean,
    selecting: Boolean,
    pixels: Int,
    cache: ThumbnailCache,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .combinedClickable(onClick = onClick, onLongClick = onLongClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (selecting) {
            Icon(
                imageVector = if (selected) Icons.Default.CheckCircle else Icons.Default.RadioButtonUnchecked,
                contentDescription = null,
                tint = if (selected) EglTokens.Primary else EglTokens.MutedForeground,
            )
        }
        Box(
            modifier = Modifier
                .size(48.dp)
                .background(EglTokens.Muted, RoundedCornerShape(4.dp)),
            contentAlignment = Alignment.Center,
        ) {
            ItemThumbnail(cell = cell, pixels = pixels, cache = cache)
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = cell?.name ?: " ",
                color = EglTokens.Foreground,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (cell != null) {
                Text(
                    text = cellDetails(cell),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                    maxLines = 1,
                )
            }
        }
        if (cell != null && cell.star > 0) {
            Text("${cell.star}★", color = EglTokens.MutedForeground, fontSize = 12.sp)
        }
    }
}

@Composable
private fun ItemThumbnail(cell: ItemCell?, pixels: Int, cache: ThumbnailCache) {
    var bitmap by remember(cell?.id, cell?.thumbVersion, pixels) { mutableStateOf<Bitmap?>(null) }
    LaunchedEffect(cell?.id, cell?.thumbVersion, pixels) {
        bitmap = if (cell != null) cache.image(forCell = cell, pixels = pixels) else null
    }
    val palette = cell?.palette0
    when {
        bitmap != null -> Image(
            bitmap = bitmap!!.asImageBitmap(),
            contentDescription = cell?.name,
            contentScale = ContentScale.Fit,
            modifier = Modifier.fillMaxSize().padding(4.dp),
        )
        palette != null && palette.size >= 3 -> Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Color(
                        red = palette[0] / 255f,
                        green = palette[1] / 255f,
                        blue = palette[2] / 255f,
                    ).copy(alpha = 0.35f),
                ),
        )
        cell != null -> Text(
            text = cell.ext.uppercase(),
            color = EglTokens.MutedForeground,
            fontSize = 11.sp,
            fontWeight = FontWeight.SemiBold,
        )
    }
}

private fun placeTitle(place: Place): String = when (place) {
    Place.All -> "All"
    Place.Uncategorized -> "Uncategorized"
    Place.Untagged -> "Untagged"
    Place.Trash -> "Trash"
    is Place.Folder -> place.name
    is Place.Smart -> place.name
    is Place.Tag -> place.name
}

private fun sortLabel(key: SortKey): String = when (key) {
    SortKey.Added -> "Date added"
    SortKey.Btime -> "Date created"
    SortKey.Mtime -> "Date modified"
    SortKey.Name -> "Name"
    SortKey.Ext -> "Type"
    SortKey.Size -> "Size"
    SortKey.Resolution -> "Resolution"
    SortKey.Star -> "Rating"
    SortKey.Random -> "Random"
}

private fun cellDetails(cell: ItemCell): String {
    val parts = mutableListOf(cell.ext.uppercase())
    if (cell.width != null && cell.height != null) {
        parts += "${cell.width} × ${cell.height}"
    }
    cell.size?.let { parts += "$it B" }
    return parts.joinToString(" · ")
}
