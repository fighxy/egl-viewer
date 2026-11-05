package app.eglviewer.release

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * What a release build carries (A8 ≈ D-100): launcher icon densities, Play Store icon,
 * privacy notes and the three-language privacy strings living next to the other catalogs.
 */
class ReleaseTest {
    private val repoRoot: File by lazy {
        var dir = File(System.getProperty("user.dir")!!)
        // Gradle sets user.dir to android/ or android/app depending on the task.
        repeat(4) {
            if (File(dir, "android/PRIVACY.md").isFile) return@lazy dir
            dir = dir.parentFile ?: dir
        }
        File(System.getProperty("user.dir")!!).let { cwd ->
            sequenceOf(cwd, cwd.parentFile, cwd.parentFile?.parentFile)
                .filterNotNull()
                .firstOrNull { File(it, "android/PRIVACY.md").isFile }
                ?: cwd
        }
    }

    @Test
    fun theAppHasAdaptiveLauncherIcons() {
        val res = File(repoRoot, "android/app/src/main/res")
        assertTrue(File(res, "mipmap-anydpi-v26/ic_launcher.xml").isFile)
        for (density in listOf("mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi")) {
            assertTrue(
                "mipmap-$density/ic_launcher.png",
                File(res, "mipmap-$density/ic_launcher.png").isFile,
            )
            assertTrue(
                "mipmap-$density/ic_launcher_round.png",
                File(res, "mipmap-$density/ic_launcher_round.png").isFile,
            )
            assertTrue(
                "mipmap-$density/ic_launcher_foreground.png",
                File(res, "mipmap-$density/ic_launcher_foreground.png").isFile,
            )
        }
        val play = File(repoRoot, "android/app/ic_launcher-playstore.png")
        assertTrue(play.isFile)
        assertEquals(512, pngSize(play).first)
        assertEquals(512, pngSize(play).second)
    }

    @Test
    fun privacyNotesDeclareNoTracking() {
        val privacy = File(repoRoot, "android/PRIVACY.md").readText()
        assertTrue(privacy.contains("No tracking"))
        assertTrue(privacy.contains("No analytics"))
        assertTrue(File(repoRoot, "android/app/src/main/res/xml/data_extraction_rules.xml").isFile)
        assertTrue(File(repoRoot, "android/app/src/main/res/xml/backup_rules.xml").isFile)
    }

    @Test
    fun privacyStringsExistInTheThreeLanguages() {
        val keys = listOf(
            "settings_privacy",
            "privacy_title",
            "privacy_intro",
            "privacy_data_heading",
            "privacy_data_body",
            "privacy_device_heading",
            "privacy_device_body",
            "privacy_network_heading",
            "privacy_network_body",
            "browse_loading_item",
        )
        val catalogs = listOf(
            "android/app/src/main/res/values/strings.xml",
            "android/app/src/main/res/values-ru/strings.xml",
            "android/app/src/main/res/values-zh-rCN/strings.xml",
        )
        for (catalog in catalogs) {
            val text = File(repoRoot, catalog).readText()
            for (key in keys) {
                assertTrue("$key in $catalog", text.contains("name=\"$key\""))
            }
            assertTrue("details_stars in $catalog", text.contains("name=\"details_stars\""))
        }
    }

    private fun pngSize(file: File): Pair<Int, Int> {
        val data = file.inputStream().use { it.readNBytes(24) }
        require(data.size >= 24 && data[0] == 0x89.toByte()) { "not a PNG: $file" }
        val width = ((data[16].toInt() and 0xff) shl 24) or
            ((data[17].toInt() and 0xff) shl 16) or
            ((data[18].toInt() and 0xff) shl 8) or
            (data[19].toInt() and 0xff)
        val height = ((data[20].toInt() and 0xff) shl 24) or
            ((data[21].toInt() and 0xff) shl 16) or
            ((data[22].toInt() and 0xff) shl 8) or
            (data[23].toInt() and 0xff)
        return width to height
    }
}
