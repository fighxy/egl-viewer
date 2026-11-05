package app.eglviewer.settings

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
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
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.filled.MenuBook
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.HealthAndSafety
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.app.OpenLibrary
import app.eglviewer.locks.AppLock
import app.eglviewer.locks.AppLockFormDialog
import app.eglviewer.theme.EglTokens
import java.util.Locale

private sealed class SettingsDest {
    data object Hub : SettingsDest()
    data object LibraryInfo : SettingsDest()
    data object Backups : SettingsDest()
    data object Check : SettingsDest()
    data object Export : SettingsDest()
}

/**
 * Settings (Android A7 ≈ D-099): the open library's upkeep, then the app's own (login,
 * version, language). From Welcome only the app's part shows.
 */
@Composable
fun SettingsScreen(
    open: OpenLibrary?,
    lock: AppLock,
    onBack: () -> Unit,
) {
    var dest by remember { mutableStateOf<SettingsDest>(SettingsDest.Hub) }
    var showAppLock by remember { mutableStateOf(false) }
    val context = LocalContext.current

    when (dest) {
        SettingsDest.Hub -> {
            SettingsHub(
                open = open,
                lock = lock,
                version = appVersion(context),
                language = currentLanguageLabel(),
                onLibrary = { dest = SettingsDest.LibraryInfo },
                onBackups = { dest = SettingsDest.Backups },
                onCheck = { dest = SettingsDest.Check },
                onExport = { dest = SettingsDest.Export },
                onAppLock = { showAppLock = true },
                onChangeLanguage = {
                    val intent = languageSettingsIntent(context.packageName)
                    runCatching { context.startActivity(intent) }
                },
                onBack = onBack,
            )
        }
        SettingsDest.LibraryInfo -> {
            if (open != null) {
                LibraryInfoScreen(open = open, onBack = { dest = SettingsDest.Hub })
            } else {
                dest = SettingsDest.Hub
            }
        }
        SettingsDest.Backups -> {
            if (open != null) {
                BackupsScreen(open = open, onBack = { dest = SettingsDest.Hub })
            } else {
                dest = SettingsDest.Hub
            }
        }
        SettingsDest.Check -> {
            if (open != null) {
                CheckScreen(open = open, onBack = { dest = SettingsDest.Hub })
            } else {
                dest = SettingsDest.Hub
            }
        }
        SettingsDest.Export -> {
            if (open != null) {
                ExportScreen(open = open, onBack = { dest = SettingsDest.Hub })
            } else {
                dest = SettingsDest.Hub
            }
        }
    }

    if (showAppLock) {
        AppLockFormDialog(
            lock = lock,
            onDismiss = { showAppLock = false },
        )
    }
}

@Composable
private fun SettingsHub(
    open: OpenLibrary?,
    lock: AppLock,
    version: String,
    language: String,
    onLibrary: () -> Unit,
    onBackups: () -> Unit,
    onCheck: () -> Unit,
    onExport: () -> Unit,
    onAppLock: () -> Unit,
    onChangeLanguage: () -> Unit,
    onBack: () -> Unit,
) {
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
            IconButton(onClick = onBack) {
                Icon(
                    Icons.AutoMirrored.Filled.ArrowBack,
                    contentDescription = stringResource(R.string.common_done),
                    tint = EglTokens.Foreground,
                )
            }
            Text(
                text = stringResource(R.string.settings_title),
                color = EglTokens.Foreground,
                fontSize = EglTokens.TitleSp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.weight(1f),
            )
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            if (open != null) {
                SectionHeader(stringResource(R.string.settings_library_section))
                SettingsRow(
                    icon = Icons.AutoMirrored.Filled.MenuBook,
                    title = open.info.name,
                    subtitle = stringResource(R.string.settings_library_detail),
                    onClick = onLibrary,
                )
                SettingsRow(
                    icon = Icons.Default.Build,
                    title = stringResource(R.string.settings_backups),
                    onClick = onBackups,
                )
                SettingsRow(
                    icon = Icons.Default.HealthAndSafety,
                    title = stringResource(R.string.settings_check),
                    onClick = onCheck,
                )
                SettingsRow(
                    icon = Icons.Default.Share,
                    title = stringResource(R.string.settings_export),
                    onClick = onExport,
                )
                Spacer(modifier = Modifier.height(16.dp))
            }

            SectionHeader(stringResource(R.string.settings_app_section))
            SettingsRow(
                icon = Icons.Default.Lock,
                title = stringResource(R.string.app_lock_title),
                trailing = stringResource(
                    if (lock.enabled) R.string.settings_on else R.string.settings_off,
                ),
                onClick = onAppLock,
            )
            LabeledValue(
                title = stringResource(R.string.settings_version),
                value = version,
            )
            LabeledValue(
                title = stringResource(R.string.settings_language),
                value = language,
            )
            SettingsRow(
                icon = null,
                title = stringResource(R.string.settings_change_language),
                onClick = onChangeLanguage,
            )
        }
    }
}

