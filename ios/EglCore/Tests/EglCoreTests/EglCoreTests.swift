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
        let picked = try library.selectionTags(ItemsTarget(viewId: view.viewId, all: true, ids: []))
        XCTAssertEqual(picked.items, 3)
        XCTAssertEqual(picked.tags.first { $0.name == "Poster" }?.imageCount, 1)

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

    /// A JPEG (or HEIC where the host can write one) turned a quarter by its orientation.
    private func turned(_ type: UTType, _ name: String) throws -> URL? {
        let context = CGContext(
            data: nil, width: 300, height: 200, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )!
        context.setFillColor(red: 0.2, green: 0.6, blue: 0.9, alpha: 1)
        context.fill(CGRect(x: 0, y: 0, width: 300, height: 200))
        let url = dir.appendingPathComponent(name)
        guard let destination = CGImageDestinationCreateWithURL(url as CFURL, type.identifier as CFString, 1, nil) else {
            return nil
        }
        CGImageDestinationAddImage(destination, context.makeImage()!, [kCGImagePropertyOrientation: 6] as CFDictionary)
        return CGImageDestinationFinalize(destination) ? url : nil
    }

    func testThePlatformDecoderAppliesTheOrientation() throws {
        let url = try XCTUnwrap(try turned(.jpeg, "turned.jpg"))
        let image = try XCTUnwrap(PlatformDecoder.decode(try Data(contentsOf: url)))
        XCTAssertEqual([image.width, image.height], [200, 300])
        XCTAssertEqual(image.rgba.count, image.pixelWidth * image.pixelHeight * 4)
        XCTAssertNil(PlatformDecoder.decode(Data("not an image".utf8)))
    }

    func testHeicGoesThroughThePlatformDecoder() throws {
        guard let url = try turned(.heic, "turned.heic") else {
            throw XCTSkip("this host cannot write HEIC")
        }
        PlatformDecoder.install()
        defer { PlatformDecoder.uninstall() }
        let library = try openNew()
        let ticket = try library.addFiles([url])
        let deadline = Date().addingTimeInterval(30)
        while try library.importStatus(ticket.id)?.isFinished != true {
            XCTAssertLessThan(Date(), deadline, "the import never ended")
            Thread.sleep(forTimeInterval: 0.02)
        }
        let view = try library.openView()
        let cell = try XCTUnwrap(try library.range(of: view.viewId, start: 0, count: 1).first)
        XCTAssertEqual(cell.ext, "heic")
        XCTAssertEqual([cell.width, cell.height], [200, 300])
        XCTAssertTrue(cell.hasThumbnail)
    }

    func testADuplicateWaitsForAnAnswer() throws {
        let library = try openNew()
        try seeded(library, count: 1)
        let again = try png("image-0.png", seed: 1)
        let ticket = try library.addFiles([again])
        let deadline = Date().addingTimeInterval(30)
        var asked = false
        while true {
            let progress = try library.importStatus(ticket.id)
            if progress?.state == .waitingForDecision, !asked {
                XCTAssertNotNil(progress?.duplicate?.existing)
                try library.answerImport(ticket.id, .skip)
                asked = true
            }
            if let progress, progress.isFinished {
                XCTAssertTrue(asked)
                XCTAssertEqual(progress.duplicates, 1)
                XCTAssertEqual(progress.added, 0)
                break
            }
            XCTAssertLessThan(Date(), deadline, "the import never ended")
            Thread.sleep(forTimeInterval: 0.02)
        }
        XCTAssertEqual(try library.counts().all, 1)
    }

    func testFoldersTagsGroupsAndSmartFoldersAreEdited() throws {
        let library = try openNew()
        try seeded(library, count: 2)
        let view = try library.openView(LibraryQuery(sort: .name, ascending: true))
        let ids = try library.ids(of: view.viewId, start: 0, count: 2)

        // Folders: create, nest, update, move to the top, delete with or without items.
        let refs = try XCTUnwrap(try library.createFolder(named: "Refs").first)
        let tree = try library.saveFolder(.create(name: "Posters", parent: refs.id, iconColor: LabelColor.blue.rawValue))
        let posters = try XCTUnwrap(tree.first?.children.first)
        XCTAssertEqual(posters.iconColor, "blue")
        try library.saveFolder(.update(
            id: posters.id, name: "Posters 🎨", description: "Old ones", tags: ["Print"],
            iconColor: nil, orderBy: .name, sortAscending: true
        ))
        var moved = try library.saveFolder(.move(id: posters.id, parent: nil, index: 0))
        XCTAssertEqual(moved.map(\.name), ["Posters 🎨", "Refs"])
        XCTAssertEqual(moved.first?.orderBy, .name)
        try library.edit(ItemsTarget(ids: [ids[0]]), .addToFolders([posters.id]))
        moved = try library.deleteFolder(posters.id, trashItems: true)
        XCTAssertEqual(moved.map(\.name), ["Refs"])
        XCTAssertEqual(try library.counts().trash, 1)
        try library.restoreTrash()

        // Tags stay case-sensitive through rename and merge.
        try library.edit(ItemsTarget(ids: ids), .addTags(["Blue"]))
        try library.edit(ItemsTarget(ids: [ids[1]]), .addTags(["blue"]))
        try library.renameTag("Blue", to: "Navy")
        XCTAssertEqual(Set(try library.tags().map(\.name)), ["Navy", "blue"])
        try library.mergeTag("blue", into: "Navy")
        XCTAssertEqual(try library.tags().map(\.name), ["Navy"])
        let groups = try library.saveTagGroup(.create(name: "Colours", color: LabelColor.green.rawValue, tags: ["Navy"]))
        let group = try XCTUnwrap(groups.first)
        XCTAssertEqual(group.tags, ["Navy"])
        try library.saveTagGroup(.update(id: group.id, name: "Colors", color: nil, tags: []))
        XCTAssertEqual(try library.tagGroups().first?.name, "Colors")
        XCTAssertEqual(try library.deleteTagGroup(group.id), [])
        try library.deleteTag("Navy")
        XCTAssertEqual(try library.tags(), [])

        // Smart folders keep rules they do not know.
        let draft = try library.smartRules(of: LibraryQuery(ext: "png", rating: 0))
        let unknown: JSONValue = .object(["property": .string("palette"), "method": .string("near"), "value": .array([.number(1), .number(2)])])
        guard case var .array(conditions) = draft.conditions, case var .object(condition) = conditions[0],
              case var .array(rules) = condition["rules"] else {
            return XCTFail("unexpected draft \(draft.conditions)")
        }
        rules.append(unknown)
        condition["rules"] = .array(rules)
        conditions[0] = .object(condition)
        let smart = try library.saveSmartFolder(.create(name: "PNG", parent: nil, conditions: .array(conditions)))
        let folder = try XCTUnwrap(smart.first)
        XCTAssertEqual(folder.unsupported.count, 1)
        let stored = folder.conditions?.array?.first?.object?["rules"]?.array ?? []
        XCTAssertTrue(stored.contains(unknown))
        let shown = try library.openView(LibraryQuery(scope: .smart(id: folder.id)))
        XCTAssertEqual(shown.total, 0, "an unknown rule matches nothing")
        try library.saveSmartFolder(.update(id: folder.id, name: "PNGs", conditions: draft.conditions))
        XCTAssertEqual(try library.openView(LibraryQuery(scope: .smart(id: folder.id))).total, 2)
        XCTAssertEqual(try library.deleteSmartFolder(folder.id), [])
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
