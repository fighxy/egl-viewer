package app.eglviewer.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.ExperimentalFoundationApi
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
import androidx.compose.material.icons.automirrored.filled.Label
import androidx.compose.material.icons.automirrored.filled.LabelOff
import androidx.compose.material.icons.filled.CreateNewFolder
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.Inventory2
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
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
import app.eglviewer.adding.EagleOutcomeDialog
import app.eglviewer.adding.ImportBanner
import app.eglviewer.app.OpenLibrary
import app.eglviewer.browse.BrowseScreen
import app.eglviewer.browse.Place
import app.eglviewer.core.Folder
import app.eglviewer.core.ReadOnlyReason
import app.eglviewer.core.SmartFolder
import app.eglviewer.locks.AppLock
import app.eglviewer.locks.LockAction
import app.eglviewer.locks.LockSheetDialog
import app.eglviewer.organize.FolderEditing
import app.eglviewer.organize.FolderEditorDialog
import app.eglviewer.organize.FolderMoveDialog
import app.eglviewer.organize.SmartEditing
import app.eglviewer.organize.SmartFolderEditorDialog
import app.eglviewer.organize.TagsScreen
import app.eglviewer.organize.labelColor
import app.eglviewer.settings.SettingsScreen
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Open library: places with counts, folder tree and smart folders. A place opens
 * [BrowseScreen] (Android A2–A7 ≈ D-094 / D-107).
 */
