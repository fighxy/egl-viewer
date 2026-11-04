import XCTest

/// Browsing a generated library (`-sampleLibrary 12`): the grid fills, search and the tag
/// filter narrow it, Select All counts the view, and the list layout shows the same items.
final class BrowseUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = [
            "-uiTesting", "-sampleLibrary", "12", "-AppleLanguages", "(en)", "-AppleLocale", "en_US",
        ]
        app.launch()
        return app
    }

    /// The element with identifier `id`, whatever its type.
    private func element(_ id: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: id).firstMatch
    }

    private func waitForLabel(_ label: String, of element: XCUIElement, timeout: TimeInterval = 30) {
        let predicate = NSPredicate(format: "label == %@", label)
        let expectation = XCTNSPredicateExpectation(predicate: predicate, object: element)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation], timeout: timeout), .completed, "expected “\(label)”")
    }

    func testBrowseSearchFilterAndSelect() {
        let app = launch()
        XCTAssertTrue(app.navigationBars["Sample"].waitForExistence(timeout: 30))
        element("place.all", in: app).tap()

        let count = app.staticTexts["browse.count"]
        XCTAssertTrue(count.waitForExistence(timeout: 15))
        waitForLabel("12 items", of: count)
        XCTAssertTrue(element("item.sample-3", in: app).waitForExistence(timeout: 10))

        let search = app.searchFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: 5))
        search.tap()
        search.typeText("sample-1")
        // sample-1, sample-10 and sample-11.
        waitForLabel("3 items", of: count)
        // Clear the field by keys: the search bar's Cancel button is gone in newer iOS.
        search.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: "sample-1".count))
        waitForLabel("12 items", of: count)
        // Leave the search, so the toolbar comes back: Cancel before iOS 26, a close button since.
        for label in ["Cancel", "close", "Close"] where app.buttons[label].exists {
            app.buttons[label].tap()
            break
        }

        app.buttons["browse.filter"].tap()
        let red = element("filter.tag.Red", in: app)
        XCTAssertTrue(red.waitForExistence(timeout: 10))
        red.tap()
        app.buttons["filter.done"].tap()
        waitForLabel("4 items", of: count)

        app.buttons["browse.select"].tap()
        app.buttons["browse.selectAll"].tap()
        let selected = app.staticTexts["browse.selectedCount"]
        XCTAssertTrue(selected.waitForExistence(timeout: 5))
        waitForLabel("4 selected", of: selected)
        app.buttons["browse.deselectAll"].tap()
        waitForLabel("0 selected", of: selected)
        app.buttons["browse.done"].tap()
        XCTAssertFalse(selected.exists)

        app.buttons["browse.options"].tap()
        app.buttons["List"].tap()
        XCTAssertTrue(element("browse.list", in: app).waitForExistence(timeout: 5))
        XCTAssertTrue(element("item.sample-0", in: app).waitForExistence(timeout: 10))
    }

    func testTheAddMenuOffersPhotosFilesAndPaste() {
        let app = launch()
        XCTAssertTrue(app.navigationBars["Sample"].waitForExistence(timeout: 30))
        element("place.all", in: app).tap()
        let menu = app.buttons["add.menu"]
        XCTAssertTrue(menu.waitForExistence(timeout: 10))
        menu.tap()
        XCTAssertTrue(element("add.photos", in: app).waitForExistence(timeout: 5))
        XCTAssertTrue(element("add.files", in: app).exists)
        XCTAssertTrue(element("add.paste", in: app).exists)
    }
}
