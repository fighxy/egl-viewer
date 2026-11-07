package app.eglviewer.locks

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.fragment.app.FragmentActivity
import app.eglviewer.R
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.delay

/** The app's login over everything (Android A6 ≈ D-098): the password or PIN, or biometrics. */
@Composable
fun LoginScreen(lock: AppLock) {
    var password by remember { mutableStateOf("") }
    var wrong by remember { mutableStateOf(false) }
    var waitLeft by remember { mutableIntStateOf(lock.waitSecondsLeft()) }

    // Stay on the lock screen; do not walk the underlying stack or exit (D-120).
    BackHandler { /* consume */ }
    val context = LocalContext.current
    val activity = context as? FragmentActivity

    LaunchedEffect(lock.waitUntilMs) {
        while (true) {
            waitLeft = lock.waitSecondsLeft()
            if (waitLeft <= 0) break
            delay(1_000)
        }
    }

    LaunchedEffect(lock.biometryEnabled, activity) {
        if (lock.biometryEnabled && activity != null) {
            lock.unlockWithBiometry(
                activity = activity,
                reason = context.getString(R.string.login_biometry_reason),
            ) { /* focus password field on failure; Compose has no FocusRequester needed here */ }
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background)
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(
            Icons.Default.Lock,
            contentDescription = null,
            tint = EglTokens.MutedForeground,
            modifier = Modifier.size(44.dp),
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = stringResource(R.string.login_title),
            color = EglTokens.Foreground,
            fontSize = 20.sp,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(modifier = Modifier.height(20.dp))
        OutlinedTextField(
            value = password,
            onValueChange = { password = it },
            label = { Text(stringResource(R.string.login_password)) },
            visualTransformation = PasswordVisualTransformation(),
            singleLine = true,
            enabled = waitLeft <= 0,
            modifier = Modifier
                .widthIn(max = 320.dp)
                .fillMaxWidth(),
        )
        Spacer(modifier = Modifier.height(8.dp))
        when {
            waitLeft > 0 -> {
                Text(
                    text = pluralStringResource(R.plurals.login_wait, waitLeft, waitLeft),
                    color = EglTokens.MutedForeground,
                    fontSize = 13.sp,
                )
            }
            wrong -> {
                Text(
                    text = stringResource(R.string.login_wrong),
                    color = EglTokens.Destructive,
                    fontSize = 13.sp,
                )
            }
        }
        Spacer(modifier = Modifier.height(16.dp))
        Button(
            onClick = {
                if (password.isEmpty()) return@Button
                wrong = !lock.unlock(password)
                password = ""
            },
            enabled = password.isNotEmpty() && waitLeft <= 0,
            colors = ButtonDefaults.buttonColors(
                containerColor = EglTokens.Primary,
                contentColor = EglTokens.PrimaryForeground,
            ),
            shape = RoundedCornerShape(EglTokens.RadiusMd),
        ) {
            Text(stringResource(R.string.login_unlock))
        }
        if (lock.biometryEnabled && activity != null) {
            Spacer(modifier = Modifier.height(8.dp))
            TextButton(
                onClick = {
                    lock.unlockWithBiometry(
                        activity = activity,
                        reason = context.getString(R.string.login_biometry_reason),
                    ) { }
                },
            ) {
                Icon(Icons.Default.Fingerprint, contentDescription = null)
                Spacer(modifier = Modifier.size(8.dp))
                Text(stringResource(R.string.login_biometry))
            }
        }
    }
}

/** What hides the app in Recents while a login is set. */
@Composable
fun PrivacyCover() {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            Icons.Default.Lock,
            contentDescription = null,
            tint = EglTokens.MutedForeground,
            modifier = Modifier.size(44.dp),
        )
    }
}
