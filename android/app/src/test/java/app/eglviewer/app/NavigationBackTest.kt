package app.eglviewer.app

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Documents the Android system-back priority (D-126). Compose [BackHandler]s follow the same
 * order; this keeps the product rule testable without a device.
 */
class NavigationBackTest {
    enum class Layer {
        Login,
        PreviewDialog,
        Preview,
        BrowseSheet,
        Selecting,
        Place,
        LibraryPlaces,
        WelcomeOverlay,
        WelcomeRoot,
    }

    /** Where system back should go from [layer]. Null = leave the activity. */
    fun destination(layer: Layer): Layer? = when (layer) {
        Layer.Login -> Layer.Login // consume; stay locked
        Layer.PreviewDialog -> Layer.Preview
        Layer.Preview -> Layer.Place
        Layer.BrowseSheet -> Layer.Place
        Layer.Selecting -> Layer.Place
        Layer.Place -> Layer.LibraryPlaces
        Layer.LibraryPlaces -> Layer.WelcomeRoot
        Layer.WelcomeOverlay -> Layer.WelcomeRoot
        Layer.WelcomeRoot -> null
    }

    @Test
    fun back_walks_in_to_welcome_then_exits() {
        assertEquals(Layer.Login, destination(Layer.Login))
        assertEquals(Layer.Preview, destination(Layer.PreviewDialog))
        assertEquals(Layer.Place, destination(Layer.Preview))
        assertEquals(Layer.Place, destination(Layer.BrowseSheet))
        assertEquals(Layer.Place, destination(Layer.Selecting))
        assertEquals(Layer.LibraryPlaces, destination(Layer.Place))
        assertEquals(Layer.WelcomeRoot, destination(Layer.LibraryPlaces))
        assertEquals(Layer.WelcomeRoot, destination(Layer.WelcomeOverlay))
        assertEquals(null, destination(Layer.WelcomeRoot))
    }
}
