package app.eglviewer.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider

private fun materialScheme(colors: EglColors, dark: Boolean) = if (dark) {
    darkColorScheme(
        primary = colors.primary,
        onPrimary = colors.primaryForeground,
        secondary = colors.muted,
        onSecondary = colors.foreground,
        tertiary = colors.sidebarAccent,
        onTertiary = colors.foreground,
        background = colors.background,
        onBackground = colors.foreground,
        surface = colors.sidebar,
        onSurface = colors.foreground,
        surfaceVariant = colors.muted,
        onSurfaceVariant = colors.mutedForeground,
        outline = colors.border,
        error = colors.destructive,
        onError = colors.primaryForeground,
    )
} else {
    lightColorScheme(
        primary = colors.primary,
        onPrimary = colors.primaryForeground,
        secondary = colors.muted,
        onSecondary = colors.foreground,
        tertiary = colors.sidebarAccent,
        onTertiary = colors.foreground,
        background = colors.background,
        onBackground = colors.foreground,
        surface = colors.sidebar,
        onSurface = colors.foreground,
        surfaceVariant = colors.muted,
        onSurfaceVariant = colors.mutedForeground,
        outline = colors.border,
        error = colors.destructive,
        onError = colors.primaryForeground,
    )
}

/**
 * Provides [LocalEglColors] and a Material3 [ColorScheme] that match the resolved
 * night mode (system / light / dark via [ThemePreference] + AppCompat DayNight).
 */
@Composable
fun EglTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val colors = if (dark) DarkEglColors else LightEglColors
    CompositionLocalProvider(LocalEglColors provides colors) {
        MaterialTheme(
            colorScheme = materialScheme(colors, dark),
            content = content,
        )
    }
}
