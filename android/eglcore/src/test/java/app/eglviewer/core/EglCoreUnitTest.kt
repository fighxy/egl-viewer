package app.eglviewer.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Host JVM tests — no device, no `.so` (D-101 / A1). */
class EglCoreUnitTest {
    /** D-115: on the JVM the bridge cannot load; the error carries the loader's reason. */
    @Test
    fun aCoreThatDidNotLoadSaysWhy() {
        if (Library.isNativeAvailable) return
        val reason = Library.nativeLoadError
        assertTrue(!reason.isNullOrEmpty())
        val error = runCatching {
            Library.create(java.io.File("build/none"), "X")
        }.exceptionOrNull() as EglError
        assertEquals(EglError.Code.UNSUPPORTED, error.code)
        assertTrue(error.message.startsWith("native core did not load: "))
        assertTrue(error.message.endsWith(reason!!))
    }

    @Test
    fun errorCodesMatchCAbi() {
        assertEquals(1, EglError.Code.INVALID.raw)
        assertEquals(4, EglError.Code.NOT_A_LIBRARY.raw)
        assertEquals(8, EglError.Code.WRONG_PASSWORD.raw)
        assertEquals(9, EglError.Code.LOCKED.raw)
        assertEquals(EglError.Code.OTHER, EglError.Code.fromRaw(0))
        assertEquals(EglError.Code.LOCKED, EglError.Code.fromRaw(9))
    }

    @Test
    fun libraryInfoParsesCamelCaseJson() {
        val info = LibraryInfo.fromJson(
            """{"kind":"native","id":"abc","name":"Demo","path":"/tmp/Demo.eglib","readOnly":true,"readOnlyReason":"syncFolder"}""",
        )
        assertEquals(LibraryKind.NATIVE, info.kind)
        assertEquals("abc", info.id)
        assertEquals("Demo", info.name)
        assertEquals("/tmp/Demo.eglib", info.path)
        assertTrue(info.readOnly)
        assertEquals(ReadOnlyReason.SYNC_FOLDER, info.readOnlyReason)
    }

    @Test
    fun countsParse() {
        val counts = LibraryCounts.fromJson(
            """{"all":12,"uncategorized":3,"untagged":4,"trash":1}""",
        )
        assertEquals(12, counts.all)
        assertEquals(3, counts.uncategorized)
        assertEquals(4, counts.untagged)
        assertEquals(1, counts.trash)
    }

    @Test
    fun eventsParseByType() {
        val importEvent = LibraryEvent.fromJson(
            """{"type":"importProgress","progress":{"id":1,"kind":"files","state":"adding","total":3,"processed":1,"added":1,"duplicates":0,"failed":0,"previewsTotal":0,"previewsDone":0}}""",
        )
        assertTrue(importEvent is LibraryEvent.ImportProgress)
        val progress = (importEvent as LibraryEvent.ImportProgress).progress
        assertEquals(1L, progress.id)
        assertEquals(ImportState.ADDING, progress.state)
        assertEquals(3, progress.total)
        val views = LibraryEvent.fromJson(
            """{"type":"viewsInvalidated","views":[]}""",
        )
        assertTrue(views is LibraryEvent.ViewsInvalidated)
        assertNull(LibraryEvent.fromJson("not-json"))
    }

    /** D-116: what an Eagle import reports, and what the import screen reads first. */
    @Test
    fun anEagleImportReportAndSummaryParse() {
        val progress = ImportProgress.fromJson(
            """{"id":3,"kind":"eagle","state":"done","total":2,"processed":2,"added":1,"duplicates":0,
            "failed":1,"previewsTotal":1,"previewsDone":1,"report":{"imported":1,"duplicates":0,
            "trashSkipped":2,"skipped":[{"path":"/e/EBROKEN.info","reason":"the original file is missing"}],
            "lockedFolders":["Секрет 🔒"]}}""",
        )
        assertEquals(ImportKind.EAGLE, progress.kind)
        val report = progress.report!!
        assertEquals(1, report.imported)
        assertEquals(2, report.trashSkipped)
        assertEquals("the original file is missing", report.skipped.single().reason)
        assertEquals(listOf("Секрет 🔒"), report.lockedFolders)
        assertNull(ImportProgress.fromJson("""{"id":1,"kind":"files","state":"done"}""").report)

        val summary = EagleLibrarySummary.fromJson(
            """{"path":"/sdcard/Мой Eagle.library","name":"Мой Eagle","itemCount":12000,"bytes":5368709120}""",
        )
        assertEquals("Мой Eagle", summary.name)
        assertEquals(12000, summary.itemCount)
        assertEquals(5368709120L, summary.bytes)
    }

    @Test
    fun importProgressParsesDuplicateNotice() {
        val progress = ImportProgress.fromJson(
            """{"id":9,"kind":"files","state":"waitingForDecision","total":2,"processed":1,"added":0,"duplicates":0,"failed":0,"previewsTotal":0,"previewsDone":0,"duplicate":{"path":"/tmp/a.png","existing":"01ABC","inTrash":false,"targetFolder":"folder1"}}""",
        )
        assertEquals(ImportState.WAITING_FOR_DECISION, progress.state)
        assertEquals("/tmp/a.png", progress.duplicate?.path)
        assertEquals("folder1", progress.duplicate?.targetFolder)
        assertEquals(DuplicateAnswer.SKIP.wire, "skip")
        assertEquals(DuplicateAnswer.ADD_TO_FOLDER.wire, "addToFolder")
    }
}