@Composable
internal fun SectionHeader(title: String) {
    Text(
        text = title,
        color = EglTokens.MutedForeground,
        fontSize = 12.sp,
        fontWeight = FontWeight.SemiBold,
        modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
    )
}

@Composable
internal fun SettingsRow(
    icon: ImageVector?,
    title: String,
    subtitle: String? = null,
    trailing: String? = null,
    onClick: (() -> Unit)? = null,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, tint = EglTokens.MutedForeground)
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                color = EglTokens.Foreground,
                fontSize = 15.sp,
                fontWeight = FontWeight.Medium,
            )
            if (subtitle != null) {
                Text(
                    text = subtitle,
                    color = EglTokens.MutedForeground,
                    fontSize = 12.sp,
                )
            }
        }
        if (trailing != null) {
            Text(text = trailing, color = EglTokens.MutedForeground, fontSize = 14.sp)
        }
        if (onClick != null) {
            Icon(
                Icons.AutoMirrored.Filled.KeyboardArrowRight,
                contentDescription = null,
                tint = EglTokens.MutedForeground,
            )
        }
    }
    HorizontalDivider(color = EglTokens.Border.copy(alpha = 0.4f))
}

@Composable
internal fun LabeledValue(title: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text = title, color = EglTokens.Foreground, fontSize = 15.sp)
        Text(text = value, color = EglTokens.MutedForeground, fontSize = 14.sp)
    }
    HorizontalDivider(color = EglTokens.Border.copy(alpha = 0.4f))
}

@Composable
internal fun SettingsTopBar(title: String, onBack: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack) {
            Icon(
                Icons.AutoMirrored.Filled.ArrowBack,
                contentDescription = stringResource(R.string.common_done),
                tint = EglTokens.Foreground,
            )
        }
        Text(
            text = title,
            color = EglTokens.Foreground,
            fontSize = EglTokens.TitleSp,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.weight(1f),
        )
    }
}

internal fun formatBytes(bytes: Long): String {
    if (bytes < 1024) return "$bytes B"
    val units = arrayOf("KB", "MB", "GB", "TB")
    var value = bytes.toDouble() / 1024.0
    var i = 0
    while (value >= 1024.0 && i < units.lastIndex) {
        value /= 1024.0
        i += 1
    }
    return String.format(Locale.US, "%.1f %s", value, units[i])
}

private fun appVersion(context: android.content.Context): String {
    val info = runCatching {
        context.packageManager.getPackageInfo(context.packageName, 0)
    }.getOrNull()
    val name = info?.versionName ?: "0.0.0"
    @Suppress("DEPRECATION")
    val code = if (Build.VERSION.SDK_INT >= 28) {
        info?.longVersionCode?.toString() ?: "0"
    } else {
        info?.versionCode?.toString() ?: "0"
    }
    return "$name ($code)"
}

private fun currentLanguageLabel(): String {
    val locale = Locale.getDefault()
    return locale.getDisplayLanguage(locale).ifEmpty { locale.language }
}

private fun languageSettingsIntent(packageName: String): Intent {
    if (Build.VERSION.SDK_INT >= 33) {
        return Intent(Settings.ACTION_APP_LOCALE_SETTINGS).apply {
            data = Uri.fromParts("package", packageName, null)
        }
    }
    return Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
        data = Uri.fromParts("package", packageName, null)
    }
}
