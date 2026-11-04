import XCTest

/// The app's login: turned on in its settings, asked for when the app comes back (D-098).
final class LocksUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    func testTheAppAsksForItsPasswordWhenItComesBack() {
        let app = XCUIApplication()
        // UI tests keep the login in memory, never in the Keychain.
        app.launchArguments = ["-uiTesting", "-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()

        app.buttons["welcome.settings"].tap()
        let appLock = app.descendants(matching: .any).matching(identifier: "settings.appLock").firstMatch
        XCTAssertTrue(appLock.waitForExistence(timeout: 5))
        appLock.tap()
        let new = app.secureTextFields["appLock.new"]
        XCTAssertTrue(new.waitForExistence(timeout: 5))
        new.tap()
        new.typeText("1234")
        let confirm = app.secureTextFields["appLock.confirm"]
        confirm.tap()
        confirm.typeText("1234")
        app.buttons["appLock.save"].tap()
        XCTAssertTrue(app.staticTexts["appLock.message"].waitForExistence(timeout: 10))
        app.buttons["settings.done"].tap()

        XCUIDevice.shared.press(.home)
        app.activate()

        let password = app.secureTextFields["login.password"]
        if !password.waitForExistence(timeout: 10) {
            // What the app shows instead, for the log.
            print(app.debugDescription)
            XCTFail("the login covers the app")
        }
        password.tap()
        password.typeText("0000")
        app.buttons["login.unlock"].tap()
        XCTAssertTrue(app.staticTexts["login.wrong"].waitForExistence(timeout: 5))
        password.tap()
        password.typeText("1234")
        app.buttons["login.unlock"].tap()
        XCTAssertTrue(app.buttons["welcome.create"].waitForExistence(timeout: 10))
        XCTAssertFalse(password.exists)
    }
}
