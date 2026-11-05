import XCTest

/// Settings on a generated library (`-sampleLibrary 4`): Check Library and an export (D-099).
final class SettingsUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func element(_ id: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: id).firstMatch
    }

    func testCheckAndExportFromSettings() {
        let app = XCUIApplication()
        app.launchArguments = [
            "-uiTesting", "-sampleLibrary", "4", "-AppleLanguages", "(en)", "-AppleLocale", "en_US",
        ]
        app.launch()
        XCTAssertTrue(app.navigationBars["Sample"].waitForExistence(timeout: 30))
        // The four images are in once the count shows them.
        XCTAssertTrue(element("place.all", in: app).waitForExistence(timeout: 10))

        app.buttons["library.settings"].tap()
        let check = element("settings.check", in: app)
        XCTAssertTrue(check.waitForExistence(timeout: 5))
        check.tap()
        app.buttons["check.run"].tap()
        XCTAssertTrue(element("check.clean", in: app).waitForExistence(timeout: 20))
        // Back from the Check page to the list of Settings.
        app.buttons["BackButton"].firstMatch.tap()
        let export = element("settings.export", in: app)
        XCTAssertTrue(export.waitForExistence(timeout: 5))
        export.tap()
        let start = app.buttons["export.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 10))
        let enabled = XCTNSPredicateExpectation(predicate: NSPredicate(format: "isEnabled == true"), object: start)
        XCTAssertEqual(XCTWaiter.wait(for: [enabled], timeout: 20), .completed, "the preview counts the items")
        start.tap()
        XCTAssertTrue(element("export.done", in: app).waitForExistence(timeout: 60))
        XCTAssertTrue(app.buttons["export.saveToFiles"].exists)
    }

    /// D-110: Decrypt Archive opens from the app's settings and waits for an archive.
    func testDecryptArchiveWaitsForAnArchiveAndAPassphrase() {
        let app = XCUIApplication()
        app.launchArguments = ["-uiTesting", "-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()
        app.buttons["welcome.settings"].tap()
        let decrypt = element("settings.decrypt", in: app)
        XCTAssertTrue(decrypt.waitForExistence(timeout: 5))
        decrypt.tap()
        let start = app.buttons["decrypt.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 5))
        XCTAssertFalse(start.isEnabled, "no archive picked yet")
        XCTAssertTrue(element("decrypt.choose", in: app).exists)
    }
}
