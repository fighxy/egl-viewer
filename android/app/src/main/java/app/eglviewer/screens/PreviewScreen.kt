package app.eglviewer.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.KeyboardArrowLeft
import androidx.compose.material.icons.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Remove
import androidx.compose.material.icons.filled.RotateRight
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.bridge.LibraryBridge
import app.eglviewer.theme.EglTokens

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PreviewScreen(
    bridge: LibraryBridge,
    itemId: String,
    onClose: () -> Unit,
    onStep: (String) -> Unit,
    editContent: @Composable (itemId: String, onDismiss: () -> Unit) -> Unit,
) {
    val item = bridge.items.firstOrNull { it.id == itemId }
    val index = bridge.items.indexOfFirst { it.id == itemId }
    val indexLabel = if (index >= 0) "${index + 1} / ${bridge.total}" else item?.name ?: itemId
    var showEdit by remember { mutableStateOf(false) }
    var scale by remember { mutableFloatStateOf(1f) }
    var rotation by remember { mutableFloatStateOf(0f) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = false)

    val aspect = run {
        val w = (item?.width ?: 3).toFloat()
        val h = (item?.height ?: 2).toFloat().coerceAtLeast(1f)
        w / h
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.PreviewStage),
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onClose) {
                    Icon(Icons.Default.Close, contentDescription = "Close", tint = Color.White)
                }
                Spacer(modifier = Modifier.weight(1f))
                Text(
                    text = item?.name ?: "Item",
                    color = Color.White.copy(alpha = 0.9f),
                    fontSize = EglTokens.ChromeSp,
                )
                Spacer(modifier = Modifier.weight(1f))
                IconButton(onClick = { showEdit = true }) {
                    Icon(Icons.Default.Info, contentDescription = "Info", tint = Color.White)
                }
            }

            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(horizontal = 20.dp)
                    .pointerInput(Unit) {
                        detectTransformGestures { _, _, zoom, _ ->
                            scale = (scale * zoom).coerceIn(1f, 4f)
                        }
                    }
                    .pointerInput(Unit) {
                        detectTapGestures(
                            onDoubleTap = { scale = if (scale > 1f) 1f else 2f },
                        )
                    },
                contentAlignment = Alignment.Center,
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .aspectRatio(aspect)
                        .graphicsLayer {
                            scaleX = scale
                            scaleY = scale
                            rotationZ = rotation
                        }
                        .background(Color.White.copy(alpha = 0.12f), RoundedCornerShape(2.dp)),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = (item?.ext ?: "img").uppercase(),
                        color = Color.White.copy(alpha = 0.55f),
                        fontSize = 28.sp,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
            }

            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 8.dp),
                horizontalArrangement = Arrangement.SpaceEvenly,
            ) {
                IconButton(onClick = {
                    if (index > 0) onStep(bridge.items[index - 1].id)
                }) {
                    Icon(Icons.Default.KeyboardArrowLeft, null, tint = Color.White)
                }
                IconButton(onClick = { scale = (scale - 0.25f).coerceAtLeast(1f) }) {
                    Icon(Icons.Default.Remove, null, tint = Color.White)
                }
                IconButton(onClick = { scale = (scale + 0.25f).coerceAtMost(4f) }) {
                    Icon(Icons.Default.Add, null, tint = Color.White)
                }
                IconButton(onClick = { rotation += 90f }) {
                    Icon(Icons.Default.RotateRight, null, tint = Color.White)
                }
                IconButton(onClick = {
                    if (index in 0 until bridge.items.lastIndex) onStep(bridge.items[index + 1].id)
                }) {
                    Icon(Icons.Default.KeyboardArrowRight, null, tint = Color.White)
                }
            }
            Text(
                text = indexLabel,
                color = Color.White.copy(alpha = 0.7f),
                fontSize = EglTokens.ChromeSp,
                modifier = Modifier
                    .align(Alignment.CenterHorizontally)
                    .padding(bottom = 20.dp),
            )
        }
    }

    if (showEdit) {
        ModalBottomSheet(
            onDismissRequest = { showEdit = false },
            sheetState = sheetState,
            containerColor = EglTokens.Background,
        ) {
            editContent(itemId) { showEdit = false }
        }
    }
}
