package app.eglviewer.a11y

import org.junit.Assert.assertEquals
import org.junit.Test

/** TalkBack labels for cells and ratings (A8 ≈ D-100). */
class AccessibilityLabelsTest {
    @Test
    fun starsReadAsWordsNotSymbols() {
        assertEquals("", AccessibilityLabels.stars(0, "%d star", "%d stars"))
        assertEquals("1 star", AccessibilityLabels.stars(1, "%d star", "%d stars"))
        assertEquals("3 stars", AccessibilityLabels.stars(3, "%d star", "%d stars"))
    }

    @Test
    fun cellsNameTheirRatingAndSayLoadingUntilTheyArrive() {
        assertEquals(
            "Loading",
            AccessibilityLabels.itemCell(null, 0, "Loading", ""),
        )
        assertEquals(
            "sample-0",
            AccessibilityLabels.itemCell("sample-0", 0, "Loading", ""),
        )
        assertEquals(
            "sample-0, 3 stars",
            AccessibilityLabels.itemCell("sample-0", 3, "Loading", "3 stars"),
        )
    }
}
