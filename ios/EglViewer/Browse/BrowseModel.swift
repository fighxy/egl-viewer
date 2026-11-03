import EglCore
import Foundation
import Observation
import SwiftUI

/// One place on screen: a view of the core opened for the place, its sort, filters and
/// search, the cells loaded so far in windows, and the selection (D-094). The whole item list
/// never crosses the C ABI: cells come in windows of `window` around what the user sees.
@MainActor
@Observable
final class BrowseModel {
    /// Cells per request; the core serves at most 200 (library-core `MAX_WINDOW`).
    static let window = 100
    /// Windows kept around the last cell the user saw; the others are dropped on a refresh.
    static let keptWindows = 3

    let open: OpenLibrary
    let place: Place

    private(set) var settings: SavedView
    private(set) var viewId: UInt64?
    private(set) var total = 0
    /// True until the first answer of the core for the current query.
    private(set) var loading = true

    // Selection: listed ids, or with `allSelected` every item of the view except them.
    private(set) var selecting = false
    private(set) var allSelected = false
    private(set) var picked: Set<String> = []

    /// The tags and file types of the library, for the filter sheet.
    private(set) var vocabulary: [Tag] = []
    private(set) var extensions: [ExtensionCount] = []

    private var windows: [Int: [ItemCell]] = [:]
    @ObservationIgnored private var requested: Set<Int> = []
    /// Bumped by each new query: older answers are dropped.
    @ObservationIgnored private var viewGeneration = 0
    /// Bumped by each refresh of the cells: older windows are dropped.
    @ObservationIgnored private var cellGeneration = 0
    @ObservationIgnored private var lastSeen = 0
    /// Between `start` and `stop`: only then does the model hold a view of the core.
    @ObservationIgnored private var active = false
    @ObservationIgnored private var pendingSearch: Task<Void, Never>?
    /// The filter sheet is open: refreshes read the tags again.
    @ObservationIgnored var wantsVocabulary = false

    init(open: OpenLibrary, place: Place) {
        self.open = open
        self.place = place
        var settings = open.savedView
        // Filters and search belong to the place they were set in.
        if settings.place != place {
            settings.filters = BrowseFilters()
            settings.search = ""
        }
        settings.place = place
        self.settings = settings
    }

    // MARK: The query

    var query: LibraryQuery {
        let filters = settings.filters
        let text = settings.search.trimmingCharacters(in: .whitespacesAndNewlines)
        return LibraryQuery(
            scope: place.scope,
            tags: filters.tags.isEmpty ? nil : filters.tags,
            tagMode: filters.tags.isEmpty ? nil : filters.tagMode,
            ext: filters.ext,
            rating: filters.rating,
            shape: filters.shape?.rawValue,
            text: text.isEmpty ? nil : text,
            sort: settings.sort,
            ascending: settings.ascending
        )
    }

    /// Opens the view when the screen appears; the library tells the model about refreshes.
    func start() async {
        active = true
        open.register(self)
        open.remember(settings)
        if viewId == nil {
            await reload()
        }
    }

    /// Closes the view when the screen goes away.
    func stop() {
        active = false
        open.unregister(self)
        pendingSearch?.cancel()
        viewGeneration += 1
        cellGeneration += 1
        if let viewId {
            let library = open.library
            Task.detached { library.closeView(viewId) }
        }
        viewId = nil
        windows = [:]
        requested = []
    }

    /// Changes the sort, look, filters or search; a change of the query opens a new view.
    func change(_ edit: (inout SavedView) -> Void) {
        let before = query
        var next = settings
        edit(&next)
        guard next != settings else { return }
        var withoutSearch = next
        withoutSearch.search = settings.search
        let searchOnly = withoutSearch == settings
        settings = next
        open.remember(settings)
        guard query != before, active else { return }
        pendingSearch?.cancel()
        if searchOnly {
            // Typing: wait for a pause before asking the core.
            pendingSearch = Task {
                try? await Task.sleep(for: .milliseconds(250))
                guard !Task.isCancelled else { return }
                await reload()
            }
        } else {
            Task { await reload() }
        }
    }

    /// A binding for SwiftUI controls that goes through `change`.
    func binding<T>(_ path: WritableKeyPath<SavedView, T>) -> Binding<T> {
        Binding(get: { self.settings[keyPath: path] }, set: { value in self.change { $0[keyPath: path] = value } })
    }

    func resetFilters() {
        change { $0.filters = BrowseFilters() }
    }

