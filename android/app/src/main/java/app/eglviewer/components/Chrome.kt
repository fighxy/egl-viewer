package app.eglviewer.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.theme.EglTokens

/** Places strip chip — muted accent when active (concept board, not Photos black). */
@Composable
fun PlaceChip(
    label: String,
    count: String? = null,
    active: Boolean,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .height(EglTokens.ChipHeight)
            .background(
                color = if (active) EglTokens.SidebarAccent else Color.Transparent,
                shape = RoundedCornerShape(EglTokens.RadiusSm),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(
            text = label,
            color = EglTokens.Foreground,
            fontSize = 12.sp,
            fontWeight = if (active) FontWeight.SemiBold else FontWeight.Medium,
        )
        if (count != null) {
            Text(
                text = count,
                color = EglTokens.MutedForeground,
                fontSize = EglTokens.CaptionSp,
            )
        }
    }
}

/** Filter bar pill — solid primary when applied (desktop FilterBar DNA). */
@Composable
fun FilterPill(
    label: String,
    active: Boolean,
    onClick: () -> Unit = {},
) {
    Text(
        text = label,
        color = if (active) EglTokens.PrimaryForeground else EglTokens.Foreground,
        fontSize = 12.sp,
        modifier = Modifier
            .background(
                color = if (active) EglTokens.Primary else EglTokens.Background,
                shape = RoundedCornerShape(7.dp),
            )
            .border(
                width = 1.dp,
                color = if (active) EglTokens.Primary else EglTokens.Border,
                shape = RoundedCornerShape(7.dp),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 5.dp),
    )
}

/** Concept segmented control (Details | Tags sheet | Folders). */
@Composable
fun SegmentedControl(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(EglTokens.Muted, RoundedCornerShape(EglTokens.RadiusSm))
            .padding(2.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        options.forEachIndexed { index, title ->
            val on = selectedIndex == index
            Box(
                modifier = Modifier
                    .weight(1f)
                    .height(28.dp)
                    .background(
                        color = if (on) EglTokens.Background else Color.Transparent,
                        shape = RoundedCornerShape(EglTokens.RadiusXs),
                    )
                    .clickable { onSelect(index) },
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = title,
                    fontSize = 12.sp,
                    fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal,
                    color = if (on) EglTokens.Foreground else EglTokens.MutedForeground,
                )
            }
        }
    }
}

@Composable
fun GhostIconButton(
    onClick: () -> Unit,
    size: Dp = 44.dp,
    radius: Dp = 12.dp,
    fill: Color = EglTokens.GhostFill,
    content: @Composable () -> Unit,
) {
    Box(
        modifier = Modifier
            .size(size)
            .background(fill, RoundedCornerShape(radius))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}
