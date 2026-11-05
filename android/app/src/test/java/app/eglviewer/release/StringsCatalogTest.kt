package app.eglviewer.release

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The three string catalogs hold the same keys, Russian plurals have all their forms, and no
 * screen writes an English label straight into the code (D-111).
 */
class StringsCatalogTest {
    private val res: File by lazy {
        var dir = File(System.getProperty("user.dir")!!).absoluteFile
        repeat(5) {
            val candidate = File(dir, "android/app/src/main/res")
            if (candidate.isDirectory) return@lazy candidate
            val local = File(dir, "src/main/res")
            if (local.isDirectory && File(dir, "build.gradle.kts").isFile) return@lazy local
            dir = dir.parentFile ?: dir
        }
        error("res folder not found from ${System.getProperty("user.dir")}")
    }

    private fun catalog(folder: String): Pair<Set<String>, Map<String, Set<String>>> {
        val text = File(res, "$folder/strings.xml").readText()
        val strings = Regex("""<string name="([a-z0-9_]+)"""").findAll(text).map { it.groupValues[1] }.toSet()
        val plurals = Regex("""<plurals name="([a-z0-9_]+)">(.*?)</plurals>""", RegexOption.DOT_MATCHES_ALL)
            .findAll(text)
            .associate { match ->
                match.groupValues[1] to Regex("""quantity="(\w+)"""").findAll(match.groupValues[2])
                    .map { it.groupValues[1] }
                    .toSet()
            }
        return strings to plurals
    }

    @Test
    fun theThreeCatalogsHoldTheSameKeys() {
        val (enStrings, enPlurals) = catalog("values")
        for (folder in listOf("values-ru", "values-zh-rCN")) {
            val (strings, plurals) = catalog(folder)
            assertEquals("strings of $folder", enStrings, strings)
            assertEquals("plurals of $folder", enPlurals.keys, plurals.keys)
        }
        val (_, ruPlurals) = catalog("values-ru")
        for ((name, forms) in ruPlurals) {
            assertTrue("$name has one, few, many and other: $forms", forms.containsAll(listOf("one", "few", "many", "other")))
        }
    }

    @Test
    fun screensTakeTheirLabelsFromTheCatalogs() {
        val source = File(res.parentFile, "java")
        val label = Regex("""(Text\("[A-Z][a-z]|contentDescription = "[A-Z]|show\("[A-Z])""")
        val found = source.walkTopDown()
            .filter { it.isFile && it.extension == "kt" }
            .flatMap { file ->
                file.readLines().mapIndexedNotNull { index, line ->
                    if (label.containsMatchIn(line)) "${file.name}:${index + 1}: ${line.trim()}" else null
                }
            }
            .toList()
        assertTrue("English labels in code:\n" + found.joinToString("\n"), found.isEmpty())
    }
}
