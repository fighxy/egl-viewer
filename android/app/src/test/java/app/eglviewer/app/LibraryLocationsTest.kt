package app.eglviewer.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Host JVM tests for library location heuristics (D-101 / A1). */
class LibraryLocationsTest {
    @Test
    fun localPrimaryStorageIsNotSynced() {
        assertFalse(LibraryLocations.looksSynced("/storage/emulated/0/Pictures/Demo.eglib"))
        assertFalse(LibraryLocations.looksSynced("content://com.android.externalstorage.documents/tree/primary%3ADemo.eglib"))
    }

    @Test
    fun cloudProvidersCountAsSynced() {
        assertTrue(LibraryLocations.looksSynced("content://com.google.android.apps.docs.storage/tree/…"))
        assertTrue(LibraryLocations.looksSynced("/storage/emulated/0/Android/data/com.dropbox.android/…"))
        assertTrue(LibraryLocations.looksSynced("OneDrive/Libraries/Art.eglib"))
        assertTrue(LibraryLocations.looksSynced("…/File Provider Storage/Library.eglib"))
    }
}
