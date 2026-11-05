package app.eglviewer.browse

import app.eglviewer.core.ItemEdit
import app.eglviewer.core.SortKey
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
import org.junit.Test

/**
 * BrowseModel against an in-memory [FakeViewSession] (no `.so`): windows, sort, search,
 * filters, Select All with trash refresh, and the saved view (D-102 ≈ D-094).
 */
@OptIn(ExperimentalCoroutinesApi::class)
class BrowseModelTest {
    private val mainDispatcher = UnconfinedTestDispatcher()

    @Before
    fun setMainDispatcher() {
        Dispatchers.setMain(mainDispatcher)
    }

    @After
    fun resetMainDispatcher() {
        Dispatchers.resetMain()
    }

    private fun model(open: BrowseHost, place: Place): BrowseModel =
        BrowseModel(
            open = open,
            place = place,
            scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined),
            compute = Dispatchers.Unconfined,
        )

    private class TestHost(
        override val session: ViewSession,
        override val libraryPath: String,
        saved: SavedViews,
    ) : BrowseHost {
        override val readOnly: Boolean = false
        override var savedView: SavedView = saved.load(forLibrary = libraryPath)
            private set
        override val thumbnails: ThumbnailCache by lazy { error("thumbnails unused in unit tests") }
        private val savedViews = saved

        override fun remember(view: SavedView) {
            savedView = view
            savedViews.save(view, forLibrary = libraryPath)
        }

        override fun forgetPlace() {
            remember(savedView.copy(place = null, filters = BrowseFilters(), search = ""))
        }

        override fun report(message: String) = Unit
        override fun register(browser: BrowseModel) = Unit
        override fun unregister(browser: BrowseModel) = Unit
    }

    private fun host(
        images: Int,
        saved: SavedViews = SavedViews.memory(),
        path: String = "/tmp/browse-test.eglib",
    ): Pair<BrowseHost, FakeViewSession> {
        val session = FakeViewSession.sample(images)
        val open = TestHost(session = session, libraryPath = path, saved = saved)
        val browsers = mutableListOf<BrowseModel>()
        val wired = object : BrowseHost by open {
            override fun register(browser: BrowseModel) {
                browsers += browser
            }

            override fun unregister(browser: BrowseModel) {
                browsers.remove(browser)
            }
        }
        session.onViewsChanged = { views ->
            browsers.forEach { it.invalidated(views) }
        }
        return wired to session
    }

    @Test
    fun cellsComeInWindowsAndFollowTheSortAndSearch() = runBlocking {
        val (open, _) = host(130)
        val model = model(open, Place.All)
        model.start()
        assertTrue(model.waitUntil { !model.loading && model.total == 130 })
        assertTrue(model.waitUntil { model.cell(at = 0) != null })
        assertNotNull(model.cell(at = BrowseModel.WINDOW - 1))
        assertNull("only the first window is loaded", model.cell(at = BrowseModel.WINDOW))
        model.need(125)
        assertTrue(model.waitUntil { model.cell(at = 129) != null })

        model.change { it.copy(sort = SortKey.Name, ascending = true) }
        assertTrue(model.waitUntil { model.cell(at = 0)?.name == "sample-0" })

        model.change { it.copy(search = "sample-12") }
        assertTrue(model.waitUntil { model.total == 11 })
        model.change { it.copy(search = "") }
        assertTrue(model.waitUntil { model.total == 130 })
        model.stop()
    }

    @Test
    fun filtersNarrowTheView() = runBlocking {
        val (open, _) = host(30)
        val model = model(open, Place.All)
        model.start()
        assertTrue(model.waitUntil { !model.loading && model.total == 30 })
        model.loadVocabulary()
        assertEquals(listOf(FakeViewSession.TAG), model.vocabulary.map { it.name })
        assertEquals(listOf("png"), model.extensions.map { it.ext })
        assertEquals(30, model.extensions.first().count)

        model.toggleTag(FakeViewSession.TAG)
        assertTrue(model.waitUntil { model.total == FakeViewSession.TAGGED })
        model.change { it.copy(filters = it.filters.copy(tagMode = app.eglviewer.core.TagMode.None)) }
        assertTrue(model.waitUntil { model.total == 30 - FakeViewSession.TAGGED })
        model.resetFilters()
        model.change { it.copy(filters = it.filters.copy(shape = BrowseFilters.Shape.Square)) }
        assertTrue(model.waitUntil { model.total == 10 })
        model.change { it.copy(filters = it.filters.copy(shape = null, rating = 0)) }
        assertTrue(model.waitUntil { model.total == 30 })
        model.change { it.copy(filters = it.filters.copy(rating = 5)) }
        assertTrue(model.waitUntil { model.total == 0 })
        assertTrue(model.settings.filters.isActive)
        model.stop()
    }

    @Test
    fun selectAllActsOnTheWholeViewAndRefreshes() = runBlocking {
        val (open, session) = host(12)
        val model = model(open, Place.All)
        model.start()
        assertTrue(model.waitUntil { !model.loading && model.total == 12 })
        assertTrue(model.waitUntil { model.cell(at = 11) != null })

        model.setSelecting(true)
        model.selectAll()
        assertEquals(12, model.selectedCount)
        val kept = model.cell(at = 0)!!
        model.toggle(kept.id)
        assertEquals(11, model.selectedCount)
        assertFalse(model.isSelected(kept.id))
        assertTrue(model.target.all)
        assertEquals(listOf(kept.id), model.target.ids)

        val trashed = session.editItems(model.target, ItemEdit.Trash)
        assertEquals(11, trashed)
        assertTrue(model.waitUntil { model.total == 1 })
        assertTrue(model.waitUntil { model.cell(at = 0)?.id == kept.id })

        val trash = model(open, Place.Trash)
        trash.start()
        assertTrue(trash.waitUntil { !trash.loading && trash.total == 11 })
        trash.stop()
        model.stop()
    }

    @Test
    fun theViewIsSavedPerLibraryAndFiltersPerPlace() = runBlocking {
        val store = SavedViews.memory()
        val path = "/tmp/saved-browse.eglib"
        val (open, _) = host(3, saved = store, path = path)
        val first = model(open, Place.Untagged)
        first.start()
        assertTrue(first.waitUntil { !first.loading })
        first.change {
            it.copy(
                sort = SortKey.Size,
                layout = BrowseLayout.List,
                filters = it.filters.copy(ext = "png"),
            )
        }
        first.stop()

        val saved = store.load(forLibrary = path)
        assertEquals(Place.Untagged, saved.place)
        assertEquals(SortKey.Size, saved.sort)

        val again = model(open, Place.Untagged)
        assertEquals("png", again.settings.filters.ext)
        val other = model(open, Place.All)
        assertEquals(SortKey.Size, other.settings.sort)
        assertEquals(BrowseLayout.List, other.settings.layout)
        assertNull("filters stay with their place", other.settings.filters.ext)

        open.forgetPlace()
        assertNull(store.load(forLibrary = path).place)
    }

    @Test
    fun selectionTargetNeverListsAllIds() = runBlocking {
        val (open, _) = host(50)
        val model = model(open, Place.All)
        model.start()
        assertTrue(model.waitUntil { !model.loading && model.total == 50 })
        model.selectAll()
        assertTrue(model.target.all)
        assertTrue(model.target.ids.isEmpty())
        assertEquals(50, model.selectedCount)
        model.stop()
    }
}
