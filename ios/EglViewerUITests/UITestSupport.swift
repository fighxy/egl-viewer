import XCTest

extension XCUIApplication {
    /// Opens All on a generated library once its `images` are in, and returns the count label.
    ///
    /// The sample library is still importing when its screen appears, and each import burst
    /// redraws the list of places: a tap that lands during a redraw on a busy simulator can
    /// be lost (BrowseUITests failed so on CI, 19 s just to find the row). So the tap waits
    /// for the All row to show the full count, and is repeated once if no browse screen came.
    @discardableResult
    func openAll(expecting images: Int) -> XCUIElement {
        let all = descendants(matching: .any).matching(identifier: "place.all").firstMatch
        XCTAssertTrue(all.waitForExistence(timeout: 30), "the All row")
        let full = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label CONTAINS %@", "\(images)"),
            object: all
        )
        // The row's label carries its count; without it the retry below still covers us.
        _ = XCTWaiter.wait(for: [full], timeout: 60)
        let count = staticTexts["browse.count"]
        all.tap()
        if !count.waitForExistence(timeout: 15) {
            all.tap()
            XCTAssertTrue(count.waitForExistence(timeout: 15), "the browse screen of All")
        }
        return count
    }
}
