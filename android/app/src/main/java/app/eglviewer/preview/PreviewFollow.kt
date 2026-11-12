package app.eglviewer.preview

/**
 * Which photo the bars and the neighbour loads follow (D-131).
 *
 * [page] is the pager's closest page. It changes in the middle of a swipe.
 * While [scrolling] is true that reading is the glide, so [PreviewFollowState.shown]
 * stays and nothing is written back into the pager — writing it back is what
 * made a swipe jump. Before the user has scrolled, a resting page other than
 * the tapped one is the layout opening at the first photo; the caller puts the
 * pager back on the start and does not follow it.
 */
internal data class PreviewFollowState(val shown: Int, val userMoved: Boolean = false)

internal sealed interface PreviewFollowEffect {
    data object None : PreviewFollowEffect
    data class Show(val page: Int) : PreviewFollowEffect
    data object SnapToStart : PreviewFollowEffect
}

internal fun followPreview(
    state: PreviewFollowState,
    start: Int,
    page: Int,
    scrolling: Boolean,
): Pair<PreviewFollowState, PreviewFollowEffect> {
    if (page < 0) return state to PreviewFollowEffect.None
    if (scrolling) {
        if (state.userMoved) return state to PreviewFollowEffect.None
        return state.copy(userMoved = true) to PreviewFollowEffect.None
    }
    if (!state.userMoved && page != start) {
        return state to PreviewFollowEffect.SnapToStart
    }
    if (page != state.shown) {
        return state.copy(shown = page) to PreviewFollowEffect.Show(page)
    }
    return state to PreviewFollowEffect.None
}
