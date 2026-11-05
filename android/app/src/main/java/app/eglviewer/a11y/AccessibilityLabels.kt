package app.eglviewer.a11y

/**
 * TalkBack labels for grid / list cells and star ratings (A8 ≈ D-100). Kept free of Compose
 * so JVM unit tests can cover them without a device.
 */
object AccessibilityLabels {
    /** "3 stars" style label for a rating; empty when unrated. */
    fun stars(stars: Int, one: String, many: String): String {
        if (stars <= 0) return ""
        return if (stars == 1) one.format(stars) else many.format(stars)
    }

    /**
     * Cell description: the item name, plus its rating when set; [loading] while the window
     * has not arrived yet.
     */
    fun itemCell(
        name: String?,
        star: Int,
        loading: String,
        starsLabel: String,
    ): String {
        if (name == null) return loading
        return if (starsLabel.isNotEmpty()) "$name, $starsLabel" else name
    }
}
