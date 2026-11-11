@testable import EglViewer
import EglCore
import XCTest

/// Original decode for the full-screen preview (D-095 / D-123).
@MainActor
final class OriginalCacheTests: XCTestCase {
    private func sample(images: Int) async throws -> OpenLibrary {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("original-cache-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = try Library.create(in: dir, name: "Originals", appVersion: "0.0.0-test")
        let library = try Library.open(at: path, appVersion: "0.0.0-test")
        try await SampleLibrary.fill(library, images: images)
        let store = SavedViews(defaults: UserDefaults(suiteName: "original-cache-\(UUID().uuidString)")!)
        return try await OpenLibrary(library: library, url: path, scoped: false, savedViews: store)
    }

    func testOriginalCacheReturnsADrawableImage() async throws {
        let open = try await sample(images: 2)
        let browse = BrowseModel(open: open, place: .all)
        await browse.start()
        let deadline = Date().addingTimeInterval(15)
        while browse.cell(at: 0) == nil, Date() < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        let cell = try XCTUnwrap(browse.cell(at: 0))

        let image = await open.originals.image(for: cell.id, version: cell.thumbVersion, pixels: 512)
        let decoded = try XCTUnwrap(image, "ImageIO should decode the sample original")
        XCTAssertGreaterThan(decoded.size.width, 0)
        XCTAssertGreaterThan(decoded.size.height, 0)

        let again = await open.originals.image(for: cell.id, version: cell.thumbVersion, pixels: 512)
        XCTAssertIdentical(again, decoded, "a second ask hits the memory cache")
    }

    func testOriginalCacheRejectsANonPositivePixelCap() async throws {
        let open = try await sample(images: 1)
        let browse = BrowseModel(open: open, place: .all)
        await browse.start()
        let deadline = Date().addingTimeInterval(15)
        while browse.cell(at: 0) == nil, Date() < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        let cell = try XCTUnwrap(browse.cell(at: 0))

        // pixels ≤ 0 used to reach ImageIO as maxPixelSize 0 and yield nothing useful;
        // the cache now clamps to at least 1 so a page with a transient zero geometry still
        // can decode once asked with a real size (and this call itself must not crash).
        let image = await open.originals.image(for: cell.id, version: cell.thumbVersion, pixels: 0)
        XCTAssertNotNil(image)
    }

    private func firstCell(_ open: OpenLibrary) async throws -> ItemCell {
        let browse = BrowseModel(open: open, place: .all)
        await browse.start()
        let deadline = Date().addingTimeInterval(15)
        while browse.cell(at: 0) == nil, Date() < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        return try XCTUnwrap(browse.cell(at: 0))
    }

    /// D-127: two pages asking for one original share one decode, and the result is there for
    /// the next page's first frame.
    func testTwoAsksShareOneDecodeAndTheResultIsCached() async throws {
        let open = try await sample(images: 1)
        let cell = try await firstCell(open)
        open.originals.removeAll()
        XCTAssertNil(open.originals.cached(for: cell.id, version: cell.thumbVersion, pixels: 300))
        async let one = open.originals.image(for: cell.id, version: cell.thumbVersion, pixels: 300)
        async let two = open.originals.image(for: cell.id, version: cell.thumbVersion, pixels: 300)
        let (first, second) = await (one, two)
        XCTAssertNotNil(first)
        XCTAssertIdentical(first, second, "one decode for both")
        XCTAssertIdentical(open.originals.cached(for: cell.id, version: cell.thumbVersion, pixels: 300), first)
        XCTAssertEqual(open.originals.runningCount, 0)
    }

    /// D-127: a page swiped away stops its decode; nothing keeps running for it.
    func testACancelledAskLeavesNoDecodeBehind() async throws {
        let open = try await sample(images: 1)
        let cell = try await firstCell(open)
        open.originals.removeAll()
        let task = Task { await open.originals.image(for: cell.id, version: cell.thumbVersion, pixels: 400) }
        task.cancel()
        let image = await task.value
        XCTAssertNil(image, "a cancelled ask gets nothing")
        XCTAssertEqual(open.originals.runningCount, 0)
        // Asked again, it decodes.
        let again = await open.originals.image(for: cell.id, version: cell.thumbVersion, pixels: 400)
        XCTAssertNotNil(again)
    }

    /// D-127: prefetching a neighbour leaves it in memory.
    func testAPrefetchWarmsTheCache() async throws {
        let open = try await sample(images: 1)
        let cell = try await firstCell(open)
        open.originals.removeAll()
        await open.originals.prefetch(cell.id, version: cell.thumbVersion, pixels: 256).value
        XCTAssertNotNil(open.originals.cached(for: cell.id, version: cell.thumbVersion, pixels: 256))
    }
}
