@testable import EglViewer
import EglCore
import XCTest

/// A library of 10,000 items on the phone (D-100): the view opens, windows anywhere in it
/// arrive, the sort and the search answer, and Select All selects the whole view, each within
/// a budget. The budgets are loose on purpose (CI simulators are slow and shared); they catch
/// work that grows with the library on the main path, like a whole item list over the C ABI.
@MainActor
final class PerformanceTests: XCTestCase {
    static let items = 10_000

    private func waitUntil(
        _ what: String, within budget: TimeInterval, _ condition: () -> Bool
    ) async throws -> TimeInterval {
        let started = Date()
        while !condition() {
            let spent = Date().timeIntervalSince(started)
            guard spent < budget else {
                XCTFail("\(what): more than \(budget) s")
                return spent
            }
            try await Task.sleep(for: .milliseconds(10))
        }
        let spent = Date().timeIntervalSince(started)
        print("perf: \(what) in \(String(format: "%.3f", spent)) s")
        return spent
    }

    func testTenThousandItems() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("perf-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let path = try Library.create(in: dir, name: "Perf", appVersion: "0.0.0-test")
        let library = try Library.open(at: path, appVersion: "0.0.0-test")
        let filled = Date()
        try await SampleLibrary.fill(library, images: Self.items)
        print("perf: \(Self.items) images added in \(String(format: "%.1f", Date().timeIntervalSince(filled))) s")
        let store = SavedViews(defaults: UserDefaults(suiteName: "perf-\(UUID().uuidString)")!)
        let open = try await OpenLibrary(library: library, url: path, scoped: false, savedViews: store)
        XCTAssertEqual(open.counts?.all, Self.items)

        let model = BrowseModel(open: open, place: .all)
        let opening = Date()
        await model.start()
        XCTAssertEqual(model.total, Self.items)
        _ = try await waitUntil("the first window", within: 3) { model.cell(at: 0) != nil }
        XCTAssertLessThan(Date().timeIntervalSince(opening), 5, "the view opens")

        // A jump to the end, as a fast scroll does: only the windows around it are loaded.
        model.need(Self.items - 10)
        _ = try await waitUntil("a window at the end", within: 3) { model.cell(at: Self.items - 1) != nil }
        XCTAssertNil(model.cell(at: Self.items / 2), "the middle is not loaded")

        model.change { $0.sort = .name; $0.ascending = true }
        _ = try await waitUntil("the name sort", within: 5) { model.cell(at: 0)?.name == "sample-0" }

        model.change { $0.search = "sample-9999" }
        _ = try await waitUntil("a search", within: 5) { model.total == 1 }
        model.change { $0.search = "" }
        _ = try await waitUntil("the search cleared", within: 5) { model.total == Self.items }

        model.toggleTag(SampleLibrary.tag)
        _ = try await waitUntil("a tag filter", within: 5) { model.total == SampleLibrary.tagged }
        model.resetFilters()
        _ = try await waitUntil("the filter cleared", within: 5) { model.total == Self.items }

        // Select All stays a rule over the view: no list of 10,000 ids.
        model.setSelecting(true)
        model.selectAll()
        XCTAssertEqual(model.selectedCount, Self.items)
        XCTAssertTrue(model.target.all)
        XCTAssertTrue(model.target.ids.isEmpty)

        model.stop()
        await open.close()
    }
}
