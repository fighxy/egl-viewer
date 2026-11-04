@testable import EglViewer
import EglCore
import XCTest

/// Folders, tags, tag groups and smart folders against the real core, and the smart rule model.
@MainActor
final class OrganizeTests: XCTestCase {
    private func sample(images: Int) async throws -> OpenLibrary {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("organize-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = try Library.create(in: dir, name: "Organize", appVersion: "0.0.0-test")
        let library = try Library.open(at: path, appVersion: "0.0.0-test")
        try await SampleLibrary.fill(library, images: images)
        let store = SavedViews(defaults: UserDefaults(suiteName: "organize-\(UUID().uuidString)")!)
        return try await OpenLibrary(library: library, url: path, scoped: false, savedViews: store)
    }

    func testRulesKeepWhatTheEditorDoesNotKnow() {
        let stored: JSONValue = .array([.object([
            "match": .string("OR"), "boolean": .string("FALSE"), "eagleOnly": .bool(true),
            "rules": .array([
                .object(["property": .string("tags"), "method": .string("union"), "value": .array([.string("Red")])]),
                .object(["property": .string("palette"), "method": .string("near"), "value": .number(3)]),
            ]),
        ])])
        var conditions = SmartRules.read(stored)
        XCTAssertEqual(conditions.count, 1)
        XCTAssertTrue(conditions[0].any)
        XCTAssertTrue(conditions[0].negated)
        XCTAssertTrue(conditions[0].rules[0].editable)
        XCTAssertFalse(conditions[0].rules[1].editable)
        XCTAssertEqual(SmartRules.write(conditions), stored, "an untouched read writes back the same")

        conditions[0].any = false
        let written = SmartRules.write(conditions).array?.first?.object
        XCTAssertEqual(written?["match"], .string("AND"))
        XCTAssertEqual(written?["eagleOnly"], .bool(true))
        XCTAssertEqual(written?["rules"]?.array?.count, 2)
        XCTAssertTrue(SmartRules.read(.string("junk")).isEmpty)
    }

    func testMethodsKeepTheValueThatStillFits() {
        let size = SmartRules.newRule("fileSize")
        XCTAssertEqual(size.fields["unit"], .string("mb"))
        let between = SmartRules.withMethod(size, "between")
        XCTAssertEqual(between.value?.numbers, [1, 1])
        XCTAssertEqual(SmartRules.withMethod(between, "<").value?.numbers, [1])

        let recent = SmartRules.newRule("createTime")
        XCTAssertEqual(recent.method, "within")
        let on = SmartRules.withMethod(recent, "on")
        XCTAssertEqual(on.value?.numbers.first, SmartRules.today())
        let range = SmartRules.withMethod(on, "between").value?.numbers ?? []
        XCTAssertEqual(range.count, 2)
        XCTAssertEqual(range[1] - range[0], 7 * SmartRules.dayMilliseconds)
        XCTAssertEqual(SmartRules.withProperty(recent, "rating").method, "equal")
        XCTAssertFalse(SmartRules.needsValue("not-empty"))
    }

    func testFoldersAreMadeEditedMovedAndDeleted() async throws {
        let open = try await sample(images: 2)
        let ok1 = await open.saveFolder(.create(name: "Refs", parent: nil))
        XCTAssertTrue(ok1)
        let refs = try XCTUnwrap(open.folders.first)
        let ok2 = await open.saveFolder(.create(name: "Posters", parent: refs.id, iconColor: "pink"))
        XCTAssertTrue(ok2)
        let posters = try XCTUnwrap(open.folders.first?.children.first)
        XCTAssertEqual(open.parent(of: posters.id), refs.id)
        XCTAssertEqual(open.subtree(of: refs.id), [refs.id, posters.id])
        let ok3 = await open.saveFolder(.move(id: refs.id, parent: posters.id, index: 0))
        XCTAssertFalse(ok3, "not into itself")
        open.error = nil

        let ok4 = await open.saveFolder(.move(id: posters.id, parent: nil, index: open.endIndex(under: nil)))
        XCTAssertTrue(ok4)
        XCTAssertEqual(open.folders.map(\.name), ["Refs", "Posters"])
        XCTAssertNil(open.parent(of: posters.id))
        XCTAssertEqual(open.folders.last?.iconColor, "pink")

        let ids = try await Task.detached { [library = open.library] () -> [String] in
            let view = try library.openView()
            return try library.ids(of: view.viewId, start: 0, count: 2)
        }.value
        _ = await open.perform { try $0.edit(ItemsTarget(ids: ids), .addToFolders([posters.id])) }
        open.remember(SavedView(place: .folder(id: posters.id, name: "Posters")))
        await open.deleteFolder(posters.id, trashItems: true)
        XCTAssertEqual(open.folders.map(\.name), ["Refs"])
        XCTAssertEqual(open.counts?.trash, 2)
        XCTAssertNil(open.savedView.place, "a deleted folder is not reopened")
        await open.close()
    }

    func testTagsAreRenamedMergedGroupedAndBrowsed() async throws {
        let open = try await sample(images: 6)
        let tagged = BrowseModel(open: open, place: .tag(name: SampleLibrary.tag))
        await tagged.start()
        XCTAssertEqual(tagged.total, SampleLibrary.tagged)
        tagged.stop()

        let ok5 = await open.renameTag(SampleLibrary.tag, to: "red")
        XCTAssertTrue(ok5)
        let ok6 = await open.renameTag("red", to: "red")
        XCTAssertFalse(ok6)
        let ids = try await Task.detached { [library = open.library] () -> [String] in
            let view = try library.openView()
            return try library.ids(of: view.viewId, start: 0, count: 1)
        }.value
        _ = await open.perform { try $0.edit(ItemsTarget(ids: ids), .addTags(["Crimson"])) }
        let ok7 = await open.mergeTag("Crimson", into: "red")
        XCTAssertTrue(ok7)
        let names = await open.perform { try $0.tags().map(\.name) }
        XCTAssertEqual(names, ["red"])

        let ok8 = await open.saveTagGroup(.create(name: "Colours", color: "red", tags: ["red"]))
        XCTAssertTrue(ok8)
        let group = try XCTUnwrap(open.tagGroups.first)
        let ok9 = await open.saveTagGroup(.update(id: group.id, name: "Colors", color: nil, tags: []))
        XCTAssertTrue(ok9)
        XCTAssertEqual(open.tagGroups.first?.name, "Colors")
        await open.deleteTagGroup(group.id)
        XCTAssertTrue(open.tagGroups.isEmpty)
        await open.deleteTag("red")
        let left = await open.perform { try $0.tags() }
        XCTAssertEqual(left, [])
        await open.close()
    }

    func testAViewIsSavedAsASmartFolder() async throws {
        let open = try await sample(images: 6)
        let browse = BrowseModel(open: open, place: .all)
        await browse.start()
        browse.change { $0.filters.tags = [SampleLibrary.tag] }
        await browse.reload()
        await browse.saveAsSmartFolder()
        guard case let .create(parent, conditions)? = browse.smartDraft else {
            return XCTFail("no draft")
        }
        XCTAssertNil(parent)
        let rules = try XCTUnwrap(conditions)
        let ok10 = await open.saveSmartFolder(.create(name: "Reds", parent: nil, conditions: rules))
        XCTAssertTrue(ok10)
        let smart = try XCTUnwrap(open.smartFolders.first)
        browse.stop()

        let shown = BrowseModel(open: open, place: .smart(id: smart.id, name: smart.name))
        await shown.start()
        XCTAssertEqual(shown.total, SampleLibrary.tagged)
        shown.stop()
        await open.deleteSmartFolder(smart.id)
        XCTAssertTrue(open.smartFolders.isEmpty)
        await open.close()
    }
}
