import XCTest

/// Xcode's accessibility audit over the main screens on a generated library
/// (`-sampleLibrary 6`): every element VoiceOver reaches has a description (D-100).
final class AccessibilityUITests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func element(_ id: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: id).firstMatch
    }

    /// Audits the screen on show; issues of system elements (no element, or one of the
    /// keyboard or the status bar) are left to the system.
    private func audit(_ app: XCUIApplication, _ screen: String) throws {
        try app.performAccessibilityAudit(for: [.sufficientElementDescription]) { issue in
            guard let element = issue.element else { return true }
            print("accessibility (\(screen)): \(issue.compactDescription) — \(element.debugDescription)")
            return false
        }
    }

    func testTheMainScreensDescribeTheirElements() throws {
        let app = XCUIApplication()
        app.launchArguments = [
            "-uiTesting", "-sampleLibrary", "6", "-AppleLanguages", "(en)", "-AppleLocale", "en_US",
        ]
        app.launch()
        XCTAssertTrue(app.navigationBars["Sample"].waitForExistence(timeout: 30))
        try audit(app, "library")

        let count = app.openAll(expecting: 6)
        let six = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label == %@", "6 items"), object: count)
        XCTAssertEqual(XCTWaiter.wait(for: [six], timeout: 30), .completed)
        try audit(app, "browse")

        let first = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH 'item.sample-'"))
            .element(boundBy: 0)
        XCTAssertTrue(first.waitForExistence(timeout: 10))
        first.tap()
        XCTAssertTrue(app.buttons["preview.close"].waitForExistence(timeout: 10))
        try audit(app, "preview")

        app.buttons["preview.info"].tap()
        XCTAssertTrue(element("details.star.3", in: app).waitForExistence(timeout: 10))
        try audit(app, "details")
    }
}
