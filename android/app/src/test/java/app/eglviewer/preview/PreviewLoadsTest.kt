package app.eglviewer.preview

import app.eglviewer.browse.SharedDecodes
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)

/** Neighbour prefetch, shared decodes, and the page the bars follow (D-127, D-131). */
class PreviewLoadsTest {
    @Test
    fun neighboursAreThePagesOnBothSides() {
        assertEquals(listOf(1), neighborIndices(0, 5))
        assertEquals(listOf(3, 5), neighborIndices(4, 10))
        assertEquals(emptyList<Int>(), neighborIndices(0, 1))
        assertEquals(emptyList<Int>(), neighborIndices(0, 0))
    }

    @Test
    fun aPageThatScrollsAwayIsCancelledAndOneThatStaysIsKept() {
        val jobs = HashMap<Int, Job>()
        val loads = NeighborLoads()
        loads.update(5, 10, 1080) { page -> Job().also { jobs[page] = it } }
        assertEquals(setOf(4, 6), loads.inflight)

        val warmed = jobs.getValue(6)
        loads.update(6, 10, 1080) { page -> Job().also { jobs[page] = it } }

        assertFalse("landing on a prefetched page keeps its decode", warmed.isCancelled)
        assertTrue("the page that is no longer next is cancelled", jobs.getValue(4).isCancelled)
        assertEquals(setOf(5, 6, 7), loads.inflight)
    }

    @Test
    fun aFastSwipeCancelsTheOldNeighbours() {
        val cancelled = mutableListOf<Int>()
        val loads = NeighborLoads()
        loads.update(2, 20, 1080) { page ->
            Job().also { job -> job.invokeOnCompletion { cancelled += page } }
        }
        loads.update(9, 20, 1080) { Job() }
        assertEquals(listOf(1, 3), cancelled.sorted())
        assertEquals(setOf(8, 10), loads.inflight)
    }

    @Test
    fun aMissingCellIsTriedAgainWhenItArrives() {
        val loads = NeighborLoads()
        loads.update(4, 10, 1080) { null }
        assertTrue(loads.inflight.isEmpty())
        loads.update(4, 10, 1080) { Job() }
        assertEquals(setOf(3, 5), loads.inflight)
    }

    @Test
    fun aNewPixelSizeCancelsTheOldDecodes() {
        val old = mutableListOf<Job>()
        val loads = NeighborLoads()
        loads.update(4, 10, 1080) { Job().also { old += it } }
        loads.update(4, 10, 1440) { Job() }
        assertTrue(old.all { it.isCancelled })
        assertEquals(setOf(3, 5), loads.inflight)
    }

    @Test
    fun aFinishedPrefetchCanRunAgain() {
        val loads = NeighborLoads()
        val done = Job()
        loads.update(4, 10, 1080) { done }
        done.complete()
        var started = 0
        loads.update(4, 10, 1080) { started += 1; Job() }
        assertEquals(2, started)
    }

    @Test
    fun leavingThePreviewCancelsWhatIsStillLoading() {
        val loads = NeighborLoads()
        val job = Job()
        loads.update(1, 4, 1080) { job }
        loads.cancelAll()
        assertTrue(job.isCancelled)
        assertTrue(loads.inflight.isEmpty())
    }

    @Test
    fun twoAsksShareOneDecode() = runTest {
        val calls = AtomicInteger()
        val release = CompletableDeferred<Unit>()
        val loads = SharedDecodes<String>(this) {
            calls.incrementAndGet()
            release.await()
            "pic"
        }
        val one = async { loads.load("a") }
        val two = async { loads.load("a") }
        advanceUntilIdle()
        assertEquals(1, calls.get())
        assertEquals(1, loads.runningCount)
        release.complete(Unit)
        assertEquals("pic", one.await())
        assertEquals("pic", two.await())
        advanceUntilIdle()
        assertEquals(0, loads.runningCount)
    }

