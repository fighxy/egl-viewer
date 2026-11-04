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
                kCGImageSourceThumbnailMaxPixelSize: pixels,
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

/// An image that pinches and double-taps to zoom, in a UIScrollView. At its fitting size it
/// lets horizontal swipes through to the pager around it.
struct ZoomingImage: UIViewRepresentable {
    let image: UIImage
    /// Tells SwiftUI whether the image is zoomed in, so the pager stops paging meanwhile.
    var zoomed: Binding<Bool>
    /// A single tap (not the first of a double tap): shows or hides the preview's bars.
    var tapped: () -> Void = {}

    func makeCoordinator() -> Coordinator { Coordinator(zoomed: zoomed, tapped: tapped) }

    func makeUIView(context: Context) -> UIScrollView {
        let scroll = UIScrollView()
        scroll.delegate = context.coordinator
        scroll.minimumZoomScale = 1
        scroll.maximumZoomScale = 6
        scroll.showsHorizontalScrollIndicator = false
        scroll.showsVerticalScrollIndicator = false
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.backgroundColor = .clear
        let view = UIImageView(image: image)
        view.contentMode = .scaleAspectFit
        view.isUserInteractionEnabled = true
        scroll.addSubview(view)
        context.coordinator.imageView = view
        let doubleTap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.doubleTapped(_:)))
        doubleTap.numberOfTapsRequired = 2
        scroll.addGestureRecognizer(doubleTap)
        let singleTap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.singleTapped))
        singleTap.require(toFail: doubleTap)
        scroll.addGestureRecognizer(singleTap)
        return scroll
    }

    func updateUIView(_ scroll: UIScrollView, context: Context) {
        context.coordinator.zoomed = zoomed
        context.coordinator.tapped = tapped
        if context.coordinator.imageView?.image !== image {
            context.coordinator.imageView?.image = image
            scroll.setZoomScale(1, animated: false)
        }
        context.coordinator.layout(scroll)
    }

    final class Coordinator: NSObject, UIScrollViewDelegate {
        var zoomed: Binding<Bool>
        var tapped: () -> Void
        weak var imageView: UIImageView?

        init(zoomed: Binding<Bool>, tapped: @escaping () -> Void) {
            self.zoomed = zoomed
            self.tapped = tapped
        }

        @objc func singleTapped() {
            tapped()
        }

        /// The image view fills the scroll view at zoom 1; the image fits inside it.
        func layout(_ scroll: UIScrollView) {
            guard let imageView, scroll.zoomScale == 1 else { return }
            imageView.frame = CGRect(origin: .zero, size: scroll.bounds.size)
            scroll.contentSize = scroll.bounds.size
        }

        func viewForZooming(in scrollView: UIScrollView) -> UIView? { imageView }

        func scrollViewDidZoom(_ scrollView: UIScrollView) {
            let now = scrollView.zoomScale > 1.01
            if zoomed.wrappedValue != now {
                zoomed.wrappedValue = now
            }
        }

        @objc func doubleTapped(_ gesture: UITapGestureRecognizer) {
            guard let scroll = gesture.view as? UIScrollView else { return }
            if scroll.zoomScale > 1.01 {
                scroll.setZoomScale(1, animated: true)
            } else {
                let point = gesture.location(in: imageView)
                let size = CGSize(width: scroll.bounds.width / 2.5, height: scroll.bounds.height / 2.5)
                scroll.zoom(
                    to: CGRect(x: point.x - size.width / 2, y: point.y - size.height / 2, width: size.width, height: size.height),
                    animated: true
                )
            }
        }
    }
}
