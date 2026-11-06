package app.eglviewer.app

import org.junit.Assert.assertEquals
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

    /** D-116: an Eagle import never lands on a library that is already there. */
    @Test
    fun aTakenNameGetsANumber() {
        val folder = kotlin.io.path.createTempDirectory("names").toFile()
        try {
            assertEquals("Мой Eagle", LibraryLocations.freeName(folder, "Мой Eagle"))
            java.io.File(folder, "Мой Eagle.eglib").mkdirs()
            assertEquals("Мой Eagle 2", LibraryLocations.freeName(folder, "Мой Eagle"))
            java.io.File(folder, "Мой Eagle 2.eglib").mkdirs()
            assertEquals("Мой Eagle 3", LibraryLocations.freeName(folder, "Мой Eagle"))
        } finally {
            folder.deleteRecursively()
        }
    }
}
