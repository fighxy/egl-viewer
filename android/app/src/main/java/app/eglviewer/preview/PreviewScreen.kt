package app.eglviewer.preview

import android.graphics.Bitmap
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Undo
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.DeleteForever
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.browse.BrowseModel
import app.eglviewer.browse.OriginalCache
import app.eglviewer.browse.Place
import app.eglviewer.browse.RatingMenu
import app.eglviewer.components.UndoSnackbar
import app.eglviewer.core.ItemCell
import app.eglviewer.core.ItemsTarget
import app.eglviewer.details.DetailsSheet
import app.eglviewer.details.ShareFiles
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch

/** Full-screen preview pager over the same core view as the grid (A3 ≈ D-095). */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun PreviewScreen(
    model: BrowseModel,
    start: Int,
    originals: OriginalCache,
    onClose: () -> Unit,
    onShare: (ShareFiles) -> Unit,
) {
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val density = LocalDensity.current
    val total = model.total.coerceAtLeast(1)
    val pagerState = rememberPagerState(initialPage = start.coerceIn(0, total - 1)) { model.total }
    var chrome by remember { mutableStateOf(true) }
    var zoomed by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    var details by remember { mutableStateOf<ItemCell?>(null) }

    LaunchedEffect(model.total) {
        if (model.total == 0) onClose()
    }
    LaunchedEffect(pagerState) {
        snapshotFlow { pagerState.currentPage }
            .distinctUntilChanged()
            .collect { page -> model.need(page) }
    }

    val index = pagerState.currentPage.coerceIn(0, (model.total - 1).coerceAtLeast(0))
    val cell = model.cell(at = index)
    val pixels = with(density) {
        val edge = maxOf(
            context.resources.displayMetrics.widthPixels,
            context.resources.displayMetrics.heightPixels,
        )
        (edge * 1.5f).toInt()
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.PreviewStage)
            .testTag("preview.pager"),
    ) {
        HorizontalPager(
            state = pagerState,
            userScrollEnabled = !zoomed,
            modifier = Modifier.fillMaxSize(),
        ) { page ->
            model.need(page)
            PreviewPage(
                model = model,
                index = page,
                pixels = pixels,
                originals = originals,
                onZoomed = { zoomed = it },
                onSingleTap = { chrome = !chrome },
            )
        }

        if (chrome) {
            PreviewTopBar(
                name = cell?.name.orEmpty(),
                position = if (model.total > 0) index + 1 else 0,
                total = model.total,
                onClose = onClose,
                modifier = Modifier.align(Alignment.TopCenter),
            )
            PreviewBottomBar(
                model = model,
                cell = cell,
                onInfo = { cell?.let { details = it } },
                onShare = {
                    cell?.let { c ->
                        scope.launch {
                            model.shareFiles(ItemsTarget(ids = listOf(c.id)), context.cacheDir)
                                ?.let(onShare)
                        }
                    }
                },
                onDeleteConfirm = { deleting = true },
                modifier = Modifier.align(Alignment.BottomCenter),
            )
        }

        UndoSnackbar(
            toast = model.open.toast,
            onUndo = { scope.launch { model.open.undo() } },
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = if (chrome) 88.dp else 24.dp),
        )
    }

    details?.let { cell ->
        DetailsSheet(
            open = model.open,
            cell = cell,
            onDismiss = { details = null },
        )
    }

    if (deleting) {
        AlertDialog(
            onDismissRequest = { deleting = false },
            title = { Text(stringResource(R.string.trash_delete_confirm, 1)) },
            text = { Text(stringResource(R.string.trash_no_undo)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        deleting = false
                        cell?.let { c ->
                            scope.launch { model.deleteForGood(ItemsTarget(ids = listOf(c.id))) }
                        }
                    },
                ) { Text(stringResource(R.string.action_delete_for_good)) }
            },
            dismissButton = {
                TextButton(onClick = { deleting = false }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }
}

@Composable
private fun PreviewPage(
    model: BrowseModel,
    index: Int,
    pixels: Int,
    originals: OriginalCache,
    onZoomed: (Boolean) -> Unit,
    onSingleTap: () -> Unit,
) {
    val cell = model.cell(at = index)
    var bitmap by remember(cell?.id, cell?.thumbVersion) { mutableStateOf<Bitmap?>(null) }
    var failed by remember(cell?.id) { mutableStateOf(false) }

    LaunchedEffect(cell?.id, cell?.thumbVersion, pixels) {
        val c = cell ?: return@LaunchedEffect
        bitmap = originals.image(forId = c.id, version = c.thumbVersion, pixels = pixels)
        failed = bitmap == null
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.PreviewStage),
        contentAlignment = Alignment.Center,
    ) {
        val image = bitmap
        when {
            image != null -> ZoomingImage(
                bitmap = image,
                onZoomed = onZoomed,
                onSingleTap = onSingleTap,
            )
            failed && cell != null -> Text(
                text = cell.ext.uppercase().ifEmpty { "?" },
                color = Color.White.copy(alpha = 0.6f),
                fontSize = 28.sp,
                fontWeight = FontWeight.Bold,
            )
            else -> CircularProgressIndicator(color = Color.White.copy(alpha = 0.5f))
        }
    }
}

