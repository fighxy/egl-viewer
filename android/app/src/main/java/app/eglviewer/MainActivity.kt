package app.eglviewer

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import app.eglviewer.bridge.LibraryBridge
import app.eglviewer.ui.screens.BrowseScreen
import app.eglviewer.ui.screens.EditSheet
import app.eglviewer.ui.screens.PreviewScreen
import app.eglviewer.ui.screens.WelcomeScreen
import app.eglviewer.ui.theme.EglTokens

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            EglApp()
        }
    }
}

@Composable
fun EglApp() {
    val bridge = remember { LibraryBridge() }
    val nav = rememberNavController()
    NavHost(
        navController = nav,
        startDestination = "welcome",
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        composable("welcome") {
            WelcomeScreen(
                bridge = bridge,
                onOpenBrowse = {
                    nav.navigate("browse") {
                        launchSingleTop = true
                    }
                },
            )
        }
        composable("browse") {
            BrowseScreen(
                bridge = bridge,
                onBackToLibraries = {
                    bridge.close()
                    nav.popBackStack("welcome", inclusive = false)
                },
                onOpenPreview = { id -> nav.navigate("preview/$id") },
            )
        }
        composable(
            route = "preview/{itemId}",
            arguments = listOf(navArgument("itemId") { type = NavType.StringType }),
        ) { entry ->
            val itemId = entry.arguments?.getString("itemId").orEmpty()
            PreviewScreen(
                bridge = bridge,
                itemId = itemId,
                onClose = { nav.popBackStack() },
                onStep = { nextId ->
                    nav.navigate("preview/$nextId") {
                        popUpTo("browse")
                        launchSingleTop = true
                    }
                },
                editContent = { id, onDismiss ->
                    EditSheet(bridge = bridge, itemId = id, onDismiss = onDismiss)
                },
            )
        }
    }
}
