package app.eglviewer.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** JSON wire for A5 organize models (D-105 ≈ D-097) — host JVM, no `.so`. */
class OrganizeModelsTest {
    @Test
    fun folderChangeEncodesLikeIos() {
        val create = FolderChange.Create(
            name = "Refs",
            parent = null,
            description = "d",
            iconColor = "pink",
        ).toJson().toString()
        assertTrue(create.contains("\"type\":\"create\""))
        assertTrue(create.contains("\"name\":\"Refs\""))
        assertTrue(create.contains("\"iconColor\":\"pink\""))

        val update = FolderChange.Update(
            id = "f1",
            name = "Posters",
            description = "",
            tags = listOf("a"),
            iconColor = null,
            orderBy = SortKey.Name,
            sortAscending = true,
        ).toJson().toString()
        assertTrue(update.contains("\"type\":\"update\""))
        assertTrue(update.contains("\"orderBy\":\"name\""))
        assertTrue(update.contains("\"sortAscending\":true"))

        val move = FolderChange.Move(id = "f1", parent = "p", index = 2).toJson().toString()
        assertTrue(move.contains("\"type\":\"move\""))
        assertTrue(move.contains("\"index\":2"))
    }

    @Test
    fun folderParsesExtraFields() {
        val folder = Folder.fromJson(
            org.json.JSONObject(
                """{"id":"1","name":"A","description":"note","iconColor":"blue","tags":["t"],"children":[],"orderBy":"name","sortAscending":false}""",
            ),
        )
        assertEquals("note", folder.description)
        assertEquals("blue", folder.iconColor)
        assertEquals(listOf("t"), folder.tags)
        assertEquals(SortKey.Name, folder.orderBy)
        assertEquals(false, folder.sortAscending)
    }

    @Test
    fun smartFolderKeepsConditions() {
        val folder = SmartFolder.fromJson(
            org.json.JSONObject(
                """{"id":"s","name":"Reds","children":[],"conditions":[{"match":"AND","boolean":"TRUE","rules":[]}],"unsupported":["palette"]}""",
            ),
        )
        assertEquals("Reds", folder.name)
        assertEquals(listOf("palette"), folder.unsupported)
        assertTrue(folder.conditions is JSONValue.Array)
    }

    @Test
    fun tagGroupAndSmartChangeRoundTrip() {
        val groups = TagGroup.listFromJson(
            """[{"id":"g","name":"Colours","color":"red","tags":["Red"]}]""",
        )
        assertEquals("Colours", groups[0].name)
        assertEquals(listOf("Red"), groups[0].tags)

        val create = TagGroupChange.Create(name = "Colours", color = "red", tags = listOf("Red"))
            .toJson().toString()
        assertTrue(create.contains("\"type\":\"create\""))

        val conditions = JSONValue.Array(
            listOf(
                JSONValue.Object(
                    mapOf(
                        "match" to JSONValue.Text("AND"),
                        "boolean" to JSONValue.Text("TRUE"),
                        "rules" to JSONValue.Array(emptyList()),
                    ),
                ),
            ),
        )
        val smart = SmartFolderChange.Create(name = "Reds", parent = null, conditions = conditions)
            .toJson().toString()
        assertTrue(smart.contains("\"type\":\"create\""))
        assertTrue(smart.contains("\"name\":\"Reds\""))

        val draft = SmartRulesDraft.fromJson(
            """{"parent":null,"conditions":[{"match":"AND","boolean":"TRUE","rules":[]}]}""",
        )
        assertNull(draft.parent)
        assertTrue(draft.conditions is JSONValue.Array)
    }

    @Test
    fun jsonValueKeepsUnknownKeysAndNumbers() {
        val stored = JSONValue.parse(
            """[{"match":"OR","boolean":"FALSE","eagleOnly":true,"rules":[{"property":"tags","method":"union","value":["Red"]},{"property":"palette","method":"near","value":3}]}]""",
        )
        assertTrue(stored is JSONValue.Array)
        val first = stored.array!![0].obj!!
        assertEquals(true, (first["eagleOnly"] as JSONValue.Bool).value)
        assertEquals(listOf(3.0), first["rules"]!!.array!![1].obj!!["value"]!!.numbers)
        assertEquals(listOf("Red"), first["rules"]!!.array!![0].obj!!["value"]!!.strings)
        assertNull(LabelColor.fromWire("teal"))
        assertEquals(LabelColor.PINK, LabelColor.fromWire("pink"))
    }
}
