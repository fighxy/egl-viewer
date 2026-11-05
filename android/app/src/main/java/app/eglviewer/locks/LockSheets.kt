package app.eglviewer.locks

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.OpenLibrary
import app.eglviewer.core.EglError
import app.eglviewer.core.Folder
import app.eglviewer.core.RecoveryKey
import androidx.compose.ui.graphics.Color
import app.eglviewer.organize.ColorChoice
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.launch

/** What a lock sheet does to a folder (Android A6 ≈ D-098). */
sealed class LockAction {
    abstract val folder: Folder

    /** Locks an open folder, or sets the first password of a lock imported without one. */
    data class Lock(override val folder: Folder) : LockAction()
    data class Unlock(override val folder: Folder) : LockAction()
    /** Opens it with the recovery key or phrase and sets a new password. */
    data class Recover(override val folder: Folder) : LockAction()
    data class ChangePassword(override val folder: Folder) : LockAction()
    data class RenewKey(override val folder: Folder) : LockAction()
    data class RemoveLock(override val folder: Folder) : LockAction()
    data class Label(override val folder: Folder) : LockAction()
}

/**
 * One lock operation: the fields it needs, a wrong password said in place, and the recovery
 * key shown once when the operation makes one. The work runs off the main thread (Argon2id).
 */
