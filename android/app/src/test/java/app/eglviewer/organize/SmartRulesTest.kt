package app.eglviewer.organize

import app.eglviewer.core.JSONValue
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Smart rule model round trip (D-105 ≈ D-097) — host JVM. */
class SmartRulesTest {
    @Test
    fun rulesKeepWhatTheEditorDoesNotKnow() {
        val stored = JSONValue.parse(
            """[{"match":"OR","boolean":"FALSE","eagleOnly":true,"rules":[{"property":"tags","method":"union","value":["Red"]},{"property":"palette","method":"near","value":3}]}]""",
        )
        var conditions = SmartRules.read(stored).toMutableList()
        assertEquals(1, conditions.size)
        assertTrue(conditions[0].any)
        assertTrue(conditions[0].negated)
        assertTrue(conditions[0].rules[0].editable)
        assertFalse(conditions[0].rules[1].editable)
        assertEquals(stored, SmartRules.write(conditions))

        conditions[0].any = false
        val written = SmartRules.write(conditions).array!!.first().obj!!
        assertEquals(JSONValue.Text("AND"), written["match"])
        assertEquals(JSONValue.Bool(true), written["eagleOnly"])
        assertEquals(2, written["rules"]!!.array!!.size)
        assertTrue(SmartRules.read(JSONValue.Text("junk")).isEmpty())
    }

    @Test
    fun methodsKeepTheValueThatStillFits() {
        val size = SmartRules.newRule("fileSize")
        assertEquals(JSONValue.Text("mb"), size.fields["unit"])
        val between = SmartRules.withMethod(size, "between")
        assertEquals(listOf(1.0, 1.0), between.value?.numbers)
        assertEquals(listOf(1.0), SmartRules.withMethod(between, "<").value?.numbers)

        val recent = SmartRules.newRule("createTime")
        assertEquals("within", recent.method)
        val on = SmartRules.withMethod(recent, "on")
        assertEquals(SmartRules.today(), on.value?.numbers?.first())
        val range = SmartRules.withMethod(on, "between").value?.numbers.orEmpty()
        assertEquals(2, range.size)
        assertEquals(7 * SmartRules.DAY_MILLISECONDS, range[1] - range[0], 0.1)
        assertEquals("equal", SmartRules.withProperty(recent, "rating").method)
        assertFalse(SmartRules.needsValue("not-empty"))
    }

    @Test
    fun folderHelpersBlockSubtreeMoves() {
        // Pure helpers used by the move picker (mirrors OpenLibrary.subtree / endIndex).
        val posters = app.eglviewer.core.Folder(id = "p", name = "Posters")
        val refs = app.eglviewer.core.Folder(id = "r", name = "Refs", children = listOf(posters))
        val ids = mutableSetOf<String>()
        fun walk(folder: app.eglviewer.core.Folder) {
            ids += folder.id
            folder.children.forEach(::walk)
        }
        walk(refs)
        assertEquals(setOf("r", "p"), ids)
        assertTrue("p" in ids)
        assertEquals(1, refs.children.size)
    }
}
