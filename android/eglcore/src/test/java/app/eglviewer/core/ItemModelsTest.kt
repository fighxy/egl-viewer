package app.eglviewer.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** JSON wire for A3 models (D-103 ≈ D-095) — host JVM, no `.so`. */
class ItemModelsTest {
    @Test
    fun itemEditEncodesLikeIos() {
        assertEquals(
            """{"type":"addTags","value":["Mood"]}""",
            ItemEdit.AddTags(listOf("Mood")).toJson().toString(),
        )
        assertEquals(
            """{"type":"setStar","value":3}""",
            ItemEdit.SetStar(3).toJson().toString(),
        )
        assertEquals("""{"type":"trash"}""", ItemEdit.Trash.toJson().toString())
        assertEquals("""{"type":"restore"}""", ItemEdit.Restore.toJson().toString())
        assertEquals(
            """{"type":"rename","value":"New"}""",
            ItemEdit.Rename("New").toJson().toString(),
        )
    }

    @Test
    fun itemParsesCamelCaseJson() {
        val item = Item.fromJson(
            """{"id":"a","name":"n","ext":"png","star":2,"tags":["T"],"folders":["f"],"annotation":"note","url":"https://x","size":10,"thumbVersion":1}""",
        )
        assertEquals("a", item.id)
        assertEquals("n", item.name)
        assertEquals(2, item.star)
        assertEquals(listOf("T"), item.tags)
        assertEquals(listOf("f"), item.folders)
        assertEquals("note", item.annotation)
        assertEquals("https://x", item.url)
    }

    @Test
    fun selectionTagsAndUndoStatusParse() {
        val tags = SelectionTags.fromJson(
            """{"items":2,"tags":[{"name":"Red","imageCount":1}],"size":100}""",
        )
        assertEquals(2, tags.items)
        assertEquals("Red", tags.tags[0].name)
        assertEquals(1, tags.tags[0].imageCount)
        assertNull(UndoStatus.fromJson("null"))
        assertNull(UndoStatus.fromJson(null))
        val status = UndoStatus.fromJson("""{"kind":"trash","items":3}""")
        assertEquals("trash", status!!.kind)
        assertEquals(3, status.items)
        assertTrue(TrashInfo.fromJson("""{"items":1,"bytes":50}""").bytes == 50)
    }

    @Test
    fun itemsTargetJson() {
        val json = ItemsTarget(viewId = 9, all = true, ids = listOf("x")).toJson().toString()
        assertTrue(json.contains("\"viewId\":9"))
        assertTrue(json.contains("\"all\":true"))
        assertTrue(json.contains("\"ids\":[\"x\"]"))
    }
}
