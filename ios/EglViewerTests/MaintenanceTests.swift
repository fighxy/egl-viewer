@testable import EglViewer
import EglCore
import XCTest

/// Library settings, backups, check and export against the real core.
@MainActor
final class MaintenanceTests: XCTestCase {
    private func sample(images: Int) async throws -> OpenLibrary {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("upkeep-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = try Library.create(in: dir, name: "Upkeep", appVersion: "0.0.0-test")
        let library = try Library.open(at: path, appVersion: "0.0.0-test")
        try await SampleLibrary.fill(library, images: images)
        let store = SavedViews(defaults: UserDefaults(suiteName: "upkeep-\(UUID().uuidString)")!)
        return try await OpenLibrary(library: library, url: path, scoped: false, savedViews: store)
    }

    private func waitUntil(_ what: String, timeout: TimeInterval = 60, _ condition: () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while !condition() {
            guard Date() < deadline else {
                XCTFail("timed out waiting for \(what)")
                return
            }
            try await Task.sleep(for: .milliseconds(30))
        }
    }

    func testRenameBackUpAndRestoreBringsTheOldStateBack() async throws {
        let open = try await sample(images: 2)
        let renamed = await open.rename(to: "Archive 📦")
        XCTAssertTrue(renamed)
        XCTAssertEqual(open.info.name, "Archive 📦")

        let made = await open.perform { try $0.backUpNow() }
        let backup = try XCTUnwrap(made)
        let browse = BrowseModel(open: open, place: .all)
        await browse.start()
        XCTAssertEqual(browse.total, 2)
        let ids = await open.perform { library -> [String] in
            let view = try library.openView()
            return try library.ids(of: view.viewId, start: 0, count: 2)
        } ?? []
        _ = await open.perform { try $0.edit(ItemsTarget(ids: ids), .trash) }
        try await waitUntil("the trash") { browse.total == 0 }

        let restored = await open.restoreBackup(backup.name)
        XCTAssertTrue(restored)
        XCTAssertEqual(open.counts?.all, 2)
        try await waitUntil("the reopened view") { browse.total == 2 }
        let kinds = await open.perform { try $0.backups().compactMap(\.kind) } ?? []
        XCTAssertTrue(kinds.contains(.beforeRestore), "the state before the restore is kept")
        browse.stop()
        await open.close()
    }

    func testACleanLibraryChecksCleanAndExportsAsAZipAndAFolder() async throws {
        let open = try await sample(images: 3)
        let checked = await open.perform { try $0.check() }
        let check = try XCTUnwrap(checked)
        XCTAssertTrue(check.isClean)
        let read = await open.perform { try $0.stats() }
        let stats = try XCTUnwrap(read)
        XCTAssertGreaterThan(stats.total, 0)

        for format in [ExportFormat.zip(.fast), .mirror] {
            let out = FileManager.default.temporaryDirectory
                .appendingPathComponent("export-test-\(UUID().uuidString)", isDirectory: true)
            try FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
            let request = ExportRequest(
                selection: ExportSelection(whole: true), includeSubfolders: true, format: format, destination: out.path
            )
            let begun = await open.startExport(request)
            let started = try XCTUnwrap(begun)
            try await waitUntil("the export") { open.exports[started.id]?.isFinished == true }
            let done = try XCTUnwrap(open.exports[started.id])
            XCTAssertEqual(done.state, .done, done.error ?? "")
            XCTAssertEqual(done.summary?.items, 3)
            var directory: ObjCBool = false
            XCTAssertTrue(FileManager.default.fileExists(atPath: done.summary?.path ?? "", isDirectory: &directory))
            XCTAssertEqual(directory.boolValue, format == .mirror)
            open.forgetExport(started.id)
            XCTAssertNil(open.exports[started.id])
            try? FileManager.default.removeItem(at: out)
        }
        await open.close()
    }
}
