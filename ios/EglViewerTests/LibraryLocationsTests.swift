@testable import EglViewer
import XCTest

final class LibraryLocationsTests: XCTestCase {
    func testRecentLibrariesKeepTheNewestFirstAndEachPathOnce() {
        let defaults = UserDefaults(suiteName: "recent-\(UUID().uuidString)")!
        let store = RecentLibraries(defaults: defaults)
        store.remember(RecentLibrary(name: "A", path: "/a.eglib", bookmark: nil, openedAt: Date()))
        store.remember(RecentLibrary(name: "B", path: "/b.eglib", bookmark: nil, openedAt: Date()))
        let list = store.remember(RecentLibrary(name: "A again", path: "/a.eglib", bookmark: nil, openedAt: Date()))
        XCTAssertEqual(list.map(\.name), ["A again", "B"])
        XCTAssertEqual(store.forget(path: "/b.eglib").map(\.path), ["/a.eglib"])
        for n in 0..<(RecentLibraries.kept + 5) {
            store.remember(RecentLibrary(name: "\(n)", path: "/\(n).eglib", bookmark: nil, openedAt: Date()))
        }
        XCTAssertEqual(store.load().count, RecentLibraries.kept)
    }

    func testLibrariesInTheAppContainerAreLocalAndICloudIsSynced() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("loc-\(UUID().uuidString)/Libraries", isDirectory: true)
        let locations = LibraryLocations(root: root)
        try locations.ensureRoot()
        let inside = root.appendingPathComponent("Refs.eglib", isDirectory: true)
        try FileManager.default.createDirectory(at: inside, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(
            at: root.appendingPathComponent("not-a-library", isDirectory: true),
            withIntermediateDirectories: true
        )
        XCTAssertTrue(locations.isLocal(inside))
        XCTAssertEqual(locations.localLibraries().map(\.lastPathComponent), ["Refs.eglib"])
        XCTAssertFalse(LibraryLocations.isSynced(inside))
        let icloud = URL(fileURLWithPath: "/private/var/mobile/Library/Mobile Documents/com~apple~CloudDocs/Refs.eglib")
        XCTAssertTrue(LibraryLocations.isSynced(icloud))
        XCTAssertFalse(locations.isLocal(icloud))
    }

    func testAModelCreatesOpensAndClosesALibrary() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("model-\(UUID().uuidString)/Libraries", isDirectory: true)
        let defaults = UserDefaults(suiteName: "model-\(UUID().uuidString)")!
        let model = await AppModel(
            locations: LibraryLocations(root: root),
            recentStore: RecentLibraries(defaults: defaults),
            arguments: []
        )
        await model.create(named: "Refs 📚")
        let open = try await XCTUnwrap(model.openLibrary)
        let name = await open.info.name
        XCTAssertEqual(name, "Refs 📚")
        let counts = await open.counts
        XCTAssertEqual(counts?.all, 0)
        await model.close()
        let recents = await model.recents
        XCTAssertEqual(recents.map(\.name), ["Refs 📚"])
        let error = await model.error
        XCTAssertNil(error)

        await model.create(named: "   ")
        let refused = await model.error
        XCTAssertNotNil(refused)
    }
}
