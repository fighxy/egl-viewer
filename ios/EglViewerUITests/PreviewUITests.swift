import XCTest

/// The preview, the details and actions on a selection, on a generated library
/// (`-sampleLibrary 12`).
final class PreviewUITests: XCTestCase {
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

    private func element(_ id: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: id).firstMatch
    }

    private func waitForLabel(_ label: String, of element: XCUIElement, timeout: TimeInterval = 30) {
        let expectation = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label == %@", label), object: element)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation], timeout: timeout), .completed, "expected “\(label)”")
    }

    /// Opens All once the twelve images are in.
    private func openAll(_ app: XCUIApplication) -> XCUIElement {
        XCTAssertTrue(app.navigationBars["Sample"].waitForExistence(timeout: 30))
        element("place.all", in: app).tap()
        let count = app.staticTexts["browse.count"]
        XCTAssertTrue(count.waitForExistence(timeout: 15))
        waitForLabel("12 items", of: count)
        return count
    }

    func testPreviewSwipesRatesAndTrashesWithUndo() {
        let app = launch()
        let count = openAll(app)
        // The first tile of the grid, whichever image the sort puts there.
        let first = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH 'item.sample-'"))
            .element(boundBy: 0)
        XCTAssertTrue(first.waitForExistence(timeout: 10))
        first.tap()

        XCTAssertTrue(app.buttons["preview.close"].waitForExistence(timeout: 10), "the preview opens")
        let position = element("preview.position", in: app)
        XCTAssertTrue(position.waitForExistence(timeout: 10))
        waitForLabel("1 of 12", of: position)
        element("preview.pager", in: app).swipeLeft()
        waitForLabel("2 of 12", of: position)

        app.buttons["preview.info"].tap()
        let star = element("details.star.3", in: app)
        XCTAssertTrue(star.waitForExistence(timeout: 10))
        star.tap()
        let selected = XCTNSPredicateExpectation(predicate: NSPredicate(format: "isSelected == true"), object: star)
        XCTAssertEqual(XCTWaiter.wait(for: [selected], timeout: 10), .completed)
        app.buttons["details.done"].tap()

        app.buttons["preview.trash"].tap()
        waitForLabel("2 of 11", of: position)
        let undo = app.buttons["toast.undo"]
        XCTAssertTrue(undo.waitForExistence(timeout: 5))
        undo.tap()
        waitForLabel("2 of 12", of: position)

        app.buttons["preview.close"].tap()
        waitForLabel("12 items", of: count)
    }

    func testASelectionGoesToTheTrashAndComesBack() {
        let app = launch()
        let count = openAll(app)
        XCTAssertTrue(element("item.sample-0", in: app).waitForExistence(timeout: 10))

        app.buttons["browse.select"].tap()
        element("item.sample-0", in: app).tap()
        element("item.sample-1", in: app).tap()
        waitForLabel("2 selected", of: app.staticTexts["browse.selectedCount"])
        app.buttons["selection.trash"].tap()
        waitForLabel("10 items", of: count)

        app.buttons["toast.undo"].tap()
        waitForLabel("12 items", of: count)
    }
}
