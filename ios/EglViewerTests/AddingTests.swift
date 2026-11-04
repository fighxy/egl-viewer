@testable import EglViewer
import EglCore
import ImageIO
import UniformTypeIdentifiers
import XCTest

/// Adding against the real core on the simulator: copies cleaned up after the import, the
/// duplicate question, HEIC through ImageIO (the iOS core has no libheif) and file names.
@MainActor
final class AddingTests: XCTestCase {
    private func emptyLibrary() async throws -> OpenLibrary {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("adding-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = try Library.create(in: dir, name: "Adding", appVersion: "0.0.0-test")
        let library = try Library.open(at: path, appVersion: "0.0.0-test")
        let store = SavedViews(defaults: UserDefaults(suiteName: "adding-\(UUID().uuidString)")!)
        return try await OpenLibrary(library: library, url: path, scoped: false, savedViews: store)
    }

    private func waitUntil(_ what: String, timeout: TimeInterval = 30, _ condition: () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while !condition() {
            guard Date() < deadline else {
                XCTFail("timed out waiting for \(what)")
                return
            }
            try await Task.sleep(for: .milliseconds(20))
        }
    }

    private func picked(_ count: Int) throws -> [PickedFile] {
        try SampleLibrary.images(count).map { try PickedFile(copying: $0) }
    }

    func testPickedFilesAreAddedAndTheirCopiesRemoved() async throws {
        let open = try await emptyLibrary()
        let files = try picked(3)
        await open.add(files, into: nil)
        try await waitUntil("the import") { open.toast != nil }
        XCTAssertEqual(open.toast?.message, String(localized: "add.done \(3)"))
        await open.refresh()
        XCTAssertEqual(open.counts?.all, 3)
        for file in files {
            XCTAssertFalse(FileManager.default.fileExists(atPath: file.url.path), "the copy is gone")
        }
        await open.close()
    }

    func testADuplicateAsksAndTheAnswerHoldsForTheBatch() async throws {
        let open = try await emptyLibrary()
        let first = try picked(2)
        let again = try first.map { try PickedFile(copying: $0.url) }
        await open.add(first, into: nil)
        try await waitUntil("the first import") { open.toast != nil }
        open.dismissToast()

        await open.add(again, into: nil)
        try await waitUntil("the question") { open.duplicateQuestion != nil }
        XCTAssertNotNil(open.duplicateQuestion?.duplicate?.existing)
        await open.answer(.skip)
        try await waitUntil("the second import") { open.toast != nil }
        XCTAssertNil(open.duplicateQuestion, "one answer for the whole batch")
        XCTAssertTrue(open.toast?.message.contains(String(localized: "add.duplicates \(2)")) ?? false)
        await open.refresh()
        XCTAssertEqual(open.counts?.all, 2)
        await open.close()
    }

    func testHeicGetsDimensionsAndAThumbnailThroughImageIO() async throws {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("photo-\(UUID().uuidString).heic")
        let context = CGContext(
            data: nil, width: 320, height: 240, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(red: 0.9, green: 0.4, blue: 0.1, alpha: 1)
        context.fill(CGRect(x: 0, y: 0, width: 320, height: 240))
        guard let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.heic.identifier as CFString, 1, nil) else {
            throw XCTSkip("this simulator cannot write HEIC")
        }
        CGImageDestinationAddImage(destination, context.makeImage()!, nil)
        guard CGImageDestinationFinalize(destination) else { throw XCTSkip("this simulator cannot write HEIC") }

        PlatformDecoder.install()
        let open = try await emptyLibrary()
        await open.add([try PickedFile(copying: url)], into: nil)
        try await waitUntil("the import") { open.toast != nil }
        let library = open.library
        let cell = try await Task.detached { () -> ItemCell? in
            let view = try library.openView()
            return try library.range(of: view.viewId, start: 0, count: 1).first
        }.value
        XCTAssertEqual(cell?.ext, "heic")
        XCTAssertEqual(cell?.width, 320)
        XCTAssertEqual(cell?.height, 240)
        XCTAssertEqual(cell?.hasThumbnail, true)
        await open.close()
    }

    func testClipboardFileNames() {
        XCTAssertEqual(Clipboard.fileName(suggested: nil, type: .png, index: 0), "Pasted.png")
        XCTAssertEqual(Clipboard.fileName(suggested: "  ", type: .png, index: 1), "Pasted 2.png")
        XCTAssertEqual(Clipboard.fileName(suggested: "Poster.png", type: .png, index: 0), "Poster.png")
        XCTAssertEqual(Clipboard.fileName(suggested: "a/b", type: .heic, index: 0), "a-b.heic")
    }
}
