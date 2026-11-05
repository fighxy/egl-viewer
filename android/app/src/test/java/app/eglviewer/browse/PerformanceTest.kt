package app.eglviewer.browse

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
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * A library of 10,000 items on the phone (A8 ≈ D-100): the view opens, windows anywhere in
 * it arrive, the sort and the search answer, and Select All selects the whole view, each
 * within a budget. The budgets are loose on purpose (CI hosts are shared); they catch work
 * that grows with the library on the main path, like a whole item-id list over JNI.
 * Timings go to the log as `perf:` lines.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class PerformanceTest {
    companion object {
        const val ITEMS = 10_000
    }

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

    private fun host(images: Int): BrowseHost {
        val session = FakeViewSession.sample(images)
        val open = TestHost(
            session = session,
            libraryPath = "/tmp/perf-$images.eglib",
            saved = SavedViews.memory(),
        )
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
        return wired
    }

    private fun perf(what: String, started: Long) {
        val spent = (System.nanoTime() - started) / 1_000_000_000.0
        println(String.format("perf: %s in %.3f s", what, spent))
    }

    @Test
    fun tenThousandItems() = runBlocking {
        val filled = System.nanoTime()
        val open = host(ITEMS)
        perf("$ITEMS cells built", filled)

        val model = model(open, Place.All)
        val opening = System.nanoTime()
        model.start()
        assertTrue(model.waitUntil(5_000) { !model.loading && model.total == ITEMS })
        assertTrue(model.waitUntil(3_000) { model.cell(at = 0) != null })
        perf("the view opens", opening)

        val end = System.nanoTime()
        model.need(ITEMS - 10)
        assertTrue(model.waitUntil(3_000) { model.cell(at = ITEMS - 1) != null })
        assertNull("the middle is not loaded", model.cell(at = ITEMS / 2))
        perf("a window at the end", end)

        val sorting = System.nanoTime()
        model.change { it.copy(sort = SortKey.Name, ascending = true) }
        assertTrue(model.waitUntil(5_000) { model.cell(at = 0)?.name == "sample-0" })
        perf("the name sort", sorting)

        val searching = System.nanoTime()
        model.change { it.copy(search = "sample-9999") }
        assertTrue(model.waitUntil(5_000) { model.total == 1 })
        perf("a search", searching)
        model.change { it.copy(search = "") }
        assertTrue(model.waitUntil(5_000) { model.total == ITEMS })

        val filtering = System.nanoTime()
        model.toggleTag(FakeViewSession.TAG)
        assertTrue(model.waitUntil(5_000) { model.total == FakeViewSession.TAGGED })
        perf("a tag filter", filtering)
        model.resetFilters()
        assertTrue(model.waitUntil(5_000) { model.total == ITEMS })

        // Select All stays a rule over the view: no list of 10,000 ids.
        model.setSelecting(true)
        model.selectAll()
        assertEquals(ITEMS, model.selectedCount)
        assertTrue(model.target.all)
        assertTrue(model.target.ids.isEmpty())

        model.stop()
    }
}
