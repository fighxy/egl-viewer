import EglCore
import Foundation
import ImageIO
import UIKit

/// Thumbnails decoded for the grid, kept in memory by cost (D-094). The core hands over the
/// WebP bytes of `thumbs/`; ImageIO decodes them off the main thread at the size a cell needs.
final class ThumbnailCache: @unchecked Sendable {
    private let library: Library
    private let cache = NSCache<NSString, UIImage>()

    init(library: Library, budget: Int = 64 * 1024 * 1024) {
        self.library = library
        cache.totalCostLimit = budget
    }

    /// The thumbnail of `cell` at most `pixels` wide or high; nil when it has none yet.
    func image(for cell: ItemCell, pixels: Int) async -> UIImage? {
        guard cell.hasThumbnail else { return nil }
        // A new thumbnail version is a new key, so a rebuilt thumbnail shows at once.
        let key = "\(cell.id)#\(cell.thumbVersion)#\(pixels)" as NSString
        if let image = cache.object(forKey: key) {
            return image
        }
        let library = library
        let id = cell.id
        let image = await Task.detached(priority: .userInitiated) { () -> UIImage? in
            guard let data = try? library.thumbnail(id) else { return nil }
            return ThumbnailCache.decode(data, pixels: pixels)
        }.value
        if let image, let cgImage = image.cgImage {
            cache.setObject(image, forKey: key, cost: cgImage.bytesPerRow * cgImage.height)
        }
        return image
    }

    func removeAll() {
        cache.removeAllObjects()
    }

    static func decode(_ data: Data, pixels: Int) -> UIImage? {
        let options = [kCGImageSourceShouldCache: false] as CFDictionary
        guard let source = CGImageSourceCreateWithData(data as CFData, options) else { return nil }
        let thumbnail = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: max(pixels, 1),
        ] as CFDictionary
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, thumbnail) else { return nil }
        return UIImage(cgImage: image)
    }
}
