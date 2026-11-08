package app.eglviewer

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.appcompat.app.AppCompatActivity
import androidx.compose.ui.Modifier
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import app.eglviewer.app.AppModel
import app.eglviewer.locks.AppLock
import app.eglviewer.locks.LoginScreen
import app.eglviewer.locks.PrivacyCover
import app.eglviewer.screens.LibraryScreen
import app.eglviewer.screens.WelcomeScreen
import app.eglviewer.theme.EglTheme
import app.eglviewer.theme.EglTokens
import kotlinx.coroutines.launch

/**
 * AppCompatActivity so ThemePreference / AppCompatDelegate.setDefaultNightMode
 * recreates us and updates uiMode (D-127). Still a FragmentActivity for
 * BiometricPrompt (D-106).
 */
class MainActivity : AppCompatActivity() {
    private val model: AppModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            EglTheme {
                val lock = remember { AppLock.standard(this) }
                EglApp(model = model, lock = lock, activity = this)
            }
        }
    }
}

@Composable
fun EglApp(model: AppModel, lock: AppLock, activity: FragmentActivity) {
    val scope = rememberCoroutineScope()
    var showPrivacy by remember { mutableStateOf(false) }

    DisposableEffect(activity, lock, model) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_STOP -> {
                    lock.lock()
                    val open = model.openLibrary
                    if (open != null) {
                        scope.launch { open.relockAll() }
                    }
                }
                Lifecycle.Event.ON_PAUSE -> {
                    // Recents takes its picture now.
                    if (lock.enabled && !lock.locked) {
                        showPrivacy = true
                        activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                    }
                }
                Lifecycle.Event.ON_RESUME -> {
                    showPrivacy = false
                    // Back from the "All files access" page with a library waiting (D-111).
                    model.retryPendingAccess()
                    if (!lock.enabled) {
                        activity.window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                    } else {
                        activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                    }
                }
                else -> Unit
            }
        }
        activity.lifecycle.addObserver(observer)
        onDispose { activity.lifecycle.removeObserver(observer) }
    }

    LaunchedEffect(lock.enabled) {
        if (lock.enabled) {
            activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        } else {
            activity.window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        when (val screen = model.screen) {
            is AppModel.Screen.Welcome -> WelcomeScreen(model = model, lock = lock)
            is AppModel.Screen.LibraryOpen -> LibraryScreen(
                open = screen.open,
                lock = lock,
                onClose = { model.close() },
            )
        }
        if (lock.locked) {
            LoginScreen(lock = lock)
        } else if (showPrivacy) {
            PrivacyCover()
        }
    }
}
