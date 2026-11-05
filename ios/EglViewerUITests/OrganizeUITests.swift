import XCTest

/// Organizing on a generated library (`-sampleLibrary 6`, four images tagged Red): a new
/// folder, the tag list and a tag's items.
final class OrganizeUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = [
            "-uiTesting", "-sampleLibrary", "6", "-AppleLanguages", "(en)", "-AppleLocale", "en_US",
        ]
        app.launch()
        XCTAssertTrue(app.navigationBars["Sample"].waitForExistence(timeout: 30))
        return app
    }

    private func element(_ id: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: id).firstMatch
    }

    func testANewFolderAppearsInTheTree() {
        let app = launch()
        let new = element("folder.new", in: app)
        XCTAssertTrue(new.waitForExistence(timeout: 10))
        new.tap()
        let name = app.textFields["folder.name"]
        // The sheet can take a while on a busy simulator.
        XCTAssertTrue(name.waitForExistence(timeout: 20))
        name.tap()
        name.typeText("Refs")
        app.buttons["folder.save"].tap()
        XCTAssertTrue(element("folder.Refs", in: app).waitForExistence(timeout: 10))
    }

    func testTheTagListOpensATagsItems() {
        let app = launch()
        element("place.tags", in: app).tap()
        let red = element("tag.row.Red", in: app)
        XCTAssertTrue(red.waitForExistence(timeout: 20))
        red.tap()
        let count = app.staticTexts["browse.count"]
        XCTAssertTrue(count.waitForExistence(timeout: 10))
        let four = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label == %@", "4 items"), object: count)
        XCTAssertEqual(XCTWaiter.wait(for: [four], timeout: 20), .completed)
    }
}
