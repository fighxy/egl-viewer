package app.eglviewer.adding

import app.eglviewer.core.ImportKind
import app.eglviewer.core.ImportProgress
import app.eglviewer.core.ImportState
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** Host JVM tests for picked copies, clipboard names, folder-tree walk and import summary (A4 / D-121). */
class AddingTest {
    @get:Rule
    val tmp = TemporaryFolder()

    @Test
    fun pickedCopyLivesUnderPrefixAndRemovesWithParent() {
        val cache = tmp.newFolder("cache")
        val source = tmp.newFile("photo.png").also { it.writeBytes(byteArrayOf(1, 2, 3)) }
        val picked = PickedFile.copying(cache, source)
        assertTrue(picked.file.exists())
        assertTrue(picked.file.parentFile!!.name.startsWith(PickedFile.FOLDER_PREFIX))
        assertEquals(byteArrayOf(1, 2, 3).toList(), picked.file.readBytes().toList())
        picked.remove()
        assertFalse(picked.file.exists())
        assertFalse(picked.file.parentFile!!.exists())
    }

    /** A name from another app never leaves the copy's folder (D-109). */
    @Test
    fun namesFromOtherAppsStayInTheirFolder() {
        assertEquals("x.xml", PickedFile.safeName("../../shared_prefs/x.xml"))
        assertEquals("evil.png", PickedFile.safeName("..\\..\\evil.png"))
        assertEquals("Imported", PickedFile.safeName(".."))
        assertEquals("Imported", PickedFile.safeName("/"))
        assertEquals("Imported", PickedFile.safeName(null))
        assertEquals("кот 🐱.png", PickedFile.safeName("кот 🐱.png"))
        assertTrue(PickedFile.safeName("a".repeat(400) + ".png").toByteArray().size <= 255)

        val cache = tmp.newFolder("cache")
        val folder = PickedFile.newFolder(cache)
        val target = PickedFile.child(folder, "../../shared_prefs/app.eglviewer.login.xml")
        assertEquals(folder.canonicalFile, target.canonicalFile.parentFile)
        val picked = PickedFile.writing(cache, byteArrayOf(7), "../escape.png")
        assertEquals("escape.png", picked.file.name)
        assertTrue(picked.file.parentFile!!.name.startsWith(PickedFile.FOLDER_PREFIX))
        assertFalse(File(cache, "escape.png").exists())
    }

    @Test
    fun leftoversAreRemovedAtLaunch() {
        val cache = tmp.newFolder("cache")
        File(cache, "${PickedFile.FOLDER_PREFIX}dead").mkdirs()
        File(cache, "keep-me").mkdirs()
        PickedFile.removeLeftovers(cache)
        assertFalse(File(cache, "${PickedFile.FOLDER_PREFIX}dead").exists())
        assertTrue(File(cache, "keep-me").exists())
    }

    @Test
    fun clipboardFileNames() {
        assertEquals("Pasted.png", ImportPaths.clipboardFileName(null, "png", 0))
        assertEquals("Pasted 2.png", ImportPaths.clipboardFileName("  ", "png", 1))
        assertEquals("Poster.png", ImportPaths.clipboardFileName("Poster.png", "png", 0))
        assertEquals("a-b.heic", ImportPaths.clipboardFileName("a/b", "heic", 0))
    }

    @Test
    fun importSummaryJoinsParts() {
        val progress = ImportProgress(
            id = 1,
            kind = ImportKind.FILES,
            state = ImportState.CANCELLED,
            total = 5,
            processed = 5,
            added = 3,
            duplicates = 1,
            failed = 1,
            previewsTotal = 0,
            previewsDone = 0,
            current = null,
            duplicate = null,
        )
        val summary = importSummary(
            progress = progress,
            addedLabel = { "Added $it items" },
            duplicatesLabel = { "$it duplicates skipped" },
            failedLabel = { "$it failed" },
            cancelledLabel = "stopped",
        )
        assertEquals("Added 3 items · 1 duplicates skipped · 1 failed · stopped", summary)
    }

    @Test
    fun fromPickedBatchCleansCopies() {
        val cache = tmp.newFolder("cache")
        val a = PickedFile.writing(cache, byteArrayOf(9), "a.png")
        val b = PickedFile.writing(cache, byteArrayOf(8), "b.png")
        val batch = ImportPaths.fromPicked(listOf(a, b))
        assertEquals(2, batch.paths.size)
        batch.cleanup()
        assertFalse(a.file.exists())
        assertFalse(b.file.exists())
    }

    /**
     * Folder pick walks nested folders and keeps files whose MIME is empty — the old
     * DocumentFile.isFile filter skipped those (D-121).
     */
    @Test
    fun documentTreeListsNestedFilesIncludingEmptyMime() {
        val tree = TreeChild(
            name = "Photos",
            isDirectory = true,
            children = listOf(
                TreeChild("IMG_001.JPG", isDirectory = false, mimeType = "image/jpeg"),
                TreeChild(
                    "Vacation",
                    isDirectory = true,
                    children = listOf(
                        TreeChild("beach.png", isDirectory = false, mimeType = "image/png"),
                        TreeChild(
                            "Raw",
                            isDirectory = true,
                            children = listOf(
                                TreeChild("shot.heic", isDirectory = false, mimeType = "image/heic"),
                                // Empty MIME: DocumentFile.isFile would be false and skip this.
                                TreeChild("no-mime.webp", isDirectory = false, mimeType = null),
                                TreeChild("empty-mime.jpg", isDirectory = false, mimeType = ""),
                            ),
                        ),
                    ),
                ),
                TreeChild("notes.txt", isDirectory = false, mimeType = "text/plain"),
                TreeChild(".", isDirectory = false, mimeType = "image/png"),
                TreeChild("", isDirectory = false, mimeType = "image/png"),
            ),
        )
        assertEquals(
            listOf(
                "IMG_001.JPG",
                "Vacation/beach.png",
                "Vacation/Raw/shot.heic",
                "Vacation/Raw/no-mime.webp",
                "Vacation/Raw/empty-mime.jpg",
                "notes.txt",
            ),
            collectTreeFiles(tree),
        )
    }

    @Test
    fun mimeExtensionsCoverPhoneFormats() {
        assertEquals("jpg", extensionForMime("image/jpeg"))
        assertEquals("png", extensionForMime("image/png"))
        assertEquals("heic", extensionForMime("image/heic"))
        assertEquals("heic", extensionForMime("image/heif"))
        assertEquals("webp", extensionForMime("image/webp"))
        assertEquals(null, extensionForMime("vnd.android.document/directory"))
        assertEquals(null, extensionForMime(null))
        assertEquals("photo.heic", fileNameWithMime("photo", "image/heic"))
        assertEquals("photo.png", fileNameWithMime("photo.png", "image/heic"))
        assertEquals("Imported", fileNameWithMime("..", null))
    }
}
