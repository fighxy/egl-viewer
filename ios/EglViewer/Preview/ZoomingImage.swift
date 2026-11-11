import EglCore
import Foundation
import ImageIO
import SwiftUI
import UIKit

/// The arithmetic of the zoom, apart from the view so it can be tested (D-129). Offsets are
/// measured from the centre of the box; the view scales the image about its centre, then
/// moves it by the offset.
enum ZoomGeometry {
    static let maxScale: CGFloat = 6
    static let doubleTapScale: CGFloat = 2.5
    /// How far past an edge a pan may go and pull back.
    static let rubber: CGFloat = 0.35
    /// A pan this far past the left or right edge leaves the zoom, so the next swipe pages.
    static let leaveDistance: CGFloat = 60

    /// The image fitted into the box, at scale 1.
    static func fitted(_ image: CGSize, in box: CGSize) -> CGSize {
        guard image.width > 0, image.height > 0, box.width > 0, box.height > 0 else { return box }
        let ratio = min(box.width / image.width, box.height / image.height)
        return CGSize(width: image.width * ratio, height: image.height * ratio)
    }

    /// The furthest the image may move from the centre on each axis at `scale`: its edges stay
    /// at or past the box's, and an axis the image does not fill stays centred.
    static func limit(image: CGSize, box: CGSize, scale: CGFloat) -> CGSize {
        let size = fitted(image, in: box)
        return CGSize(
            width: max((size.width * scale - box.width) / 2, 0),
            height: max((size.height * scale - box.height) / 2, 0)
        )
    }

    static func clamp(_ offset: CGSize, to limit: CGSize) -> CGSize {
        CGSize(
            width: min(max(offset.width, -limit.width), limit.width),
            height: min(max(offset.height, -limit.height), limit.height)
        )
    }

    /// Past the limit a pan moves at a third of the finger's speed, so the edge is felt.
    static func banded(_ offset: CGSize, to limit: CGSize) -> CGSize {
        func band(_ value: CGFloat, _ limit: CGFloat) -> CGFloat {
            let over = abs(value) - limit
            guard over > 0 else { return value }
            return (limit + over * rubber) * (value < 0 ? -1 : 1)
        }
        return CGSize(width: band(offset.width, limit.width), height: band(offset.height, limit.height))
    }

    /// The offset that keeps the image point under `point` (in the box) where it is while the
    /// scale goes from `from` to `to`: zooming happens where the fingers or the tap are.
    static func anchored(offset: CGSize, at point: CGPoint, box: CGSize, from: CGFloat, to: CGFloat) -> CGSize {
        guard from > 0 else { return offset }
        let x = point.x - box.width / 2
        let y = point.y - box.height / 2
        let ratio = to / from
        return CGSize(width: x - (x - offset.width) * ratio, height: y - (y - offset.height) * ratio)
    }

    /// How far a pan went past the left or right edge (positive either way).
    static func overshoot(_ offset: CGSize, limit: CGSize) -> CGFloat {
        max(abs(offset.width) - limit.width, 0)
    }
}

/// Pinch and double-tap zoom over a decoded original (D-095, D-129). Pure SwiftUI so a nested
/// UIScrollView cannot sit at zero bounds (black stage) or swallow the pager's horizontal swipes
/// while fitting. The zoom happens where the fingers or the tap are; a zoomed image pans within
/// its edges and pulls back past them; a pan well past the left or right edge leaves the zoom,
/// so the next swipe pages.
struct ZoomingImage: View {
    let image: UIImage
    /// VoiceOver name for the photo (D-100 / D-123); the SwiftUI `Image` needs a description.
    var label: String = ""
    /// Tells SwiftUI whether the image is zoomed in, so the pager stops paging meanwhile.
    @Binding var zoomed: Bool
    /// A single tap (not the first of a double tap): shows or hides the preview's bars.
    var tapped: () -> Void = {}

