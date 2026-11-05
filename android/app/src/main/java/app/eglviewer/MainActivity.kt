package app.eglviewer

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import app.eglviewer.app.AppModel
import app.eglviewer.screens.LibraryScreen
import app.eglviewer.screens.WelcomeScreen
import app.eglviewer.theme.EglTokens

class MainActivity : ComponentActivity() {
    private val model: AppModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            EglApp(model)
        }
    }
}

@Composable
fun EglApp(model: AppModel) {
    androidx.compose.foundation.layout.Box(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        when (val screen = model.screen) {
            is AppModel.Screen.Welcome -> WelcomeScreen(model)
            is AppModel.Screen.LibraryOpen -> LibraryScreen(
                open = screen.open,
                onClose = { model.close() },
            )
        }
    }
}
