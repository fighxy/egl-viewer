@testable import EglViewer
import EglCore
import XCTest

/// D-116: an Eagle library picked in Files is read, then imported into a new library on the
/// phone, and the report is shown once the import ends.
@MainActor
final class EagleImportTests: XCTestCase {
    private var dir: URL!

    override func setUp() async throws {
        dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("eagle-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: dir)
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

    /// A one-item Eagle library: a folder `Posters` with a PNG tagged `warm`.
    private func tinyEagleLibrary() throws -> URL {
        let root = dir.appendingPathComponent("Tiny.library", isDirectory: true)
        let info = root.appendingPathComponent("images/EIMG1.info", isDirectory: true)
        try FileManager.default.createDirectory(at: info, withIntermediateDirectories: true)
        func write(_ text: String, _ url: URL) throws { try Data(text.utf8).write(to: url) }
        try write(
            #"{"folders":[{"id":"FPOSTERS","name":"Posters","children":[]}],"smartFolders":[],"tagsGroups":[],"quickAccess":[],"modificationTime":1}"#,
            root.appendingPathComponent("metadata.json")
        )
        try write(#"{"EIMG1":2,"all":1}"#, root.appendingPathComponent("mtime.json"))
        try write(#"{"historyTags":[],"starredTags":[]}"#, root.appendingPathComponent("tags.json"))
        let image = try XCTUnwrap(SampleLibrary.images(1).first)
        let bytes = try Data(contentsOf: image)
        try bytes.write(to: info.appendingPathComponent("poster.png"))
        try write(
            #"{"id":"EIMG1","name":"poster","size":\#(bytes.count),"btime":1,"mtime":1,"ext":"png","tags":["warm"],"folders":["FPOSTERS"],"isDeleted":false,"url":"","annotation":"","modificationTime":1,"noThumbnail":true,"palettes":[],"lastModified":2}"#,
            info.appendingPathComponent("metadata.json")
        )
        return root
    }

    private func model() -> AppModel {
        AppModel(
            locations: LibraryLocations(root: dir.appendingPathComponent("Libraries", isDirectory: true)),
            recentStore: RecentLibraries(defaults: UserDefaults(suiteName: "eagle-\(UUID().uuidString)")!),
            arguments: []
        )
    }

    func testAnEagleLibraryBecomesANewLibraryOnThePhone() async throws {
        let eagle = try tinyEagleLibrary()
        let model = model()
        await model.pickEagle(eagle)
        let plan = try XCTUnwrap(model.eagleImport, model.error ?? "")
        XCTAssertEqual(plan.summary.name, "Tiny")
        XCTAssertEqual(plan.summary.itemCount, 1)

        await model.importEagle(plan, named: "Tiny", importTrash: false)
        XCTAssertNil(model.eagleImport)
        let open = try XCTUnwrap(model.openLibrary, model.error ?? "")
        XCTAssertEqual(open.info.name, "Tiny")
        try await waitUntil("the import report") { open.eagleOutcome != nil }
        let outcome = try XCTUnwrap(open.eagleOutcome)
        XCTAssertFalse(outcome.cancelled)
        XCTAssertEqual(outcome.report.imported, 1)
        XCTAssertTrue(EagleOutcomeAlert.message(outcome).contains("1"))
        try await waitUntil("the folder") { open.folders.contains { $0.name == "Posters" } }
        XCTAssertTrue(FileManager.default.fileExists(atPath: eagle.appendingPathComponent("metadata.json").path),
                      "the Eagle library is left as it was")
        await model.close()

        // A second import with the same name gets a free one.
        await model.pickEagle(eagle)
        let again = try XCTUnwrap(model.eagleImport)
        await model.importEagle(again, named: "Tiny", importTrash: false)
        XCTAssertEqual(model.openLibrary?.url.lastPathComponent, "Tiny 2.eglib")
        await model.close()
    }

    func testAFolderThatIsNotAnEagleLibraryIsRefused() async throws {
        let model = model()
        await model.pickEagle(dir)
        XCTAssertNil(model.eagleImport)
        XCTAssertEqual(model.error, String(localized: "eagle.notALibrary"))
        XCTAssertTrue(model.localLibraries.isEmpty, "no library was made")
    }
}
