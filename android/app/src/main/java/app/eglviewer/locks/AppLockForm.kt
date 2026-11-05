package app.eglviewer.locks

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.LockOpen
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.theme.EglTokens

/**
 * The app's login (Android A6 ≈ D-098): set a password or PIN, change it, turn it off, and
 * biometrics. On Welcome for now; Settings arrive with A7.
 */
@Composable
fun AppLockFormDialog(
    lock: AppLock,
    onDismiss: () -> Unit,
) {
    var current by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    var message by remember { mutableStateOf<String?>(null) }
    var failed by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val biometryAvailable = remember { lock.canUseBiometry(context) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.app_lock_title)) },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                androidx.compose.foundation.layout.Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Icon(
                        if (lock.enabled) Icons.Default.Lock else Icons.Default.LockOpen,
                        contentDescription = null,
                        tint = EglTokens.MutedForeground,
                    )
                    Text(
                        text = stringResource(
                            if (lock.enabled) R.string.app_lock_on else R.string.app_lock_off,
                        ),
                        color = EglTokens.Foreground,
                    )
                }
                Text(
                    text = stringResource(R.string.app_lock_hint),
                    color = EglTokens.MutedForeground,
                    fontSize = 13.sp,
                )
                if (lock.enabled) {
                    OutlinedTextField(
                        value = current,
                        onValueChange = { current = it },
                        label = { Text(stringResource(R.string.app_lock_current_password)) },
                        visualTransformation = PasswordVisualTransformation(),
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                Text(
                    text = stringResource(
                        if (lock.enabled) R.string.app_lock_change else R.string.app_lock_set,
                    ),
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                )
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it },
                    label = { Text(stringResource(R.string.app_lock_new_password)) },
                    visualTransformation = PasswordVisualTransformation(),
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = confirm,
                    onValueChange = { confirm = it },
                    label = { Text(stringResource(R.string.app_lock_confirm_password)) },
                    visualTransformation = PasswordVisualTransformation(),
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                TextButton(
                    enabled = password.isNotEmpty() &&
                        password == confirm &&
                        (!lock.enabled || current.isNotEmpty()),
                    onClick = {
                        if (lock.enabled) {
                            if (!lock.change(current = current, to = password)) {
                                message = context.getString(R.string.app_lock_wrong)
                                failed = true
                                return@TextButton
                            }
                            message = context.getString(R.string.app_lock_changed)
                        } else {
                            lock.setPassword(password)
                            message = context.getString(R.string.app_lock_set_done)
                        }
                        failed = false
                        current = ""
                        password = ""
                        confirm = ""
                    },
                ) {
                    Text(
                        stringResource(
                            if (lock.enabled) R.string.app_lock_change_action else R.string.app_lock_set_action,
                        ),
                    )
                }
                if (lock.enabled && biometryAvailable) {
                    androidx.compose.foundation.layout.Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Text(
                            text = stringResource(R.string.app_lock_biometry),
                            modifier = Modifier
                                .weight(1f)
                                .padding(end = 12.dp),
                        )
                        Switch(
                            checked = lock.biometryEnabled,
                            onCheckedChange = { on ->
                                runCatching { lock.setBiometry(on) }
                                    .onFailure {
                                        message = context.getString(R.string.app_lock_failed)
                                        failed = true
                                    }
                            },
                        )
                    }
                }
                if (lock.enabled) {
                    TextButton(
                        enabled = current.isNotEmpty(),
                        onClick = {
                            if (!lock.turnOff(current)) {
                                message = context.getString(R.string.app_lock_wrong)
                                failed = true
                                return@TextButton
                            }
                            current = ""
                            message = context.getString(R.string.app_lock_turned_off)
                            failed = false
                        },
                    ) {
                        Text(stringResource(R.string.app_lock_turn_off), color = Color(0xFFDC2626))
                    }
                }
                message?.let {
                    Text(
                        text = it,
                        color = if (failed) Color(0xFFDC2626) else EglTokens.MutedForeground,
                        fontSize = 13.sp,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.common_done))
            }
        },
    )
}
