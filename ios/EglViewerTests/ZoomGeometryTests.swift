@testable import EglViewer
import XCTest

/// D-129: where the zoom happens and how far a zoomed photo moves.
final class ZoomGeometryTests: XCTestCase {
    private let box = CGSize(width: 400, height: 800)
    /// A landscape photo: fitted 400 × 300, centred.
    private let photo = CGSize(width: 4000, height: 3000)

    func testAFittedPhotoCannotMoveAndAZoomedOneStopsAtItsEdges() {
        XCTAssertEqual(ZoomGeometry.fitted(photo, in: box), CGSize(width: 400, height: 300))
        XCTAssertEqual(ZoomGeometry.limit(image: photo, box: box, scale: 1), .zero)
        // At 2× it is 800 × 600: 200 pt each side horizontally, still shorter than the box.
        XCTAssertEqual(ZoomGeometry.limit(image: photo, box: box, scale: 2), CGSize(width: 200, height: 0))
        XCTAssertEqual(
            ZoomGeometry.clamp(CGSize(width: -350, height: 90), to: CGSize(width: 200, height: 0)),
            CGSize(width: -200, height: 0)
        )
    }

    func testTheZoomKeepsThePointUnderTheFingers() {
        // A double tap at the photo's right edge zooms there, not at the centre.
        let point = CGPoint(x: 380, y: 400)
        let offset = ZoomGeometry.anchored(offset: .zero, at: point, box: box, from: 1, to: 2.5)
        // The image point under the tap was 180 pt right of centre; at 2.5× it would be at 450,
        // so the image moves 270 pt left to keep it at 180.
        XCTAssertEqual(offset.width, -270, accuracy: 0.001)
        XCTAssertEqual(offset.height, 0, accuracy: 0.001)
        // Zooming at the centre does not move it.
        let centre = ZoomGeometry.anchored(offset: .zero, at: CGPoint(x: 200, y: 400), box: box, from: 1, to: 3)
        XCTAssertEqual(centre, .zero)
    }

    func testPastTheEdgeAPanPullsAndFarPastItLeavesTheZoom() {
        let limit = CGSize(width: 200, height: 0)
        let banded = ZoomGeometry.banded(CGSize(width: 300, height: 0), to: limit)
        XCTAssertEqual(banded.width, 200 + 100 * ZoomGeometry.rubber, accuracy: 0.001)
        XCTAssertEqual(ZoomGeometry.banded(CGSize(width: -150, height: 0), to: limit).width, -150)
        XCTAssertEqual(ZoomGeometry.overshoot(CGSize(width: 260, height: 0), limit: limit), 60)
        XCTAssertEqual(ZoomGeometry.overshoot(CGSize(width: -120, height: 0), limit: limit), 0)
    }
}
