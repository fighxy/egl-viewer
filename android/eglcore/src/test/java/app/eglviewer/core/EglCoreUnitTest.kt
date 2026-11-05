package app.eglviewer.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Host JVM tests — no device, no `.so` (D-101 / A1). */
class EglCoreUnitTest {
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
            """{"type":"importProgress","progress":{"id":1}}""",
        )
        assertTrue(importEvent is LibraryEvent.ImportProgress)
        val views = LibraryEvent.fromJson(
            """{"type":"viewsInvalidated","views":[]}""",
        )
        assertTrue(views is LibraryEvent.ViewsInvalidated)
        assertNull(LibraryEvent.fromJson("not-json"))
    }
}