    /// Asks the core for the view of the current query; earlier answers still on their way
    /// are dropped.
    func reload() async {
        viewGeneration += 1
        let current = viewGeneration
        let library = open.library
        let query = query
        let old = viewId
        do {
            let opened = try await Task.detached {
                if let old { library.closeView(old) }
                return try library.openView(query)
            }.value
            guard current == viewGeneration, active else {
                Task.detached { library.closeView(opened.viewId) }
                return
            }
            cellGeneration += 1
            viewId = opened.viewId
            total = opened.total
            windows = [:]
            requested = []
            allSelected = false
            picked = []
            loading = false
            load(window: 0)
        } catch {
            guard current == viewGeneration else { return }
            viewId = nil
            total = 0
            windows = [:]
            loading = false
            open.report(error)
        }
    }

    // MARK: Cells

    /// The cell at `index`, if its window has arrived.
    func cell(at index: Int) -> ItemCell? {
        let window = windows[index / Self.window]
        let offset = index % Self.window
        guard let window, offset < window.count else { return nil }
        return window[offset]
    }

    /// The user sees `index`: loads its window and the next one.
    func need(_ index: Int) {
        lastSeen = index
        let window = index / Self.window
        load(window: window)
        if (index % Self.window) > Self.window / 2 {
            load(window: window + 1)
        }
    }

    /// The core recomputed views: the open one gets its new total and fresh cells near what
    /// the user sees; windows further away are dropped and come back when needed.
    func invalidated(_ views: [ViewOpened]) {
        guard let viewId, let match = views.first(where: { $0.viewId == viewId }) else { return }
        cellGeneration += 1
        total = match.total
        let seen = lastSeen / Self.window
        let keep = (seen - Self.keptWindows)...(seen + Self.keptWindows)
        windows = windows.filter { keep.contains($0.key) && $0.key * Self.window < total }
        requested = []
        for window in windows.keys.sorted() {
            load(window: window, force: true)
        }
        if windows.isEmpty {
            load(window: seen)
        }
        if wantsVocabulary {
            Task { await loadVocabulary() }
        }
    }

    private func load(window: Int, force: Bool = false) {
        guard let viewId, window >= 0, window * Self.window < total else { return }
        guard force || (!requested.contains(window) && windows[window] == nil) else { return }
        requested.insert(window)
        let current = cellGeneration
        let library = open.library
        let start = window * Self.window
        let count = Self.window
        Task {
            let cells = try? await Task.detached {
                try library.range(of: viewId, start: start, count: count)
            }.value
            guard current == cellGeneration else { return }
            requested.remove(window)
            if let cells {
                windows[window] = cells
            }
        }
    }

    // MARK: Selection

    var selectedCount: Int { allSelected ? max(total - picked.count, 0) : picked.count }

    func isSelected(_ id: String) -> Bool { allSelected != picked.contains(id) }

    /// What the selection's actions act on.
    var target: ItemsTarget { ItemsTarget(viewId: viewId, all: allSelected, ids: Array(picked).sorted()) }

    func setSelecting(_ on: Bool) {
        selecting = on
        if !on {
            allSelected = false
            picked = []
        }
    }

    func toggle(_ id: String) {
        if picked.contains(id) {
            picked.remove(id)
        } else {
            picked.insert(id)
        }
        if allSelected, picked.count >= total {
            allSelected = false
            picked = []
        }
    }

    func selectAll() {
        allSelected = true
        picked = []
    }

    func deselectAll() {
        allSelected = false
        picked = []
    }

    /// A tap: toggles in selection mode. Opening the preview arrives with the next PR.
    func tap(_ cell: ItemCell) {
        if selecting {
            toggle(cell.id)
        }
    }

    /// A long press starts selecting with this item.
    func longPress(_ cell: ItemCell) {
        if !selecting {
            selecting = true
        }
        if !isSelected(cell.id) {
            toggle(cell.id)
        }
    }

    // MARK: The filter sheet

    func loadVocabulary() async {
        let library = open.library
        if let tags = try? await Task.detached(operation: { try library.tags() }).value {
            vocabulary = tags.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        }
        if let list = try? await Task.detached(operation: { try library.extensions() }).value {
            extensions = list
        }
    }

    func toggleTag(_ name: String) {
        change { view in
            if let index = view.filters.tags.firstIndex(of: name) {
                view.filters.tags.remove(at: index)
            } else {
                view.filters.tags.append(name)
            }
        }
    }
}
