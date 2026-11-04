@testable import EglViewer
import EglCore
import Security
import XCTest

/// The app's login and locked folders, against the real core.
@MainActor
final class LocksTests: XCTestCase {
    func testAHashMatchesOnlyItsPassword() {
        let hash = LoginHash.make("1234 🔐", iterations: 1000)
        XCTAssertTrue(hash.matches("1234 🔐"))
        XCTAssertFalse(hash.matches("1234"))
        XCTAssertFalse(hash.matches(""))
        XCTAssertNotEqual(LoginHash.make("1234 🔐", iterations: 1000).salt, hash.salt, "every hash has its own salt")
    }

    func testTheAppLockSetsChangesAndTurnsOff() throws {
        let lock = AppLock(store: MemoryLoginStore(), iterations: 1000)
        XCTAssertFalse(lock.enabled)
        lock.lock()
        XCTAssertFalse(lock.locked, "no login, no lock")

        try lock.setPassword("1234")
        lock.lock()
        XCTAssertTrue(lock.locked)
        XCTAssertFalse(lock.unlock("0000"))
        XCTAssertTrue(lock.locked)
        XCTAssertTrue(lock.unlock("1234"))
        XCTAssertFalse(lock.locked)

        XCTAssertFalse(try lock.change(current: "0000", to: "5678"))
        XCTAssertTrue(try lock.change(current: "1234", to: "5678"))
        XCTAssertFalse(try lock.turnOff(current: "1234"))
        XCTAssertTrue(try lock.turnOff(current: "5678"))
        XCTAssertFalse(lock.enabled)
    }

    func testWrongTriesMakeTheNextOneWait() throws {
        let lock = AppLock(store: MemoryLoginStore(), iterations: 1000)
        try lock.setPassword("1234")
        lock.lock()
        for _ in 0..<5 {
            XCTAssertFalse(lock.unlock("0000"))
        }
        let wait = try XCTUnwrap(lock.waitUntil)
        XCTAssertGreaterThan(wait, Date())
        XCTAssertFalse(lock.unlock("1234"), "even the right password waits")
        XCTAssertTrue(lock.locked)
    }

    func testTheKeychainKeepsTheLogin() throws {
        let store = KeychainLoginStore(service: "app.eglviewer.tests.\(UUID().uuidString)")
        do {
            try store.saveHash(LoginHash.make("1234", iterations: 1000))
        } catch let error as NSError where error.code == Int(errSecMissingEntitlement) {
            throw XCTSkip("this test host has no Keychain")
        }
        XCTAssertTrue(store.loadHash()?.matches("1234") ?? false)
        XCTAssertFalse(store.hasBiometricToken)
        try store.saveHash(nil)
        XCTAssertNil(store.loadHash())
    }

    func testAFolderLocksUnlocksAndLocksAgain() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("locks-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let path = try Library.create(in: dir, name: "Locks", appVersion: "0.0.0-test")
        let library = try Library.open(at: path, appVersion: "0.0.0-test")
        try await SampleLibrary.fill(library, images: 3)
        let store = SavedViews(defaults: UserDefaults(suiteName: "locks-\(UUID().uuidString)")!)
        let open = try await OpenLibrary(library: library, url: path, scoped: false, savedViews: store)

        let made = await open.saveFolder(.create(name: "Private", parent: nil))
        XCTAssertTrue(made)
        let folder = try XCTUnwrap(open.folders.first)
        let ids = try await Task.detached { () -> [String] in
            let view = try library.openView()
            return try library.ids(of: view.viewId, start: 0, count: 2)
        }.value
        _ = await open.perform { try $0.edit(ItemsTarget(ids: ids), .addToFolders([folder.id])) }

        let key = try await open.lockWork { try $0.lockFolder(folder.id, password: "secret", label: "Mine", labelColor: "blue") }.get()
        XCTAssertEqual(key.phrase.split(separator: " ").count, 12)
        XCTAssertEqual(open.counts?.all, 1, "the locked items are hidden")
        XCTAssertEqual(open.folders.first?.lockLabel, "Mine")
        XCTAssertFalse(open.hasUnlockedFolders)

        let wrong = await open.lockWork { try $0.unlockFolder(folder.id, password: "nope") }
        guard case let .failure(error) = wrong else { return XCTFail("a wrong password opened it") }
        XCTAssertEqual(error.code, .wrongPassword)
        XCTAssertNil(open.error, "a wrong password is said in the sheet, not the alert")

        _ = try await open.lockWork { try $0.unlockFolder(folder.id, password: "secret") }.get()
        XCTAssertEqual(open.counts?.all, 3)
        XCTAssertTrue(open.hasUnlockedFolders)
        await open.relockAll()
        XCTAssertFalse(open.hasUnlockedFolders)
        XCTAssertEqual(open.counts?.all, 1)
        await open.close()
    }
}