    /// The settled scale and offset; a gesture works from them.
    @State private var scale: CGFloat = 1
    @State private var offset: CGSize = .zero
    /// What the gesture shows while it runs.
    @State private var liveScale: CGFloat?
    @State private var liveOffset: CGSize?

    var body: some View {
        GeometryReader { geo in
            let box = geo.size
            Image(uiImage: image)
                .resizable()
                .scaledToFit()
                .scaleEffect(liveScale ?? scale)
                .offset(liveOffset ?? offset)
                .frame(width: box.width, height: box.height)
                .contentShape(Rectangle())
                .gesture(magnify(box))
                // Pan is masked off at fit size so the pager keeps horizontal swipes.
                .simultaneousGesture(pan(box), including: zoomed ? .all : .none)
                .onTapGesture(count: 2, coordinateSpace: .local) { location in
                    doubleTap(at: location, box: box)
                }
                .onTapGesture(count: 1) {
                    tapped()
                }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: label))
        .accessibilityAddTraits(.isImage)
        .accessibilityIdentifier("preview.zoom")
    }

    private func limit(_ box: CGSize, _ scale: CGFloat) -> CGSize {
        ZoomGeometry.limit(image: image.size, box: box, scale: scale)
    }

    private func magnify(_ box: CGSize) -> some Gesture {
        MagnifyGesture()
            .onChanged { value in
                let to = min(max(scale * value.magnification, 1), ZoomGeometry.maxScale * 1.15)
                liveScale = to
                liveOffset = ZoomGeometry.anchored(
                    offset: offset, at: value.startLocation, box: box, from: scale, to: to
                )
                publishZoomed(to)
            }
            .onEnded { _ in
                let to = min(liveScale ?? scale, ZoomGeometry.maxScale)
                let moved = liveOffset ?? offset
                withAnimation(.spring(duration: 0.3)) {
                    liveScale = nil
                    liveOffset = nil
                    if to <= 1.01 {
                        settle(scale: 1, offset: .zero)
                    } else {
                        settle(scale: to, offset: ZoomGeometry.clamp(moved, to: limit(box, to)))
                    }
                }
            }
    }

    private func pan(_ box: CGSize) -> some Gesture {
        DragGesture(minimumDistance: 8)
            .onChanged { value in
                let moved = CGSize(
                    width: offset.width + value.translation.width,
                    height: offset.height + value.translation.height
                )
                liveOffset = ZoomGeometry.banded(moved, to: limit(box, scale))
            }
            .onEnded { value in
                let moved = CGSize(
                    width: offset.width + value.translation.width,
                    height: offset.height + value.translation.height
                )
                let bound = limit(box, scale)
                withAnimation(.spring(duration: 0.3)) {
                    liveOffset = nil
                    if ZoomGeometry.overshoot(ZoomGeometry.banded(moved, to: bound), limit: bound)
                        > ZoomGeometry.leaveDistance * ZoomGeometry.rubber
                    {
                        // Well past the side: back to fit, and the pager takes the next swipe.
                        settle(scale: 1, offset: .zero)
                    } else {
                        settle(scale: scale, offset: ZoomGeometry.clamp(moved, to: bound))
                    }
                }
            }
    }

    private func doubleTap(at point: CGPoint, box: CGSize) {
        withAnimation(.easeInOut(duration: 0.22)) {
            if scale > 1.01 {
                settle(scale: 1, offset: .zero)
            } else {
                let to = ZoomGeometry.doubleTapScale
                let anchored = ZoomGeometry.anchored(offset: .zero, at: point, box: box, from: 1, to: to)
                settle(scale: to, offset: ZoomGeometry.clamp(anchored, to: limit(box, to)))
            }
        }
    }

    private func settle(scale: CGFloat, offset: CGSize) {
        self.scale = scale
        self.offset = offset
        publishZoomed(scale)
    }

    private func publishZoomed(_ scale: CGFloat) {
        let now = scale > 1.01
        if zoomed != now {
            zoomed = now
        }
    }
}
