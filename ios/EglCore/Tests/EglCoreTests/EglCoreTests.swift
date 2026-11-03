// EglCore against the real Rust core (crates/library-ffi), on the host: create, open, add
// files, views, edits with undo, events, locked folders, thumbnails and errors.

import CoreGraphics
import EglCore
import Foundation
import ImageIO
import UniformTypeIdentifiers
import XCTest

final class EglCoreTests: XCTestCase {
    private var dir: URL!

    override func setUpWithError() throws {
        dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("eglcore-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: dir)
    }

    /// A small PNG with a colour of its own, so duplicates are not detected.
    private func png(_ name: String, seed: Int) throws -> URL {
        let width = 40, height = 20
        let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(
            red: CGFloat(seed % 7) / 7, green: CGFloat(seed % 5) / 5, blue: CGFloat(seed % 3) / 3,
            alpha: 1
        )
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let url = dir.appendingPathComponent(name)
        let destination = CGImageDestinationCreateWithURL(
            url as CFURL, UTType.png.identifier as CFString, 1, nil
        )!
        CGImageDestinationAddImage(destination, context.makeImage()!, nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))
        return url
    }

    private func openNew(_ name: String = "Core") throws -> Library {
        let path = try Library.create(in: dir, name: name, appVersion: "0.0.0-test")
        return try Library.open(at: path, appVersion: "0.0.0-test")
    }

    /// Adds `count` images and waits for the import and its thumbnails.
    private func seeded(_ library: Library, count: Int) throws {
        let files = try (0..<count).map { try png("image-\($0).png", seed: $0 + 1) }
        let ticket = try library.addFiles(files)
        let deadline = Date().addingTimeInterval(30)
        while true {
            if let progress = try library.importStatus(ticket.id), progress.isFinished {
                XCTAssertEqual(progress.state, .done)
                XCTAssertEqual(progress.added, count)
                return
            }
            XCTAssertLessThan(Date(), deadline, "the import never ended")
            Thread.sleep(forTimeInterval: 0.02)
        }
    }

    func testCreateOpenAndInfo() throws {
        let library = try openNew("Mobile 📱")
        let info = try library.info()
        XCTAssertEqual(info.kind, .native)
        XCTAssertEqual(info.name, "Mobile 📱")
        XCTAssertFalse(info.readOnly)
        XCTAssertEqual(try library.counts().all, 0)
        library.close()
        XCTAssertThrowsError(try library.info())
    }

    func testOpeningSomethingElseIsAnError() throws {
        XCTAssertThrowsError(try Library.open(at: dir, appVersion: "0.0.0-test")) { error in
            // The reason comes from the core (egl_last_error_global).
            XCTAssertFalse((error as! EglError).message.isEmpty)
        }
    }

    func testViewsEditsUndoAndEvents() throws {
        let library = try openNew()
        let heard = Heard()
        try library.setEventHandler { heard.add($0) }
        try seeded(library, count: 3)

        let view = try library.openView()
        XCTAssertEqual(view.total, 3)
        let cells = try library.range(of: view.viewId, start: 0, count: 10)
        XCTAssertEqual(cells.count, 3)
        XCTAssertEqual(try library.dimensions(of: view.viewId).first, [40, 20])
        XCTAssertTrue(heard.events.contains { if case .importProgress = $0 { return true }; return false })

        let all = ItemsTarget(viewId: view.viewId, all: true, ids: [cells[0].id])
        XCTAssertEqual(try library.edit(all, .setStar(4)), 2)
        XCTAssertEqual(try library.item(cells[1].id).star, 4)
        XCTAssertEqual(try library.undoStatus()?.items, 2)
        try library.undo()
        XCTAssertEqual(try library.item(cells[1].id).star, 0)

        // Tags stay exactly as typed.
        try library.edit(cells[0].id, .addTags(["Poster", "poster"]))
        XCTAssertEqual(Set(try library.tags().map(\.name)), ["Poster", "poster"])

        try library.edit(ItemsTarget(ids: [cells[0].id]), .trash)
        XCTAssertTrue(heard.events.contains {
            if case let .viewsInvalidated(views) = $0 {
                return views.contains { $0.viewId == view.viewId && $0.total == 2 }
            }
            return false
        })
        XCTAssertEqual(try library.trashInfo().items, 1)
        XCTAssertEqual(try library.restoreTrash(), 1)
        XCTAssertEqual(try library.counts().all, 3)

        XCTAssertEqual(try library.extensions(), [ExtensionCount(ext: "png", count: 3)])
        XCTAssertEqual(try library.smartFolders(), [])
        let searched = try library.openView(LibraryQuery(text: "image-1"))
        XCTAssertEqual(searched.total, 1)
        let sorted = try library.openView(LibraryQuery(sort: .name, ascending: true))
        XCTAssertEqual(
            try library.range(of: sorted.viewId, start: 0, count: 3).map(\.name),
            ["image-0", "image-1", "image-2"]
        )

        let thumbnail = try XCTUnwrap(try library.thumbnail(cells[0].id))
        XCTAssertEqual(thumbnail.prefix(4), Data("RIFF".utf8))
        XCTAssertNil(try library.thumbnail("01NOTANITEM000000000000000"))
        try library.setEventHandler(nil)
    }

    func testALockedFolderHidesItsItemsUntilUnlocked() throws {
        let library = try openNew()
        try seeded(library, count: 1)
        let folder = try XCTUnwrap(try library.createFolder(named: "Private").first)
        let view = try library.openView()
        let id = try library.ids(of: view.viewId, start: 0, count: 1)[0]
        try library.edit(ItemsTarget(ids: [id]), .addToFolders([folder.id]))

        let key = try library.lockFolder(folder.id, password: "pässwörd 🔒")
        XCTAssertEqual(key.phrase.split(separator: " ").count, 12)
        XCTAssertEqual(try library.counts().all, 0)
        XCTAssertThrowsError(try library.unlockFolder(folder.id, password: "nope")) { error in
            XCTAssertEqual((error as! EglError).code, .wrongPassword)
        }
        try library.unlockFolder(folder.id, password: "pässwörd 🔒")
        XCTAssertEqual(try library.counts().all, 1)
        let original = try XCTUnwrap(try library.originalBytes(id))
        XCTAssertEqual(original.dropFirst().prefix(3), Data("PNG".utf8))
        try library.relock()
        XCTAssertEqual(try library.counts().all, 0)
    }
}

/// Events the handler heard, from the core's threads.
private final class Heard: @unchecked Sendable {
    private let lock = NSLock()
    private var list: [LibraryEvent] = []

    func add(_ event: LibraryEvent) {
        lock.lock()
        list.append(event)
        lock.unlock()
    }

    var events: [LibraryEvent] {
        lock.lock()
        defer { lock.unlock() }
        return list
    }
}
