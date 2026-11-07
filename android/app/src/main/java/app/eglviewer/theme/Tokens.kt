package app.eglviewer.theme

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Desktop-aligned colour roles from `ui/src/index.css` / D-028.
 * Light and dark sets; the active set is provided by [EglTheme] via [LocalEglColors].
 */
data class EglColors(
    val background: Color,
    val foreground: Color,
    val primary: Color,
    val primaryForeground: Color,
    val muted: Color,
    val mutedForeground: Color,
    val sidebar: Color,
    val sidebarAccent: Color,
    val grid: Color,
    val border: Color,
    val ring: Color,
    val destructive: Color,
    val success: Color,
    val warning: Color,
)

/** Light palette — matches `:root` in `ui/src/index.css`. */
val LightEglColors = EglColors(
    background = Color.White,
    foreground = Color(0xFF171717),
    primary = Color(0xFF262626),
    primaryForeground = Color(0xFFFAFAFA),
    muted = Color(0xFFF5F5F5),
    mutedForeground = Color(0xFF737373),
    sidebar = Color(0xFFFAFAFA),
    sidebarAccent = Color(0xFFF5F5F5),
    grid = Color(0xFFF0F0F0),
    border = Color(0xFFEBEBEB),
    ring = Color(0xFFA3A3A3),
    destructive = Color(0xFFDC2626),
    success = Color(0xFF15803D),
    warning = Color(0xFFB45309),
)

/** Dark palette — matches `.dark` in `ui/src/index.css`. */
val DarkEglColors = EglColors(
    background = Color(0xFF171717),
    foreground = Color(0xFFFAFAFA),
    primary = Color(0xFFEBEBEB),
    primaryForeground = Color(0xFF262626),
    muted = Color(0xFF404040),
    mutedForeground = Color(0xFFA3A3A3),
    sidebar = Color(0xFF262626),
    sidebarAccent = Color(0xFF404040),
    grid = Color(0xFF2E2E2E),
    border = Color.White.copy(alpha = 0.10f),
    ring = Color(0xFF8A8A8A),
    destructive = Color(0xFFF87171),
    success = Color(0xFF4ADE80),
    warning = Color(0xFFFBBF24),
)

val LocalEglColors = staticCompositionLocalOf { LightEglColors }

/**
 * Theme tokens used by screens. Colour roles resolve from [LocalEglColors].
 * [PreviewStage] stays dark in both modes (desktop / iOS preview stage).
 */
object EglTokens {
    val Background: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.background
    val Foreground: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.foreground
    val Primary: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.primary
    val PrimaryForeground: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.primaryForeground
    val Muted: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.muted
    val MutedForeground: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.mutedForeground
    val Sidebar: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.sidebar
    val SidebarAccent: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.sidebarAccent
    val Grid: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.grid
    val Border: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.border
    val Ring: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.ring
    val Destructive: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.destructive
    val Success: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.success
    val Warning: Color
        @Composable @ReadOnlyComposable get() = LocalEglColors.current.warning

    /** Full-screen preview stage — always dark (D-028 / iOS Tokens). */
    val PreviewStage = Color(0xFF0A0A0A)

    /** Ghost controls on the dark preview stage (always light fill). */
    val GhostFill = Color.White.copy(alpha = 0.10f)
    val GhostFillSoft = Color.White.copy(alpha = 0.08f)

    val ChromeSp = 13.sp
    val TitleSp = 17.sp
    val HeroSp = 28.sp
    val CaptionSp = 11.sp
    val TabLabelSp = 10.sp

    val RadiusLg = 12.dp
    val RadiusMd = 10.dp
    val RadiusSm = 8.dp
    val RadiusXs = 6.dp
    val ChipHeight = 28.dp
    val FilterPillHeight = 26.dp
    val ButtonHeight = 48.dp
    val GridGutter = 3.dp
}
