@testable import EglViewer
import EglCore
import XCTest

/// Item details, actions on a selection with undo, sharing and tag suggestions, against the
/// real core.
@MainActor
final class DetailsAndActionsTests: XCTestCase {
    private func sample(images: Int) async throws -> OpenLibrary {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("details-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = try Library.create(in: dir, name: "Details", appVersion: "0.0.0-test")
        let library = try Library.open(at: path, appVersion: "0.0.0-test")
        try await SampleLibrary.fill(library, images: images)
        let store = SavedViews(defaults: UserDefaults(suiteName: "details-\(UUID().uuidString)")!)
        return try await OpenLibrary(library: library, url: path, scoped: false, savedViews: store)
    }

    private func waitUntil(_ what: String, timeout: TimeInterval = 15, _ condition: () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while !condition() {
            guard Date() < deadline else {
                XCTFail("timed out waiting for \(what)")
                return
            }
            try await Task.sleep(for: .milliseconds(20))
        }
    }

    func testDetailsEditTheItemAndUndoTakesEachEditBack() async throws {
        let open = try await sample(images: 2)
        let created = await open.perform { try $0.createFolder(named: "Refs") }
        let folder = try XCTUnwrap(created?.first)
        await open.refresh()
        let browse = BrowseModel(open: open, place: .all)
        await browse.start()
        try await waitUntil("the cells") { browse.cell(at: 1) != nil }
        let cell = try XCTUnwrap(browse.cell(at: 0))

        let details = ItemDetailsModel(open: open, id: cell.id)
        await details.load()
        XCTAssertEqual(details.item?.name, cell.name)
        XCTAssertEqual(details.vocabulary.map(\.name), [SampleLibrary.tag])

        await details.rename("Renamed ✨")
        XCTAssertEqual(details.item?.name, "Renamed ✨")
        await details.rate(4)
        XCTAssertEqual(details.item?.star, 4)
        await details.rate(4)
        XCTAssertEqual(details.item?.star, 0, "the same rating again clears it")
        await details.addTag("Mood")
        await details.addTag("mood")
        XCTAssertEqual(details.item?.tags.filter { $0.lowercased() == "mood" }.sorted(), ["Mood", "mood"])
        await details.removeTag("mood")
        XCTAssertFalse(details.item?.tags.contains("mood") ?? true)
        await details.toggleFolder(folder.id)
        XCTAssertEqual(details.folderNames.map(\.name), ["Refs"])
        await details.setNotes("Line one\nline two")
        await details.setLink("https://example.com/a")
        XCTAssertEqual(details.item?.annotation, "Line one\nline two")
        XCTAssertEqual(details.item?.url, "https://example.com/a")

        // The view hears the rename.
        try await waitUntil("the renamed cell") {
            (0..<2).contains { browse.cell(at: $0)?.name == "Renamed ✨" }
        }

        await open.undo()
        await details.load()
        XCTAssertEqual(details.item?.url, "")
        XCTAssertEqual(open.toast?.message, String(localized: "toast.undone"))
        browse.stop()
        await open.close()
    }

    func testTrashRestoreAndDeleteForGoodFromASelection() async throws {
        let open = try await sample(images: 6)
        let browse = BrowseModel(open: open, place: .all)
        await browse.start()
        try await waitUntil("the cells") { browse.cell(at: 5) != nil }

        browse.setSelecting(true)
        browse.toggle(try XCTUnwrap(browse.cell(at: 0)).id)
        browse.toggle(try XCTUnwrap(browse.cell(at: 1)).id)
        await browse.trash(browse.target)
        XCTAssertFalse(browse.selecting, "an action ends the selection")
        XCTAssertEqual(open.toast?.undoable, true)
        try await waitUntil("the trash") { browse.total == 4 }

        await open.undo()
        try await waitUntil("the undo") { browse.total == 6 }

        browse.setSelecting(true)
        browse.selectAll()
        await browse.rate(browse.target, stars: 3)
        let rated = try await waitForCell(browse, at: 5)
        XCTAssertEqual(rated.star, 3)

        browse.setSelecting(true)
        browse.selectAll()
        await browse.trash(browse.target)
        try await waitUntil("all in the trash") { browse.total == 0 }
        browse.stop()

        let trash = BrowseModel(open: open, place: .trash)
        await trash.start()
        XCTAssertEqual(trash.total, 6)
        try await waitUntil("the trash cells") { trash.cell(at: 5) != nil }
        let first = try XCTUnwrap(trash.cell(at: 0))
        await trash.restore(ItemsTarget(ids: [first.id]))
        try await waitUntil("the restore") { trash.total == 5 }
        await trash.deleteForGood(ItemsTarget(viewId: trash.viewId, all: true, ids: []))
        try await waitUntil("the delete") { trash.total == 0 }
        XCTAssertEqual(open.toast?.undoable, false)
        await open.refresh()
        XCTAssertEqual(open.counts?.all, 1)
        XCTAssertEqual(open.counts?.trash, 0)
        trash.stop()
        await open.close()
    }

    func testSharingNamesTheFilesAfterTheItems() async throws {
        let open = try await sample(images: 3)
        let browse = BrowseModel(open: open, place: .all)
        await browse.start()
        browse.change { $0.sort = .name; $0.ascending = true }
        try await waitUntil("the name sort") { browse.cell(at: 0)?.name == "sample-0" && browse.cell(at: 2) != nil }

        let excluded = try XCTUnwrap(browse.cell(at: 1)).id
        let ids = await browse.ids(of: ItemsTarget(viewId: browse.viewId, all: true, ids: [excluded]), limit: 10)
        XCTAssertEqual(ids.count, 2, "Select All without the excluded item")

        let shared = await browse.shareFiles(ItemsTarget(viewId: browse.viewId, all: true, ids: []))
        let files = try XCTUnwrap(shared)
        XCTAssertEqual(files.urls.map(\.lastPathComponent), ["sample-0.png", "sample-1.png", "sample-2.png"])
        let data = try Data(contentsOf: files.urls[0])
        XCTAssertEqual(data.dropFirst().prefix(3), Data("PNG".utf8))
        files.clean()
        XCTAssertFalse(FileManager.default.fileExists(atPath: files.folder.path))
        browse.stop()
        await open.close()
    }

    func testFileNamesAndTagSuggestions() {
        var used = Set<String>()
        XCTAssertEqual(ShareFiles.unique("a.png", in: &used), "a.png")
        XCTAssertEqual(ShareFiles.unique("A.png", in: &used), "A (2).png")
        XCTAssertEqual(ShareFiles.unique("a.png", in: &used), "a (3).png")

        let vocabulary = ["Poster", "poster art", "Typography", "Pastel"].map { Tag(name: $0, imageCount: nil) }
        XCTAssertEqual(
            TagSuggestions.matching("po", in: vocabulary, excluding: ["Poster"]),
            ["poster art", "Typography"]
        )
        XCTAssertEqual(TagSuggestions.matching("st", in: vocabulary, excluding: []), ["Poster", "poster art", "Pastel"])
        XCTAssertEqual(TagSuggestions.matching("  ", in: vocabulary, excluding: []), [])
    }

    private func waitForCell(_ model: BrowseModel, at index: Int) async throws -> ItemCell {
        var found: ItemCell?
        try await waitUntil("the rated cell") {
            found = model.cell(at: index)
            return found?.star == 3
        }
        return try XCTUnwrap(found)
    }
}
