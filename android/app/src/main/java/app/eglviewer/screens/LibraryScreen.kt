package app.eglviewer.screens

import androidx.compose.foundation.background
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.LabelOff
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.Inventory2
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.adding.AddMenuButton
import app.eglviewer.adding.DuplicateQuestionDialog
import app.eglviewer.adding.ImportBanner
import app.eglviewer.app.OpenLibrary
import app.eglviewer.browse.BrowseScreen
import app.eglviewer.browse.Place
import app.eglviewer.core.Folder
import app.eglviewer.core.ReadOnlyReason
import app.eglviewer.core.SmartFolder
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Open library: places with counts, folder tree and smart folders. A place opens
 * [BrowseScreen] (Android A2 ≈ D-094 / D-102).
 */
@Composable
fun LibraryScreen(
    open: OpenLibrary,
    onClose: () -> Unit,
) {
    var place by remember { mutableStateOf<Place?>(null) }
    var restored by remember(open) { mutableStateOf(false) }

    LaunchedEffect(open) {
        withContext(Dispatchers.IO) { open.refresh() }
        if (!restored) {
            restored = true
            val saved = open.savedView.place
            if (saved != null && open.exists(saved)) {
                place = saved
            }
        }
    }

    if (place != null) {
        BrowseScreen(
            open = open,
            place = place!!,
            onBack = {
                open.forgetPlace()
                place = null
            },
        )
        return
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onClose) {
                Icon(
                    Icons.AutoMirrored.Filled.ArrowBack,
                    contentDescription = stringResource(R.string.library_close),
                    tint = EglTokens.Foreground,
                )
            }
            Text(
                text = open.info.name,
                color = EglTokens.Foreground,
                fontSize = EglTokens.TitleSp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.weight(1f),
            )
            if (!open.readOnly) {
                AddMenuButton(open = open, folder = null)
            }
        }

        ImportBanner(open = open)

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            if (open.readOnly) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(vertical = 8.dp),
                ) {
                    Icon(
                        Icons.Default.Lock,
                        contentDescription = null,
                        tint = EglTokens.MutedForeground,
                    )
                    Text(
                        text = readOnlyLabel(open.info.readOnlyReason),
                        color = EglTokens.MutedForeground,
                        fontSize = 13.sp,
                    )
                }
            }

            Text(
                text = stringResource(R.string.library_places),
                color = EglTokens.MutedForeground,
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(top = 12.dp, bottom = 4.dp),
            )
            PlaceRow(
                icon = Icons.Default.GridView,
                title = stringResource(R.string.place_all),
                count = open.counts?.all,
                onClick = { place = Place.All },
            )
            PlaceRow(
                icon = Icons.Default.Inventory2,
                title = stringResource(R.string.place_uncategorized),
                count = open.counts?.uncategorized,
                onClick = { place = Place.Uncategorized },
            )
            PlaceRow(
                icon = Icons.AutoMirrored.Filled.LabelOff,
                title = stringResource(R.string.place_untagged),
                count = open.counts?.untagged,
                onClick = { place = Place.Untagged },
            )
            PlaceRow(
                icon = Icons.Default.Delete,
                title = stringResource(R.string.place_trash),
                count = open.counts?.trash,
                onClick = { place = Place.Trash },
            )

            if (open.folders.isNotEmpty()) {
                HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
                Text(
                    text = stringResource(R.string.library_folders),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(bottom = 4.dp),
                )
                FolderTree(folders = open.folders, onOpen = { place = it })
            }

            if (open.smartFolders.isNotEmpty()) {
                HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
                Text(
                    text = stringResource(R.string.library_smart_folders),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(bottom = 4.dp),
                )
                SmartTree(folders = open.smartFolders, onOpen = { place = it })
            }

            Spacer(modifier = Modifier.height(24.dp))
        }
    }

    DuplicateQuestionDialog(open = open)
}

@Composable
private fun FolderTree(folders: List<Folder>, onOpen: (Place) -> Unit) {
    for (folder in folders) {
        val lockedClosed = folder.isLocked && !folder.isUnlocked
        PlaceRow(
            icon = if (lockedClosed) Icons.Default.Lock else Icons.Default.Folder,
            title = folder.name,
            count = folder.descendantImageCount ?: folder.imageCount,
            subtitle = if (lockedClosed) folder.lockLabel else null,
            onClick = if (lockedClosed) {
                null
            } else {
                { onOpen(Place.Folder(id = folder.id, name = folder.name)) }
            },
        )
        FolderTree(folders = folder.children, onOpen = onOpen)
    }
}

@Composable
private fun SmartTree(folders: List<SmartFolder>, onOpen: (Place) -> Unit) {
    for (folder in folders) {
        PlaceRow(
            icon = Icons.Default.Settings,
            title = folder.name,
            count = null,
            onClick = { onOpen(Place.Smart(id = folder.id, name = folder.name)) },
        )
        SmartTree(folders = folder.children, onOpen = onOpen)
    }
}

@Composable
private fun PlaceRow(
    icon: ImageVector,
    title: String,
    count: Int?,
    subtitle: String? = null,
    onClick: (() -> Unit)?,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Icon(icon, contentDescription = null, tint = EglTokens.MutedForeground)
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                color = EglTokens.Foreground,
                fontSize = 15.sp,
                fontWeight = FontWeight.Medium,
            )
            if (!subtitle.isNullOrBlank()) {
                Text(text = subtitle, color = EglTokens.MutedForeground, fontSize = 12.sp)
            }
        }
        if (count != null) {
            Text(
                text = count.toString(),
                color = EglTokens.MutedForeground,
                fontSize = 14.sp,
            )
        }
    }
}

@Composable
private fun readOnlyLabel(reason: ReadOnlyReason?): String =
    when (reason) {
        ReadOnlyReason.OTHER_INSTANCE -> stringResource(R.string.library_read_only_other)
        ReadOnlyReason.SYNC_FOLDER -> stringResource(R.string.library_read_only_synced)
        else -> stringResource(R.string.library_read_only)
    }