@Composable
fun LockSheetDialog(
    open: OpenLibrary,
    action: LockAction,
    onDismiss: () -> Unit,
) {
    var password by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    var current by remember { mutableStateOf("") }
    var recovery by remember { mutableStateOf("") }
    var label by remember { mutableStateOf("") }
    var color by remember { mutableStateOf<String?>(null) }
    var working by remember { mutableStateOf(false) }
    var wrong by remember { mutableStateOf(false) }
    var key by remember { mutableStateOf<RecoveryKey?>(null) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current

    LaunchedEffect(action) {
        label = action.folder.lockLabel.orEmpty()
        color = action.folder.lockColor
        password = ""
        confirm = ""
        current = ""
        recovery = ""
        wrong = false
        key = null
    }

    val shown = key
    if (shown != null) {
        RecoveryKeyDialog(
            recoveryKey = shown,
            onDone = onDismiss,
            onCopy = { text -> copySecret(context, text) },
        )
        return
    }

    val title = when (action) {
        is LockAction.Lock ->
            if (action.folder.isLocked && !action.folder.hasPassword) {
                stringResource(R.string.lock_set_password)
            } else {
                stringResource(R.string.lock_lock)
            }
        is LockAction.Unlock -> stringResource(R.string.lock_unlock)
        is LockAction.Recover -> stringResource(R.string.lock_recover)
        is LockAction.ChangePassword -> stringResource(R.string.lock_change_password)
        is LockAction.RenewKey -> stringResource(R.string.lock_renew_key)
        is LockAction.RemoveLock -> stringResource(R.string.lock_remove_lock)
        is LockAction.Label -> stringResource(R.string.lock_label_title)
    }

    val hint = when (action) {
        is LockAction.Lock ->
            if (action.folder.isLocked && !action.folder.hasPassword) {
                stringResource(R.string.lock_set_password_hint)
            } else {
                stringResource(R.string.lock_lock_hint)
            }
        is LockAction.Unlock -> stringResource(R.string.lock_unlock_hint)
        is LockAction.Recover -> stringResource(R.string.lock_recover_hint)
        is LockAction.ChangePassword -> stringResource(R.string.lock_change_hint)
        is LockAction.RenewKey -> stringResource(R.string.lock_renew_hint)
        is LockAction.RemoveLock -> stringResource(R.string.lock_remove_hint)
        is LockAction.Label -> stringResource(R.string.lock_label_hint)
    }

    val ready = when (action) {
        is LockAction.Lock, is LockAction.Recover ->
            password.isNotEmpty() && password == confirm &&
                (action !is LockAction.Recover || recovery.isNotBlank())
        is LockAction.ChangePassword ->
            current.isNotEmpty() && password.isNotEmpty() && password == confirm
        is LockAction.Unlock, is LockAction.RenewKey -> password.isNotEmpty()
        is LockAction.RemoveLock ->
            password.isNotEmpty() || !action.folder.hasPassword
        is LockAction.Label -> true
    }

    val confirmTitle = when (action) {
        is LockAction.Lock -> stringResource(R.string.lock_lock)
        is LockAction.Unlock, is LockAction.Recover -> stringResource(R.string.lock_unlock)
        is LockAction.RemoveLock -> stringResource(R.string.lock_remove)
        else -> stringResource(R.string.common_save)
    }

    AlertDialog(
        onDismissRequest = { if (!working) onDismiss() },
        title = { Text(title) },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Text(
                    text = action.folder.lockLabel ?: action.folder.name,
                    color = EglTokens.Foreground,
                    fontSize = 16.sp,
                )
                Text(text = hint, color = EglTokens.MutedForeground, fontSize = 13.sp)
                when (action) {
                    is LockAction.Lock -> {
                        PasswordField(R.string.lock_password, password) { password = it }
                        PasswordField(R.string.lock_confirm_password, confirm) { confirm = it }
                        LabelFields(label, color, onLabel = { label = it }, onColor = { color = it })
                    }
                    is LockAction.Unlock, is LockAction.RemoveLock, is LockAction.RenewKey -> {
                        PasswordField(R.string.lock_password, password) { password = it }
                    }
                    is LockAction.Recover -> {
                        OutlinedTextField(
                            value = recovery,
                            onValueChange = { recovery = it },
                            label = { Text(stringResource(R.string.lock_recovery)) },
                            modifier = Modifier.fillMaxWidth(),
                            minLines = 2,
                        )
                        PasswordField(R.string.lock_new_password, password) { password = it }
                        PasswordField(R.string.lock_confirm_password, confirm) { confirm = it }
                    }
                    is LockAction.ChangePassword -> {
                        PasswordField(R.string.lock_current_password, current) { current = it }
                        PasswordField(R.string.lock_new_password, password) { password = it }
                        PasswordField(R.string.lock_confirm_password, confirm) { confirm = it }
                    }
                    is LockAction.Label -> {
                        LabelFields(label, color, onLabel = { label = it }, onColor = { color = it })
                    }
                }
                if (wrong) {
                    Text(
                        text = stringResource(R.string.lock_wrong_password),
                        color = Color(0xFFDC2626),
                        fontSize = 13.sp,
                    )
                }
                if (working) {
                    CircularProgressIndicator(
                        modifier = Modifier
                            .padding(top = 8.dp)
                            .align(Alignment.CenterHorizontally),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = ready && !working,
                onClick = {
                    working = true
                    wrong = false
                    val id = action.folder.id
                    val passwordValue = password
                    val currentValue = current
                    val recoveryValue = recovery.trim()
                    val labelValue = label.trim().ifEmpty { null }
                    val colorValue = color
                    scope.launch {
                        try {
                            when (action) {
                                is LockAction.Lock -> {
                                    val made = open.lockWork {
                                        it.lockFolder(id, passwordValue, labelValue, colorValue)
                                    }.getOrElse { error ->
                                        wrong = error is EglError && error.code == EglError.Code.WRONG_PASSWORD
                                        return@launch
                                    }
                                    key = made
                                }
                                is LockAction.Unlock -> {
                                    finish(
                                        open.lockWork { it.unlockFolder(id, passwordValue) },
                                        onWrong = { wrong = true },
                                        onOk = onDismiss,
                                    )
                                }
                                is LockAction.Recover -> {
                                    finish(
                                        open.lockWork {
                                            it.recoverFolder(id, recoveryValue, passwordValue)
                                        },
                                        onWrong = { wrong = true },
                                        onOk = onDismiss,
                                    )
                                }
                                is LockAction.ChangePassword -> {
                                    finish(
                                        open.lockWork {
                                            it.changeFolderPassword(id, currentValue, passwordValue)
                                        },
                                        onWrong = { wrong = true },
                                        onOk = onDismiss,
                                    )
                                }
                                is LockAction.RenewKey -> {
                                    val made = open.lockWork {
                                        it.renewRecoveryKey(id, passwordValue)
                                    }.getOrElse { error ->
                                        wrong = error is EglError && error.code == EglError.Code.WRONG_PASSWORD
                                        return@launch
                                    }
                                    key = made
                                }
                                is LockAction.RemoveLock -> {
                                    finish(
                                        open.lockWork { it.removeFolderLock(id, passwordValue) },
                                        onWrong = { wrong = true },
                                        onOk = onDismiss,
                                    )
                                }
                                is LockAction.Label -> {
                                    finish(
                                        open.lockWork { it.setLockLabel(id, labelValue, colorValue) },
                                        onWrong = { wrong = true },
                                        onOk = onDismiss,
                                    )
                                }
                            }
                        } finally {
                            working = false
                        }
                    }
                },
            ) { Text(confirmTitle) }
        },
        dismissButton = {
            TextButton(enabled = !working, onClick = onDismiss) {
                Text(stringResource(R.string.common_cancel))
            }
        },
    )
}

@Composable
private fun RecoveryKeyDialog(
    recoveryKey: RecoveryKey,
    onDone: () -> Unit,
    onCopy: (String) -> Unit,
) {
    var saved by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = {},
        title = { Text(stringResource(R.string.lock_key)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(stringResource(R.string.lock_key_hint), color = EglTokens.MutedForeground, fontSize = 13.sp)
                Text(stringResource(R.string.lock_phrase), color = EglTokens.MutedForeground, fontSize = 12.sp)
                Text(recoveryKey.phrase, fontFamily = FontFamily.Monospace, color = EglTokens.Foreground)
                TextButton(onClick = { onCopy(recoveryKey.phrase) }) {
                    Text(stringResource(R.string.lock_copy_phrase))
                }
                Text(stringResource(R.string.lock_key), color = EglTokens.MutedForeground, fontSize = 12.sp)
                Text(recoveryKey.key, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = EglTokens.Foreground)
                TextButton(onClick = { onCopy(recoveryKey.key) }) {
                    Text(stringResource(R.string.lock_copy_key))
                }
                TextButton(onClick = { saved = !saved }) {
                    Text(
                        if (saved) {
                            stringResource(R.string.lock_saved_it) + " ✓"
                        } else {
                            stringResource(R.string.lock_saved_it)
                        },
                    )
                }
            }
        },
        confirmButton = {
            TextButton(enabled = saved, onClick = onDone) {
                Text(stringResource(R.string.common_done))
            }
        },
    )
}

@Composable
private fun PasswordField(labelRes: Int, value: String, onChange: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(stringResource(labelRes)) },
        visualTransformation = PasswordVisualTransformation(),
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
    )
}

@Composable
private fun LabelFields(
    label: String,
    color: String?,
    onLabel: (String) -> Unit,
    onColor: (String?) -> Unit,
) {
    OutlinedTextField(
        value = label,
        onValueChange = onLabel,
        label = { Text(stringResource(R.string.lock_label)) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
    )
    Text(stringResource(R.string.lock_label_title), color = EglTokens.MutedForeground, fontSize = 12.sp)
    ColorChoice(color = color, onColor = onColor)
}

private fun finish(
    result: Result<Unit>,
    onWrong: () -> Unit,
    onOk: () -> Unit,
) {
    result.fold(
        onSuccess = { onOk() },
        onFailure = { error ->
            if (error is EglError && error.code == EglError.Code.WRONG_PASSWORD) {
                onWrong()
            }
        },
    )
}

/** Copies a secret for this device; the system clipboard holds it briefly. */
private fun copySecret(context: Context, text: String) {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    clipboard.setPrimaryClip(ClipData.newPlainText("egl-viewer", text))
}