@Composable
fun LibraryScreen(
    open: OpenLibrary,
    lock: AppLock,
    onClose: () -> Unit,
) {
    var place by remember { mutableStateOf<Place?>(null) }
    var showTags by remember { mutableStateOf(false) }
    var showSettings by remember { mutableStateOf(false) }
    var restored by remember(open) { mutableStateOf(false) }
    var folderEditing by remember { mutableStateOf<FolderEditing?>(null) }
    var movingFolder by remember { mutableStateOf<Folder?>(null) }
    var deletingFolder by remember { mutableStateOf<Folder?>(null) }
    var smartEditing by remember { mutableStateOf<SmartEditing?>(null) }
    var deletingSmart by remember { mutableStateOf<SmartFolder?>(null) }
    var lockAction by remember { mutableStateOf<LockAction?>(null) }
    val scope = rememberCoroutineScope()

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

    if (showSettings) {
        SettingsScreen(
            open = open,
            lock = lock,
            onBack = { showSettings = false },
        )
        return
    }

    if (showTags) {
        TagsScreen(
            open = open,
            onOpenPlace = {
                showTags = false
                place = it
            },
            onBack = { showTags = false },
        )
        return
    }

    if (place != null) {
        BrowseScreen(
            open = open,
            place = place!!,
            onBack = {
                open.forgetPlace()
                place = null
            },
            onSaveAsSmart = { draft ->
                smartEditing = SmartEditing.Create(parent = null, conditions = draft)
            },
        )
        // Smart editor can open from browse "Save as Smart Folder".
        smartEditing?.let { editing ->
            SmartFolderEditorDialog(
                open = open,
                editing = editing,
                onDismiss = { smartEditing = null },
            )
        }
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
            if (open.hasUnlockedFolders) {
                IconButton(
                    onClick = {
                        scope.launch { open.relockAll() }
                    },
                ) {
                    Icon(
                        Icons.Default.Lock,
                        contentDescription = stringResource(R.string.lock_lock_all),
                        tint = EglTokens.Foreground,
                    )
                }
            }
            IconButton(onClick = { showSettings = true }) {
                Icon(
                    Icons.Default.Settings,
                    contentDescription = stringResource(R.string.settings_title),
                    tint = EglTokens.Foreground,
                )
            }
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
            PlaceRow(
                icon = Icons.AutoMirrored.Filled.Label,
                title = stringResource(R.string.library_tags),
                count = null,
                onClick = { showTags = true },
            )

            if (open.folders.isNotEmpty() || !open.readOnly) {
                HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
                Text(
                    text = stringResource(R.string.library_folders),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(bottom = 4.dp),
                )
                FolderTree(
                    folders = open.folders,
                    readOnly = open.readOnly,
                    onOpen = { place = it },
                    onUnlock = { lockAction = LockAction.Unlock(it) },
                    onLockAction = { lockAction = it },
                    onNewSubfolder = { folderEditing = FolderEditing.Create(parent = it.id) },
                    onEdit = { folderEditing = FolderEditing.Edit(it) },
                    onMove = { movingFolder = it },
                    onDelete = { deletingFolder = it },
                    onRelock = { folder ->
                        scope.launch {
                            open.lockWork { it.relock(folder.id) }
                        }
                    },
                )
                if (!open.readOnly) {
                    PlaceRow(
                        icon = Icons.Default.CreateNewFolder,
                        title = stringResource(R.string.folder_new),
                        count = null,
                        onClick = { folderEditing = FolderEditing.Create(parent = null) },
                    )
                }
            }

            if (open.smartFolders.isNotEmpty() || !open.readOnly) {
                HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
                Text(
                    text = stringResource(R.string.library_smart_folders),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.padding(bottom = 4.dp),
                )
                SmartTree(
                    folders = open.smartFolders,
                    readOnly = open.readOnly,
                    onOpen = { place = it },
                    onNewChild = {
                        smartEditing = SmartEditing.Create(parent = it.id, conditions = null)
                    },
                    onEdit = { smartEditing = SmartEditing.Edit(it) },
                    onDelete = { deletingSmart = it },
                )
                if (!open.readOnly) {
                    PlaceRow(
                        icon = Icons.Default.Settings,
                        title = stringResource(R.string.smart_new),
                        count = null,
                        onClick = {
                            smartEditing = SmartEditing.Create(parent = null, conditions = null)
                        },
                    )
                }
            }

            Spacer(modifier = Modifier.height(24.dp))
        }
    }

    DuplicateQuestionDialog(open = open)
    EagleOutcomeDialog(open = open)

    folderEditing?.let { editing ->
        FolderEditorDialog(
            open = open,
            editing = editing,
            onDismiss = { folderEditing = null },
        )
    }
    movingFolder?.let { folder ->
        FolderMoveDialog(
            open = open,
            folder = folder,
            onDismiss = { movingFolder = null },
        )
    }
    deletingFolder?.let { folder ->
        AlertDialog(
            onDismissRequest = { deletingFolder = null },
            title = { Text(stringResource(R.string.folder_delete_title)) },
            text = { Text(folder.name) },
            confirmButton = {
                Column {
                    TextButton(
                        onClick = {
                            scope.launch {
                                open.deleteFolder(folder.id, trashItems = false)
                                deletingFolder = null
                            }
                        },
                    ) { Text(stringResource(R.string.folder_delete_keep_items)) }
                    TextButton(
                        onClick = {
                            scope.launch {
                                open.deleteFolder(folder.id, trashItems = true)
                                deletingFolder = null
                            }
                        },
                    ) { Text(stringResource(R.string.folder_delete_trash_items)) }
                }
            },
            dismissButton = {
                TextButton(onClick = { deletingFolder = null }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }
    smartEditing?.let { editing ->
        SmartFolderEditorDialog(
            open = open,
            editing = editing,
            onDismiss = { smartEditing = null },
        )
    }
    deletingSmart?.let { folder ->
        AlertDialog(
            onDismissRequest = { deletingSmart = null },
            title = { Text(stringResource(R.string.smart_delete_title)) },
            text = {
                Text(
                    if (folder.children.isEmpty()) {
                        stringResource(R.string.smart_delete_hint)
                    } else {
                        stringResource(R.string.smart_delete_hint_children)
                    },
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        scope.launch {
                            open.deleteSmartFolder(folder.id)
                            deletingSmart = null
                        }
                    },
                ) { Text(stringResource(R.string.smart_delete)) }
            },
            dismissButton = {
                TextButton(onClick = { deletingSmart = null }) {
                    Text(stringResource(R.string.common_cancel))
                }
            },
        )
    }
    lockAction?.let { action ->
        LockSheetDialog(
            open = open,
            action = action,
            onDismiss = { lockAction = null },
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun FolderTree(
    folders: List<Folder>,
    readOnly: Boolean,
    depth: Int = 0,
    onOpen: (Place) -> Unit,
    onUnlock: (Folder) -> Unit,
    onLockAction: (LockAction) -> Unit,
    onNewSubfolder: (Folder) -> Unit,
    onEdit: (Folder) -> Unit,
    onMove: (Folder) -> Unit,
    onDelete: (Folder) -> Unit,
    onRelock: (Folder) -> Unit,
) {
    for (folder in folders) {
        val lockedClosed = folder.isLocked && !folder.isUnlocked
        var menu by remember(folder.id) { mutableStateOf(false) }
        val lockTint = labelColor(folder.lockColor) ?: EglTokens.MutedForeground
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = (depth * 16).dp)
                .combinedClickable(
                    onClick = {
                        if (lockedClosed) {
                            onUnlock(folder)
                        } else {
                            onOpen(Place.Folder(id = folder.id, name = folder.name))
                        }
                    },
                    onLongClick = { menu = true },
                )
                .padding(vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Icon(
                if (lockedClosed) Icons.Default.Lock else Icons.Default.Folder,
                contentDescription = null,
                tint = if (lockedClosed) lockTint else EglTokens.MutedForeground,
            )
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = if (lockedClosed) {
                        folder.lockLabel?.ifBlank { null } ?: folder.name
                    } else {
                        folder.name
                    },
                    color = EglTokens.Foreground,
                    fontSize = 15.sp,
                    fontWeight = FontWeight.Medium,
                )
            }
            if (!lockedClosed) {
                val count = folder.descendantImageCount ?: folder.imageCount
                if (count != null) {
                    Text(text = count.toString(), color = EglTokens.MutedForeground, fontSize = 14.sp)
                }
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                if (lockedClosed) {
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.lock_unlock)) },
                        onClick = {
                            menu = false
                            onLockAction(LockAction.Unlock(folder))
                        },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.lock_recover)) },
                        onClick = {
                            menu = false
                            onLockAction(LockAction.Recover(folder))
                        },
                    )
                    if (!readOnly && !folder.hasPassword) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.lock_set_password)) },
                            onClick = {
                                menu = false
                                onLockAction(LockAction.Lock(folder))
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.lock_remove_lock)) },
                            onClick = {
                                menu = false
                                onLockAction(LockAction.RemoveLock(folder))
                            },
                        )
                    }
                } else {
                    if (!readOnly) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.folder_new_subfolder)) },
                            onClick = {
                                menu = false
                                onNewSubfolder(folder)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.folder_edit)) },
                            onClick = {
                                menu = false
                                onEdit(folder)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.folder_move)) },
                            onClick = {
                                menu = false
                                onMove(folder)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.folder_delete)) },
                            onClick = {
                                menu = false
                                onDelete(folder)
                            },
                        )
                        if (!folder.isLocked) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.lock_lock)) },
                                onClick = {
                                    menu = false
                                    onLockAction(LockAction.Lock(folder))
                                },
                            )
                        } else {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.lock_lock_now)) },
                                onClick = {
                                    menu = false
                                    onRelock(folder)
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.lock_change_password)) },
                                onClick = {
                                    menu = false
                                    onLockAction(LockAction.ChangePassword(folder))
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.lock_renew_key)) },
                                onClick = {
                                    menu = false
                                    onLockAction(LockAction.RenewKey(folder))
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.lock_label_title)) },
                                onClick = {
                                    menu = false
                                    onLockAction(LockAction.Label(folder))
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.lock_remove_lock)) },
                                onClick = {
                                    menu = false
                                    onLockAction(LockAction.RemoveLock(folder))
                                },
                            )
                        }
                    }
                }
            }
        }
        if (!lockedClosed) {
            FolderTree(
                folders = folder.children,
                readOnly = readOnly,
                depth = depth + 1,
                onOpen = onOpen,
                onUnlock = onUnlock,
                onLockAction = onLockAction,
                onNewSubfolder = onNewSubfolder,
                onEdit = onEdit,
                onMove = onMove,
                onDelete = onDelete,
                onRelock = onRelock,
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SmartTree(
    folders: List<SmartFolder>,
    readOnly: Boolean,
    depth: Int = 0,
    onOpen: (Place) -> Unit,
    onNewChild: (SmartFolder) -> Unit,
    onEdit: (SmartFolder) -> Unit,
    onDelete: (SmartFolder) -> Unit,
) {
    for (folder in folders) {
        var menu by remember(folder.id) { mutableStateOf(false) }
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = (depth * 16).dp)
                .combinedClickable(
                    onClick = { onOpen(Place.Smart(id = folder.id, name = folder.name)) },
                    onLongClick = { if (!readOnly) menu = true },
                )
                .padding(vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Icon(Icons.Default.Settings, contentDescription = null, tint = EglTokens.MutedForeground)
            Text(
                text = folder.name,
                color = EglTokens.Foreground,
                fontSize = 15.sp,
                fontWeight = FontWeight.Medium,
                modifier = Modifier.weight(1f),
            )
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.smart_new_child)) },
                    onClick = {
                        menu = false
                        onNewChild(folder)
                    },
                )
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.smart_edit)) },
                    onClick = {
                        menu = false
                        onEdit(folder)
                    },
                )
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.smart_delete)) },
                    onClick = {
                        menu = false
                        onDelete(folder)
                    },
                )
            }
        }
        SmartTree(
            folders = folder.children,
            readOnly = readOnly,
            depth = depth + 1,
            onOpen = onOpen,
            onNewChild = onNewChild,
            onEdit = onEdit,
            onDelete = onDelete,
        )
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
