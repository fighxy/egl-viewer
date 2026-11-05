package app.eglviewer.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.browse.ActionToast
import app.eglviewer.theme.EglTokens

/** Snackbar after an action; optional Undo when the core can take it back. */
@Composable
fun UndoSnackbar(
    toast: ActionToast?,
    onUndo: () -> Unit,
    modifier: Modifier = Modifier,
) {
    if (toast == null) return
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp)
            .background(EglTokens.Primary, RoundedCornerShape(10.dp))
            .padding(horizontal = 14.dp, vertical = 10.dp)
            .testTag("toast"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = toast.message,
            color = EglTokens.PrimaryForeground,
            fontSize = 14.sp,
            modifier = Modifier.weight(1f),
        )
        if (toast.undoable) {
            TextButton(
                onClick = onUndo,
                modifier = Modifier.testTag("toast.undo"),
            ) {
                Text(
                    text = stringResource(R.string.toast_undo),
                    color = EglTokens.PrimaryForeground,
                )
            }
        }
    }
}
