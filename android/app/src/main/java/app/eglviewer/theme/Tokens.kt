package app.eglviewer.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Desktop-aligned light tokens from `ui/src/index.css` / D-028
 * (ios-ui-concept.md + iPhone MVP HTML board).
 */
object EglTokens {
    val Background = Color.White
    val Foreground = Color(0xFF171717)
    val Primary = Color(0xFF262626)
    val PrimaryForeground = Color(0xFFFAFAFA)
    val Muted = Color(0xFFF5F5F5)
    val MutedForeground = Color(0xFF737373)
    val Sidebar = Color(0xFFFAFAFA)
    val SidebarAccent = Color(0xFFF5F5F5)
    val Grid = Color(0xFFF0F0F0)
    val Border = Color(0xFFEBEBEB)
    val Ring = Color(0xFFA3A3A3)
    val PreviewStage = Color(0xFF0A0A0A)
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