@Composable
private fun PreviewTopBar(
    name: String,
    position: Int,
    total: Int,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(Color.Black.copy(alpha = 0.45f))
            .padding(horizontal = 4.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onClose, modifier = Modifier.testTag("preview.close")) {
            Icon(Icons.Default.Close, contentDescription = stringResource(R.string.common_close), tint = Color.White)
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = name,
                color = Color.White,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (total > 0) {
                Text(
                    text = stringResource(R.string.preview_position, position, total),
                    color = Color.White.copy(alpha = 0.7f),
                    fontSize = 12.sp,
                    modifier = Modifier.testTag("preview.position"),
                )
            }
        }
    }
}

@Composable
private fun PreviewBottomBar(
    model: BrowseModel,
    cell: ItemCell?,
    onInfo: () -> Unit,
    onShare: () -> Unit,
    onDeleteConfirm: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val scope = rememberCoroutineScope()
    val target = cell?.let { ItemsTarget(ids = listOf(it.id)) }
    val readOnly = model.open.readOnly
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(Color.Black.copy(alpha = 0.45f))
            .padding(horizontal = 8.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onInfo, modifier = Modifier.testTag("preview.info")) {
            Icon(Icons.Default.Info, contentDescription = stringResource(R.string.action_info), tint = Color.White)
        }
        if (model.place == Place.Trash) {
            IconButton(
                onClick = { target?.let { t -> scope.launch { model.restore(t) } } },
                enabled = !readOnly && target != null,
                modifier = Modifier.testTag("preview.restore"),
            ) {
                Icon(Icons.AutoMirrored.Filled.Undo, contentDescription = stringResource(R.string.action_restore), tint = Color.White)
            }
            IconButton(
                onClick = onDeleteConfirm,
                enabled = !readOnly && target != null,
                modifier = Modifier.testTag("preview.delete"),
            ) {
                Icon(Icons.Default.DeleteForever, contentDescription = stringResource(R.string.action_delete_for_good), tint = Color.White)
            }
        } else {
            RatingMenu(
                enabled = !readOnly && target != null,
                modifier = Modifier.testTag("preview.rate"),
            ) { stars ->
                target?.let { t -> scope.launch { model.rate(t, stars) } }
            }
            IconButton(
                onClick = onShare,
                enabled = target != null,
                modifier = Modifier.testTag("preview.share"),
            ) {
                Icon(Icons.Default.Share, contentDescription = stringResource(R.string.action_share), tint = Color.White)
            }
            IconButton(
                onClick = { target?.let { t -> scope.launch { model.trash(t) } } },
                enabled = !readOnly && target != null,
                modifier = Modifier.testTag("preview.trash"),
            ) {
                Icon(Icons.Default.Delete, contentDescription = stringResource(R.string.action_trash), tint = Color.White)
            }
        }
    }
}