    @Test
    fun cancellingOneWaiterKeepsTheDecodeForTheOther() = runTest {
        val calls = AtomicInteger()
        val release = CompletableDeferred<Unit>()
        val loads = SharedDecodes<String>(this) {
            calls.incrementAndGet()
            release.await()
            "pic"
        }
        val one = launch { loads.load("a") }
        val two = async { loads.load("a") }
        advanceUntilIdle()
        one.cancel()
        advanceUntilIdle()
        assertEquals(1, loads.runningCount)
        assertEquals(1, calls.get())
        release.complete(Unit)
        assertEquals("pic", two.await())
    }

    @Test
    fun aCancelledAskLeavesNoDecodeAndCanBeAskedAgain() = runTest {
        val calls = AtomicInteger()
        val loads = SharedDecodes<String>(this) {
            calls.incrementAndGet()
            suspendCancellableCoroutine { }
        }
        val job = launch { loads.load("a") }
        advanceUntilIdle()
        assertEquals(1, loads.runningCount)
        job.cancel()
        advanceUntilIdle()
        assertEquals(0, loads.runningCount)
        assertTrue(job.isCancelled)

        val release = CompletableDeferred<Unit>()
        val again = SharedDecodes<String>(this) {
            calls.incrementAndGet()
            release.await()
            "again"
        }
        val second = async { again.load("a") }
        advanceUntilIdle()
        release.complete(Unit)
        assertEquals("again", second.await())
        assertEquals(2, calls.get())
    }

    @Test
    fun atMostTwoDecodesRunAtOnce() = runTest {
        val inside = AtomicInteger()
        val peak = AtomicInteger()
        val release = CompletableDeferred<Unit>()
        val loads = SharedDecodes<String>(this, limit = 2) { key ->
            val now = inside.incrementAndGet()
            peak.updateAndGet { maxOf(it, now) }
            try {
                release.await()
            } finally {
                inside.decrementAndGet()
            }
            key
        }
        val jobs = (1..4).map { index -> async { loads.load("k$index") } }
        advanceUntilIdle()
        assertEquals(2, inside.get())
        release.complete(Unit)
        assertEquals(setOf("k1", "k2", "k3", "k4"), jobs.map { it.await() }.toSet())
        assertTrue(peak.get() <= 2)
    }

    @Test
    fun thePreviewOpensOnTheTappedPhoto() {
        val (state, effect) = followPreview(PreviewFollowState(shown = 5), start = 5, page = 5, scrolling = false)
        assertEquals(5, state.shown)
        assertFalse(state.userMoved)
        assertTrue(effect is PreviewFollowEffect.None)
    }

    @Test
    fun aRestingFirstPageBeforeAnyScrollSnapsBackToTheTap() {
        val (state, effect) = followPreview(PreviewFollowState(shown = 5), start = 5, page = 0, scrolling = false)
        assertEquals(5, state.shown)
        assertFalse(state.userMoved)
        assertTrue(effect is PreviewFollowEffect.SnapToStart)
    }

    @Test
    fun aSwipeGlidesWithoutMovingTheBarsAndPublishesWhenItRests() {
        val started = followPreview(PreviewFollowState(shown = 5), start = 5, page = 5, scrolling = true)
        assertEquals(5, started.first.shown)
        assertTrue(started.first.userMoved)
        assertTrue(started.second is PreviewFollowEffect.None)

        val mid = followPreview(started.first, start = 5, page = 6, scrolling = true)
        assertEquals("the bars stay on the page the swipe left", 5, mid.first.shown)
        assertTrue(mid.second is PreviewFollowEffect.None)

        val rest = followPreview(mid.first, start = 5, page = 6, scrolling = false)
        assertEquals(6, rest.first.shown)
        assertTrue(rest.second is PreviewFollowEffect.Show)
        assertEquals(6, (rest.second as PreviewFollowEffect.Show).page)
    }
}
