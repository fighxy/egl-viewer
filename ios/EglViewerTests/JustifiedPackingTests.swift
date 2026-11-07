@testable import EglViewer
import XCTest

/// Pure packing for the justified browse grid (D-122), mirroring desktop `justified.test.ts`.
final class JustifiedPackingTests: XCTestCase {
    func testMissingSizesAreSquare() {
        XCTAssertEqual(JustifiedPacking.aspectOf(width: 0, height: 0), 1)
        XCTAssertEqual(JustifiedPacking.aspectOf(width: 200, height: 100), 2)
    }

    func testFillsRowWidthWithEqualHeightCells() {
        // Landscape 2:1 at 800×160 → two per row after stretch.
        let layout = JustifiedPacking.pack(
            total: 5,
            containerWidth: 800,
            targetHeight: 160,
            showNames: false
        ) { _ in .init(width: 40, height: 20) }
        XCTAssertEqual(layout.rows.count, 3)
        XCTAssertEqual(layout.rows[0].cells.count, 2)
        XCTAssertEqual(layout.rows[1].cells.count, 2)
        XCTAssertEqual(layout.rows[2].cells.count, 1)
        let available = 800 - 2 * JustifiedPacking.padding
        let first = layout.rows[0]
        let widths = first.cells.map(\.width)
        XCTAssertEqual(widths[0] + widths[1] + JustifiedPacking.gap, available, accuracy: 0.5)
        XCTAssertEqual(first.cells[0].height, first.cells[1].height)
        XCTAssertNotEqual(first.cells[0].height, 160)
    }

    func testNameLineAddsToRowHeight() {
        let plain = JustifiedPacking.pack(
            total: 2, containerWidth: 800, targetHeight: 160, showNames: false
        ) { _ in .init(width: 1, height: 1) }
        let named = JustifiedPacking.pack(
            total: 2, containerWidth: 800, targetHeight: 160, showNames: true
        ) { _ in .init(width: 1, height: 1) }
        XCTAssertEqual(
            named.rows[0].contentHeight,
            plain.rows[0].thumbHeight + JustifiedPacking.nameHeight,
            accuracy: 0.01
        )
    }

    func testLastShortRowStaysNearTarget() {
        let layout = JustifiedPacking.pack(
            total: 1,
            containerWidth: 800,
            targetHeight: 120,
            showNames: false
        ) { _ in .init(width: 40, height: 20) }
        XCTAssertEqual(layout.rows.count, 1)
        XCTAssertEqual(layout.rows[0].thumbHeight, 120, accuracy: 0.01)
    }

    func testMixedAspectsPackDenserThanSquares() {
        // Tall + wide + square: justified should put more than one item on the first row
        // at a phone-ish width when the target height is medium.
        let sizes: [JustifiedPacking.Dimensions] = [
            .init(width: 9, height: 16),
            .init(width: 16, height: 9),
            .init(width: 1, height: 1),
            .init(width: 4, height: 3),
            .init(width: 3, height: 4),
        ]
        let layout = JustifiedPacking.pack(
            total: sizes.count,
            containerWidth: 390,
            targetHeight: 110,
            showNames: true
        ) { sizes[$0] }
        XCTAssertGreaterThan(layout.rows[0].cells.count, 1)
        XCTAssertGreaterThan(layout.maxColumns, 1)
        XCTAssertEqual(layout.rowOf.count, sizes.count)
    }
}
