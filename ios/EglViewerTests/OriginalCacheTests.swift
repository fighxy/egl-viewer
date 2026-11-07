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
}
