package app.eglviewer.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Host JVM tests of lock JSON (Android A6 ≈ D-098 / D-106). */
class LockModelsTest {
    @Test
    fun recoveryKeyParses() {
        val key = RecoveryKey.fromJson(
            """{"key":"01020304","phrase":"alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima"}""",
        )
        assertEquals("01020304", key.key)
        assertEquals(12, key.phrase.split(" ").size)
    }

    @Test
    fun folderLockFieldsParse() {
        val folder = Folder.fromJson(
            org.json.JSONObject(
                """{"id":"f1","name":"Private","isLocked":true,"isUnlocked":false,"hasPassword":true,"lockLabel":"Mine","lockColor":"blue","children":[]}""",
            ),
        )
        assertTrue(folder.isLocked)
        assertFalse(folder.isUnlocked)
        assertTrue(folder.hasPassword)
        assertEquals("Mine", folder.lockLabel)
        assertEquals("blue", folder.lockColor)
    }
}
