@testable import EglViewer
import XCTest

/// What Welcome lists above its actions (D-130).
final class WelcomeEntriesTests: XCTestCase {
    private let locations = LibraryLocations(root: URL(fileURLWithPath: "/phone/Libraries", isDirectory: true))

    private func recent(_ name: String, _ path: String, _ openedAt: Date) -> RecentLibrary {
        RecentLibrary(name: name, path: path, bookmark: nil, openedAt: openedAt)
    }

    func testRecentLibrariesComeFirstAndOnMyIPhoneListsTheRest() {
        let now = Date()
        let entries = WelcomeEntries(
            recents: [
                recent("Refs", "/phone/Libraries/Refs.eglib", now),
                recent("Shared", "/cloud/Team/Shared.eglib", now.addingTimeInterval(-3600)),
            ],
            local: [
                URL(fileURLWithPath: "/phone/Libraries/Archive.eglib", isDirectory: true),
                URL(fileURLWithPath: "/phone/Libraries/Refs.eglib", isDirectory: true),
            ],
            locations: locations
        )
        XCTAssertEqual(entries.recent.map(\.name), ["Refs", "Shared"])
        XCTAssertEqual(entries.local.map(\.name), ["Archive"], "a recent library is listed once")
        XCTAssertFalse(entries.isEmpty)
    }

    func testEachEntrySaysWhereItLivesAndWhenItWasOpened() {
        let opened = Date(timeIntervalSince1970: 1_000)
        let entries = WelcomeEntries(
            recents: [
                recent("Refs", "/phone/Libraries/Refs.eglib", opened),
                recent("Shared", "/cloud/Team/Shared.eglib", opened),
            ],
            local: [URL(fileURLWithPath: "/phone/Libraries/Archive.eglib", isDirectory: true)],
            locations: locations
        )
        XCTAssertNil(entries.recent[0].folder, "On My iPhone")
        XCTAssertEqual(entries.recent[1].folder, "Team")
        XCTAssertEqual(entries.recent[0].openedAt, opened)
        XCTAssertNil(entries.local[0].folder)
        XCTAssertNil(entries.local[0].openedAt)
    }

    func testNoLibrariesIsEmpty() {
        XCTAssertTrue(WelcomeEntries(recents: [], local: [], locations: locations).isEmpty)
    }
}
