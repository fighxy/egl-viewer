import XCTest

/// The app on a simulator with the real core: create a library, see it open, close it, and
/// find it under Recent.
final class WelcomeUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    func testCreateOpenCloseAndReopen() {
        let app = XCUIApplication()
        app.launchArguments = ["-uiTesting", "-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()

        app.buttons["welcome.create"].tap()
        let name = app.textFields["create.name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("UI Library")
        app.buttons["create.confirm"].tap()

        XCTAssertTrue(app.navigationBars["UI Library"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.descendants(matching: .any).matching(identifier: "place.all").firstMatch.exists)

        app.buttons["library.close"].tap()
        let recent = app.buttons["welcome.recent.UI Library"]
        XCTAssertTrue(recent.waitForExistence(timeout: 5))
        recent.tap()
        XCTAssertTrue(app.navigationBars["UI Library"].waitForExistence(timeout: 15))
    }

    func testTheWelcomeScreenSpeaksRussian() {
        let app = XCUIApplication()
        app.launchArguments = ["-uiTesting", "-AppleLanguages", "(ru)", "-AppleLocale", "ru_RU"]
        app.launch()
        let create = app.buttons["welcome.create"]
        XCTAssertTrue(create.waitForExistence(timeout: 10))
        XCTAssertEqual(create.label, "Создать библиотеку")
    }
}
