@testable import EglViewer
import EglCore
import XCTest

/// The browse model against the real core: windows of a view, sort, search, filters,
/// selection with Select All, refreshes after edits, and the saved view.
@MainActor
final class BrowseModelTests: XCTestCase {
    private func sample(images: Int, defaults: UserDefaults? = nil) async throws -> OpenLibrary {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("browse-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = try Library.create(in: dir, name: "Browse", appVersion: "0.0.0-test")
        let library = try Library.open(at: path, appVersion: "0.0.0-test")
        try await SampleLibrary.fill(library, images: images)
        let store = SavedViews(defaults: defaults ?? UserDefaults(suiteName: "browse-\(UUID().uuidString)")!)
        return try await OpenLibrary(library: library, url: path, scoped: false, savedViews: store)
    }

    private func waitUntil(
        _ what: String, timeout: TimeInterval = 15, _ condition: () -> Bool
    ) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while !condition() {
            guard Date() < deadline else {
                XCTFail("timed out waiting for \(what)")
                return
            }
            try await Task.sleep(for: .milliseconds(20))
        }
    }

    func testCellsComeInWindowsAndFollowTheSortAndSearch() async throws {
        let open = try await sample(images: 130)
        let model = BrowseModel(open: open, place: .all)
        await model.start()
        XCTAssertEqual(model.total, 130)
        try await waitUntil("the first window") { model.cell(at: 0) != nil }
        XCTAssertNotNil(model.cell(at: BrowseModel.window - 1))
        XCTAssertNil(model.cell(at: BrowseModel.window), "only the first window is loaded")
        model.need(125)
        try await waitUntil("the second window") { model.cell(at: 129) != nil }

        model.change { $0.sort = .name; $0.ascending = true }
        try await waitUntil("the name sort") { model.cell(at: 0)?.name == "sample-0" }

        model.change { $0.search = "sample-12" }
        // sample-12 and sample-120 … sample-129.
        try await waitUntil("the search") { model.total == 11 }
        model.change { $0.search = "" }
        try await waitUntil("the search cleared") { model.total == 130 }
        model.stop()
        await open.close()
    }

    func testFiltersNarrowTheView() async throws {
        let open = try await sample(images: 30)
        let model = BrowseModel(open: open, place: .all)
        await model.start()
        await model.loadVocabulary()
        XCTAssertEqual(model.vocabulary.map(\.name), [SampleLibrary.tag])
        XCTAssertEqual(model.extensions, [ExtensionCount(ext: "png", count: 30)])

        model.toggleTag(SampleLibrary.tag)
        try await waitUntil("the tag filter") { model.total == SampleLibrary.tagged }
        model.change { $0.filters.tagMode = .none }
        try await waitUntil("the none mode") { model.total == 30 - SampleLibrary.tagged }
        model.resetFilters()
        // Every third image is square (SampleLibrary.images).
        model.change { $0.filters.shape = .square }
        try await waitUntil("the shape filter") { model.total == 10 }
        model.change { $0.filters.shape = nil; $0.filters.rating = 0 }
        try await waitUntil("unrated") { model.total == 30 }
        model.change { $0.filters.rating = 5 }
        try await waitUntil("five stars") { model.total == 0 }
        XCTAssertTrue(model.settings.filters.isActive)
        model.stop()
        await open.close()
    }

    func testSelectAllActsOnTheWholeViewAndRefreshes() async throws {
        let open = try await sample(images: 12)
        let model = BrowseModel(open: open, place: .all)
        await model.start()
        try await waitUntil("the cells") { model.cell(at: 11) != nil }

        model.setSelecting(true)
        model.selectAll()
        XCTAssertEqual(model.selectedCount, 12)
        let kept = try XCTUnwrap(model.cell(at: 0))
        model.toggle(kept.id)
        XCTAssertEqual(model.selectedCount, 11)
        XCTAssertFalse(model.isSelected(kept.id))
        XCTAssertEqual(model.target.all, true)
        XCTAssertEqual(model.target.ids, [kept.id])

        let library = open.library
        let target = model.target
        let trashed = try await Task.detached { try library.edit(target, .trash) }.value
        XCTAssertEqual(trashed, 11)
        // The core recomputes the view and the model hears it.
        try await waitUntil("the refresh") { model.total == 1 }
        try await waitUntil("the fresh cell") { model.cell(at: 0)?.id == kept.id }
        try await waitUntil("the counts") { open.counts?.trash == 11 }

        let trash = BrowseModel(open: open, place: .trash)
        await trash.start()
        XCTAssertEqual(trash.total, 11)
        trash.stop()
        model.stop()
        await open.close()
    }

    func testTheViewIsSavedPerLibraryAndFiltersPerPlace() async throws {
        let defaults = UserDefaults(suiteName: "saved-\(UUID().uuidString)")!
        let open = try await sample(images: 3, defaults: defaults)
        let first = BrowseModel(open: open, place: .untagged)
        await first.start()
        first.change {
            $0.sort = .size
            $0.layout = .list
            $0.filters.ext = "png"
        }
        first.stop()

        let saved = SavedViews(defaults: defaults).load(for: open.url.standardizedFileURL.path)
        XCTAssertEqual(saved.place, .untagged)
        XCTAssertEqual(saved.sort, .size)

        let again = BrowseModel(open: open, place: .untagged)
        XCTAssertEqual(again.settings.filters.ext, "png")
        let other = BrowseModel(open: open, place: .all)
        XCTAssertEqual(other.settings.sort, .size)
        XCTAssertEqual(other.settings.layout, .list)
        XCTAssertNil(other.settings.filters.ext, "filters stay with their place")

        open.forgetPlace()
        XCTAssertNil(SavedViews(defaults: defaults).load(for: open.url.standardizedFileURL.path).place)
        await open.close()
    }
}
