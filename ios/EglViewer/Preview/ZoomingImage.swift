import EglCore
import Foundation
import ImageIO
import SwiftUI
import UIKit

/// Decodes originals for the preview at screen size (D-095): a stored original is read by path,
/// an item of an unlocked folder through the core's decrypted bytes. A few stay in memory so
/// paging back and forth is instant.
final class OriginalCache: @unchecked Sendable {
    private let library: Library
    private let cache = NSCache<NSString, UIImage>()

    init(library: Library) {
        self.library = library
        cache.countLimit = 6
    }

    /// The original of `id` at most `pixels` on its long side; nil when ImageIO cannot read it
    /// (a video, a PDF, a format the phone does not know).
    func image(for id: String, version: Int, pixels: Int) async -> UIImage? {
        let key = "\(id)#\(version)#\(pixels)" as NSString
        if let image = cache.object(forKey: key) {
            return image
        }
        let library = library
        let capped = max(pixels, 1)
        let image = await Task.detached(priority: .userInitiated) { () -> UIImage? in
            let source: CGImageSource?
            if let path = try? library.originalPath(id) {
                source = CGImageSourceCreateWithURL(path as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary)
            } else if let bytes = try? library.originalBytes(id) {
                source = CGImageSourceCreateWithData(bytes as CFData, [kCGImageSourceShouldCache: false] as CFDictionary)
            } else {
                source = nil
            }
            guard let source, CGImageSourceGetCount(source) > 0 else { return nil }
            let options = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceShouldCacheImmediately: true,
                kCGImageSourceThumbnailMaxPixelSize: capped,
            ] as CFDictionary
            guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options) else { return nil }
            return UIImage(cgImage: image)
        }.value
        if let image {
            cache.setObject(image, forKey: key)
        }
        return image
    }

    func removeAll() {
        cache.removeAllObjects()
    }
}

/// Pinch / double-tap zoom over a decoded original. Pure SwiftUI so a nested UIScrollView
/// cannot sit at zero bounds (black stage) or swallow the pager's horizontal swipes while
/// fitting (D-095 follow-up).
struct ZoomingImage: View {
    let image: UIImage
    /// VoiceOver name for the photo (D-100 / D-123); the SwiftUI `Image` needs a description.
    var label: String = ""
    /// Tells SwiftUI whether the image is zoomed in, so the pager stops paging meanwhile.
    @Binding var zoomed: Bool
    /// A single tap (not the first of a double tap): shows or hides the preview's bars.
    var tapped: () -> Void = {}

    @State private var scale: CGFloat = 1
    @State private var pinchScale: CGFloat = 1
    @State private var offset: CGSize = .zero
    @State private var dragStart: CGSize = .zero

    private var shownScale: CGFloat { max(scale * pinchScale, 1) }

    var body: some View {
        GeometryReader { geo in
            Image(uiImage: image)
                .resizable()
                .scaledToFit()
                .scaleEffect(shownScale)
                .offset(offset)
                .frame(width: geo.size.width, height: geo.size.height)
                .contentShape(Rectangle())
                .gesture(magnifyGesture)
                // Pan is masked off at fit size so the pager keeps horizontal swipes.
                .simultaneousGesture(panGesture, including: zoomed ? .all : .none)
                .onTapGesture(count: 2) {
                    doubleTap()
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

    private var magnifyGesture: some Gesture {
        MagnifyGesture()
            .onChanged { value in
                pinchScale = value.magnification
                publishZoomed()
            }
            .onEnded { value in
                scale = min(max(scale * value.magnification, 1), 6)
                pinchScale = 1
                if scale <= 1.01 {
                    resetZoom()
                } else {
                    publishZoomed()
                }
            }
    }

    private var panGesture: some Gesture {
        DragGesture(minimumDistance: 8)
            .onChanged { value in
                offset = CGSize(
                    width: dragStart.width + value.translation.width,
                    height: dragStart.height + value.translation.height
                )
            }
            .onEnded { _ in
                dragStart = offset
            }
    }

    private func doubleTap() {
        if shownScale > 1.01 {
            withAnimation(.easeInOut(duration: 0.2)) { resetZoom() }
        } else {
            withAnimation(.easeInOut(duration: 0.2)) {
                scale = 2.5
                pinchScale = 1
                offset = .zero
                dragStart = .zero
                zoomed = true
            }
        }
    }

    private func resetZoom() {
        scale = 1
        pinchScale = 1
        offset = .zero
        dragStart = .zero
        if zoomed { zoomed = false }
    }

    private func publishZoomed() {
        let now = shownScale > 1.01
        if zoomed != now {
            zoomed = now
        }
    }
}
