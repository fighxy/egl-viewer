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
        XCTAssertEqual(locations.storage(of: inside), .app)
        XCTAssertFalse(locations.opensReadOnly(inside, onDevice: nil))
        let icloud = URL(fileURLWithPath: "/private/var/mobile/Library/Mobile Documents/com~apple~CloudDocs/Refs.eglib")
        XCTAssertEqual(locations.storage(of: icloud), .cloud)
        XCTAssertTrue(locations.opensReadOnly(icloud, onDevice: true), "iCloud stays read-only")
        XCTAssertFalse(locations.isLocal(icloud))
    }

    /// On My iPhone outside the app and Dropbox-like providers share one kind of path: read-only
    /// until the user says the folder is on the device (D-132). A drive is on the device.
    func testAProviderFolderIsWritableOnlyOnceTheUserSaysItIsOnTheDevice() {
        let locations = LibraryLocations(root: URL(fileURLWithPath: "/app/Documents/Libraries", isDirectory: true))
        let provider = URL(fileURLWithPath: "/private/var/mobile/Containers/Shared/AppGroup/X/File Provider Storage/Refs/Refs.eglib")
        XCTAssertEqual(locations.storage(of: provider), .provider)
        XCTAssertTrue(locations.opensReadOnly(provider, onDevice: nil))
        XCTAssertTrue(locations.opensReadOnly(provider, onDevice: false))
        XCTAssertFalse(locations.opensReadOnly(provider, onDevice: true))
        let drive = URL(fileURLWithPath: "/private/var/mobile/Library/LiveFiles/com.apple.filesystems.userfsd/USB/Refs.eglib")
        XCTAssertEqual(locations.storage(of: drive), .device)
        XCTAssertFalse(locations.opensReadOnly(drive, onDevice: nil))
    }

    func testRecentListsSavedBeforeDecodeWithoutTheAnswer() throws {
        let old = #"[{"name":"Refs","path":"/a.eglib","openedAt":0}]"#
        let list = try JSONDecoder().decode([RecentLibrary].self, from: Data(old.utf8))
        XCTAssertEqual(list.map(\.name), ["Refs"])
        XCTAssertNil(list[0].onDevice)
    }

    /// A folder picked in Files outlives the app (D-132): the library is created there, opens
    /// read-write and is kept with a bookmark.
    @MainActor
    func testAModelCreatesALibraryInAPickedFolder() async throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent("pick-\(UUID().uuidString)", isDirectory: true)
        let folder = base.appendingPathComponent("Kept", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let model = AppModel(
            locations: LibraryLocations(root: base.appendingPathComponent("App/Libraries", isDirectory: true)),
            recentStore: RecentLibraries(defaults: UserDefaults(suiteName: "pick-\(UUID().uuidString)")!),
            arguments: []
        )
        await model.create(named: "Refs", in: folder)
        XCTAssertNil(model.error)
        let open = try XCTUnwrap(model.openLibrary)
        XCTAssertFalse(open.readOnly)
        await model.close()
        XCTAssertTrue(FileManager.default.fileExists(atPath: folder.appendingPathComponent("Refs.eglib").path))
        let recent = try XCTUnwrap(model.recents.first)
        XCTAssertNotNil(recent.bookmark)
        XCTAssertEqual(recent.onDevice, true)
        XCTAssertTrue(model.localLibraries.isEmpty, "nothing in the app's folder")

        let icloud = base.appendingPathComponent("Mobile Documents/com~apple~CloudDocs", isDirectory: true)
        try FileManager.default.createDirectory(at: icloud, withIntermediateDirectories: true)
        await model.create(named: "Synced", in: icloud)
        XCTAssertNotNil(model.error, "no library in iCloud Drive")
        XCTAssertFalse(FileManager.default.fileExists(atPath: icloud.appendingPathComponent("Synced.eglib").path))
    }

    /// A library in the app's folder moves to a picked folder: the copy opens, the original goes,
    /// and Recent points at the new place (D-132).
    @MainActor
    func testAModelMovesALibraryOutOfTheAppsFolder() async throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent("move-\(UUID().uuidString)", isDirectory: true)
        let folder = base.appendingPathComponent("Kept", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let model = AppModel(
            locations: LibraryLocations(root: base.appendingPathComponent("App/Libraries", isDirectory: true)),
            recentStore: RecentLibraries(defaults: UserDefaults(suiteName: "move-\(UUID().uuidString)")!),
            arguments: []
        )
        await model.create(named: "Refs")
        await model.close()
        let inApp = try XCTUnwrap(model.localLibraries.first)

        await model.move(inApp, to: folder)
        XCTAssertNil(model.error)
        let moved = folder.appendingPathComponent("Refs.eglib")
        XCTAssertTrue(FileManager.default.fileExists(atPath: moved.appendingPathComponent("library.db").path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: inApp.path), "the original is gone")
        XCTAssertTrue(model.localLibraries.isEmpty)
        let recent = try XCTUnwrap(model.recents.first)
        XCTAssertEqual(recent.path, moved.path)
        XCTAssertEqual(recent.onDevice, true)
        XCTAssertEqual(model.recents.count, 1)

        await model.open(recent: recent)
        let open = try XCTUnwrap(model.openLibrary)
        XCTAssertEqual(open.info.name, "Refs")
        XCTAssertFalse(open.readOnly)
        await model.close()

        // A second move into the same folder is refused, and nothing is lost.
        await model.create(named: "Refs")
        await model.close()
        let again = try XCTUnwrap(model.localLibraries.first)
        await model.move(again, to: folder)
        XCTAssertNotNil(model.error)
        XCTAssertTrue(FileManager.default.fileExists(atPath: again.path))
    }

    /// A library picked in a File Provider folder waits for the user to say where it is.
    @MainActor
    func testAPickedLibraryInAProviderFolderAsksWhereItIs() async {
        let model = AppModel(
            locations: LibraryLocations(root: URL(fileURLWithPath: "/app/Documents/Libraries", isDirectory: true)),
            recentStore: RecentLibraries(defaults: UserDefaults(suiteName: "ask-\(UUID().uuidString)")!),
            arguments: []
        )
        let picked = URL(fileURLWithPath: "/private/var/mobile/Containers/Shared/AppGroup/X/File Provider Storage/Refs.eglib")
        await model.open(picked: picked)
        XCTAssertEqual(model.askingWhere?.url, picked)
        XCTAssertNil(model.openLibrary)
    }

    @MainActor
    func testAModelCreatesOpensAndClosesALibrary() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("model-\(UUID().uuidString)/Libraries", isDirectory: true)
        let defaults = UserDefaults(suiteName: "model-\(UUID().uuidString)")!
        let model = AppModel(
            locations: LibraryLocations(root: root),
            recentStore: RecentLibraries(defaults: defaults),
            arguments: []
        )
        await model.create(named: "Refs 📚")
        let open = try XCTUnwrap(model.openLibrary)
        XCTAssertEqual(open.info.name, "Refs 📚")
        XCTAssertEqual(open.counts?.all, 0)
        await model.close()
        XCTAssertEqual(model.recents.map(\.name), ["Refs 📚"])
        XCTAssertNil(model.error)

        await model.create(named: "   ")
        XCTAssertNotNil(model.error)
    }
}
