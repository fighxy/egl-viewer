package app.eglviewer.core

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Host JVM tests of settings / export JSON (Android A7 ≈ D-099 / D-107). */
class SettingsModelsTest {
    @Test
    fun libraryStatsAndBackupParse() {
        val stats = LibraryStats.fromJson(
            """{"originals":10,"thumbnails":2,"database":3,"backups":4}""",
        )
        assertEquals(10L, stats.originals)
        assertEquals(19L, stats.total)

        val backups = Backup.listFromJson(
            """[{"name":"library-2026-11-05.db","kind":"manual","createdAt":1730764800000,"size":1024}]""",
        )
        assertEquals(1, backups.size)
        assertEquals(BackupKind.MANUAL, backups[0].kind)
        assertEquals("library-2026-11-05.db", backups[0].name)
    }

    @Test
    fun checkAndRepairRoundTrip() {
        val check = LibraryCheck.fromJson(
            """{"items":2,"databaseOk":true,"databaseProblems":[],"missingCount":0,"missing":[],"orphanCount":0,"orphanBytes":0,"orphans":[],"orphanThumbnails":0}""",
        )
        assertTrue(check.isClean)
        assertEquals(2, check.items)

        val dirty = LibraryCheck.fromJson(
            """{"items":1,"databaseOk":false,"databaseProblems":["ok"],"missingCount":1,"missing":[{"id":"a","name":"","inTrash":false}],"orphanCount":1,"orphanBytes":9,"orphans":["x"],"orphanThumbnails":2}""",
        )
        assertFalse(dirty.isClean)
        assertEquals(1, dirty.missingCount)

        val options = RepairOptions(orphans = true, missing = false).toJson()
        assertTrue(options.getBoolean("orphans"))
        assertFalse(options.getBoolean("missing"))
    }

    @Test
    fun exportRequestAndPreviewEncode() {
        val selection = ExportSelection(whole = true)
        assertFalse(selection.isEmpty)
        assertEquals(true, selection.toJson().getBoolean("whole"))

        val request = ExportRequest(
            selection = selection,
            includeSubfolders = true,
            format = ExportFormat.Zip(ExportCompression.FAST),
            destination = "/tmp/out",
            encrypt = "pässphrase",
        )
        val json = request.toJson()
        assertEquals("/tmp/out", json.getString("destination"))
        assertEquals("zip", json.getJSONObject("format").getString("kind"))
        assertEquals("fast", json.getJSONObject("format").getString("compression"))
        assertEquals("pässphrase", json.getString("encrypt"))

        val mirror = ExportFormat.Mirror.toJson()
        assertEquals("mirror", mirror.getString("kind"))
        assertEquals("age", json.getJSONObject("format").getString("encryption"))

        // D-112 / D-113: a ZIP with a password and a 7z, as the core reads them.
        val aes = ExportFormat.Zip(ExportCompression.NONE, ZipEncryption.AES).toJson()
        assertEquals("aes", aes.getString("encryption"))
        val seven = ExportFormat.SevenZip(ExportCompression.SMALLEST).toJson()
        assertEquals("sevenZip", seven.getString("kind"))
        assertEquals("smallest", seven.getString("compression"))
        assertFalse(seven.has("encryption"))
        assertEquals(true, ExportFormat.SevenZip(ExportCompression.FAST).isArchive)
        assertEquals(false, ExportFormat.Mirror.isArchive)

        val preview = ExportPreview.fromJson("""{"items":2,"bytes":100,"hidden":1}""")
        assertEquals(2, preview.items)
        assertEquals(1, preview.hidden)

        val ticket = PreviewsTicket.fromJson("""{"id":null,"waiting":3}""")
        assertNull(ticket.id)
        assertEquals(3, ticket.waiting)
    }

    @Test
    fun exportProgressAndEventParse() {
        val progress = ExportProgress.fromJson(
            """{"id":7,"state":"done","total":2,"done":2,"bytesTotal":10,"bytesDone":10,"summary":{"items":2,"files":2,"bytes":10,"missing":[],"path":"/tmp/a.zip"}}""",
        )
        assertTrue(progress.isFinished)
        assertEquals(ExportState.DONE, progress.state)
        assertEquals("/tmp/a.zip", progress.summary?.path)

        val event = LibraryEvent.fromJson(
            """{"type":"exportProgress","progress":{"id":7,"state":"copying","total":2,"done":1,"bytesTotal":10,"bytesDone":5}}""",
        )
        assertTrue(event is LibraryEvent.ExportProgress)
        assertEquals(7L, (event as LibraryEvent.ExportProgress).progress.id)
        assertEquals(ExportState.COPYING, event.progress.state)

        // encrypt omitted as null
        val plain = ExportRequest(
            selection = ExportSelection(folders = listOf("f1")),
            includeSubfolders = false,
            format = ExportFormat.Mirror,
            destination = "/tmp",
        ).toJson()
        assertTrue(plain.isNull("encrypt"))
        assertEquals(JSONObject.NULL, plain.get("encrypt"))
    }

    /** D-110 / D-111: what Decrypt Archive returns, and the passphrase floor. */
    @Test
    fun aDecryptedArchiveParsesAndShortPassphrasesAreCaught() {
        val decrypted = DecryptedArchive.fromJson("""{"path":"/tmp/Фото 🎨.zip","bytes":2048}""")
        assertEquals("/tmp/Фото 🎨.zip", decrypted.path)
        assertEquals(2048L, decrypted.bytes)
        assertEquals("Фото 🎨.zip", decrypted.file.name)
        assertEquals(8, MIN_PASSPHRASE_CHARS)
        assertEquals(false, passphraseLongEnough("🔥🔥🔥🔥🔥🔥🔥"))
        assertEquals(true, passphraseLongEnough("пароль12"))
    }
}
