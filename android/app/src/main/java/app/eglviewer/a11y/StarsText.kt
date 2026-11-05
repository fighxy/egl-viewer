package app.eglviewer.a11y

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.theme.EglTokens

/** A rating as stars, read by TalkBack as "3 stars" rather than the symbols (A8 ≈ D-100). */
@Composable
fun StarsText(
    stars: Int,
    modifier: Modifier = Modifier,
    fontSize: TextUnit = 14.sp,
) {
    val label = if (stars > 0) {
        pluralStringResource(R.plurals.details_stars, stars, stars)
    } else {
        ""
    }
    Text(
        text = "★".repeat(stars.coerceIn(0, 5)),
        color = EglTokens.Foreground,
        fontSize = fontSize,
        modifier = modifier.semantics {
            if (label.isNotEmpty()) contentDescription = label
        },
    )
}
