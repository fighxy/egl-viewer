package app.eglviewer.browse

import app.eglviewer.core.ItemEdit
import app.eglviewer.core.ItemsTarget
import app.eglviewer.details.ShareFiles
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/**
 * Details edits, selection actions with undo, and share naming (A3 ≈ D-095) against
 * [FakeViewSession] — no `.so`.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class DetailsAndActionsTest {
    private val mainDispatcher = UnconfinedTestDispatcher()

    @get:Rule
    val tmp = TemporaryFolder()

    @Before
    fun setMainDispatcher() {
        Dispatchers.setMain(mainDispatcher)
    }

    @After
    fun resetMainDispatcher() {
        Dispatchers.resetMain()
    }

    private class TestHost(
        override val session: FakeViewSession,
        override val libraryPath: String = "/tmp/a3-test.eglib",
    ) : BrowseHost {
        override val readOnly: Boolean = false
        override var savedView: SavedView = SavedView()
            private set
        override val thumbnails: ThumbnailCache by lazy { error("unused") }
        override var toast: ActionToast? = null
            private set

        override fun remember(view: SavedView) {
            savedView = view
        }

        override fun forgetPlace() {
            remember(savedView.copy(place = null, filters = BrowseFilters(), search = ""))
        }

        override fun report(message: String) = Unit
        override fun register(browser: BrowseModel) = Unit
        override fun unregister(browser: BrowseModel) = Unit

        override fun show(message: String, undoable: Boolean) {
            toast = ActionToast(message, undoable)
        }

        override suspend fun undo() {
            val status = session.undo()
            toast = if (status != null) ActionToast("Undone", undoable = false) else null
        }
    }

    private fun model(open: BrowseHost, place: Place = Place.All): BrowseModel =
        BrowseModel(
            open = open,
            place = place,
            scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined),
            compute = Dispatchers.Unconfined,
        )

    private fun host(images: Int): Pair<TestHost, FakeViewSession> {
        val session = FakeViewSession.sample(images)
        val open = TestHost(session)
        session.onViewsChanged = { views ->
            // BrowseModel listens via register; wire manually for actions.
        }
        return open to session
    }

    @Test
    fun detailsEditTheItemAndUndoTakesEachEditBack() = runBlocking {
        val (open, session) = host(3)
        val browsers = mutableListOf<BrowseModel>()
        val wired = object : BrowseHost by open {
            override fun register(browser: BrowseModel) {
                browsers += browser
            }
            override fun unregister(browser: BrowseModel) {
                browsers.remove(browser)
            }
        }
        session.onViewsChanged = { views -> browsers.forEach { it.invalidated(views) } }

        val details = app.eglviewer.details.ItemDetailsModel(wired, "id-0")
        details.load()
        assertEquals("sample-0", details.item.value?.name)

        details.rename("Renamed")
        assertEquals("Renamed", details.item.value?.name)
        assertEquals("Renamed", session.item("id-0").name)

        details.rate(3)
        assertEquals(3, details.item.value?.star)
        details.rate(3)
        assertEquals(0, details.item.value?.star)

        details.addTag("Mood")
        assertTrue(details.item.value?.tags?.contains("Mood") == true)
        details.addTag("mood")
        // sample-0 already carries FakeViewSession.TAG ("Red"); tags stay case-sensitive.
        assertTrue(details.item.value?.tags?.containsAll(listOf("Red", "Mood", "mood")) == true)

        details.setNotes("hello")
        details.setLink("https://example.com")
        assertEquals("hello", details.item.value?.annotation)
        assertEquals("https://example.com", details.item.value?.url)

        open.undo()
        assertEquals("", session.item("id-0").url)
        assertEquals("Undone", open.toast?.message)
        assertFalse(open.toast?.undoable == true)
    }

    @Test
    fun trashRestoreAndDeleteForGoodFromASelection() = runBlocking {
        val (open, session) = host(4)
        val browsers = mutableListOf<BrowseModel>()
        val wired = object : BrowseHost by open {
            override fun register(browser: BrowseModel) {
                browsers += browser
            }
            override fun unregister(browser: BrowseModel) {
                browsers.remove(browser)
            }
            override fun show(message: String, undoable: Boolean) = open.show(message, undoable)
            override suspend fun undo() = open.undo()
            override val toast: ActionToast? get() = open.toast
        }
        session.onViewsChanged = { views -> browsers.forEach { it.invalidated(views) } }

        val model = model(wired, Place.All)
        model.start()
        assertTrue(model.waitUntil { !model.loading && model.total == 4 })

        model.setSelecting(true)
        model.toggle("id-0")
        model.toggle("id-1")
        model.trash(model.target)
        assertTrue(model.waitUntil { model.total == 2 })
        assertTrue(open.toast?.undoable == true)
        assertFalse(model.selecting)

        open.undo()
        assertTrue(model.waitUntil { model.total == 4 })

        model.selectAll()
        model.rate(model.target, stars = 2)
        assertEquals(2, session.item("id-0").star)

        model.setSelecting(true)
        model.selectAll()
        model.trash(model.target)
        assertTrue(model.waitUntil { model.total == 0 })

        val trash = model(wired, Place.Trash)
        trash.start()
        assertTrue(trash.waitUntil { !trash.loading && trash.total == 4 })
        trash.setSelecting(true)
        trash.toggle("id-0")
        trash.restore(trash.target)
        assertTrue(trash.waitUntil { trash.total == 3 })

        trash.selectAll()
        trash.deleteForGood(trash.target)
        assertTrue(trash.waitUntil { trash.total == 0 })
        assertFalse(open.toast?.undoable == true)

        trash.stop()
        model.stop()
    }

    @Test
    fun sharingNamesTheFilesAfterTheItems() = runBlocking {
        val (open, session) = host(3)
        session.originalPayload = byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47)
        val model = model(open)
        model.start()
        assertTrue(model.waitUntil { !model.loading && model.total == 3 })
        model.selectAll()
        model.toggle("id-2")
        val ids = model.ids(of = model.target, limit = ShareFiles.LIMIT)
        assertEquals(2, ids.size)
        assertTrue(ids.containsAll(listOf("id-0", "id-1")))
        assertFalse(ids.contains("id-2"))

        val files = ShareFiles.prepare(listOf("id-0", "id-1", "id-2"), session, tmp.root)
        assertEquals(
            listOf("sample-0.png", "sample-1.png", "sample-2.png"),
            files.files.map { it.name },
        )
        assertTrue(files.files[0].readBytes().contentEquals(session.originalPayload))
        files.clean()
        assertFalse(files.folder.exists())
        model.stop()
    }

    @Test
    fun fileNamesAndTagSuggestions() {
        val item = FakeViewSession.itemFrom(
            app.eglviewer.core.ItemCell(id = "x", name = "a/b:c", ext = "png"),
        )
        assertEquals("a-b-c.png", ShareFiles.fileName(item))
        val used = mutableSetOf<String>()
        assertEquals("photo.png", ShareFiles.unique("photo.png", used))
        assertEquals("photo (2).png", ShareFiles.unique("photo.png", used))
        assertEquals(
            listOf("Mood", "Moody"),
            TagSuggestions.matching("mo", listOf("Mood", "Moody", "Red"), emptySet(), 6),
        )
        assertEquals(
            emptyList<String>(),
            TagSuggestions.matching("mo", listOf("Mood"), setOf("Mood"), 6),
        )
    }

    @Test
    fun previewOpensFromTap() = runBlocking {
        val (open, _) = host(2)
        val model = model(open)
        model.start()
        assertTrue(model.waitUntil { model.cell(at = 0) != null })
        val cell = model.cell(at = 0)!!
        val tap = model.tap(cell, 0)
        assertTrue(tap is BrowseTap.OpenPreview)
        assertEquals(0, model.previewing)
        model.closePreview()
        assertNull(model.previewing)
        model.stop()
    }
}
