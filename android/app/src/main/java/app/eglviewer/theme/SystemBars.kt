package app.eglviewer.theme

import android.graphics.Color as AndroidColor
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

/**
 * Edge-to-edge with transparent system bars whose icon/content colours follow the resolved
 * night mode (D-120). Pair with [eglSafeDrawingPadding] so chrome is not drawn under the bars.
 */
fun ComponentActivity.enableEglEdgeToEdge() {
    enableEdgeToEdge(
        statusBarStyle = SystemBarStyle.auto(
            lightScrim = AndroidColor.TRANSPARENT,
            darkScrim = AndroidColor.TRANSPARENT,
        ),
        navigationBarStyle = SystemBarStyle.auto(
            lightScrim = AndroidColor.TRANSPARENT,
            darkScrim = AndroidColor.TRANSPARENT,
        ),
    )
}

/**
 * Keeps status / navigation bar icon colours in sync with the active Compose theme.
 * Pass [darkIcons] = false on always-dark surfaces (preview stage).
 */
@Composable
fun EglSystemBarIcons(darkIcons: Boolean = !isSystemInDarkTheme()) {
    val view = LocalView.current
    if (view.isInEditMode) return
    SideEffect {
        val window = (view.context as? ComponentActivity)?.window ?: return@SideEffect
        val controller = WindowCompat.getInsetsController(window, view)
        controller.isAppearanceLightStatusBars = darkIcons
        controller.isAppearanceLightNavigationBars = darkIcons
    }
}

/** Pads content for status, nav and display cutout so it does not sit under system bars. */
@Composable
fun Modifier.eglSafeDrawingPadding(): Modifier =
    windowInsetsPadding(WindowInsets.safeDrawing)
