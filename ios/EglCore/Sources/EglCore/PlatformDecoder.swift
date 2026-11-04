// HEIC and other formats this build of the core cannot read itself, decoded by ImageIO for
// the core's thumbnails (D-096). The core calls back on its import threads; ImageIO is
// thread-safe, so the callback needs no lock.

import CEglLibrary
import CoreGraphics
import Foundation
import ImageIO

public enum PlatformDecoder {
    /// The pixels handed to the core are at most this many on their long side: the thumbnail
    /// is far smaller, and a 48-megapixel photo would otherwise take 190 MB.
    public static let maxPixels = 2048

    /// Lends ImageIO to the core, for the whole process. Call once, at launch.
    public static func install() {
        egl_set_platform_decoder({ _, bytes, length, _, sink in
            guard let bytes, let sink else { return 0 }
            let data = Data(bytesNoCopy: UnsafeMutableRawPointer(mutating: bytes), count: length, deallocator: .none)
            guard let image = PlatformDecoder.decode(data) else { return 0 }
            image.rgba.withUnsafeBufferPointer { buffer in
                egl_decode_sink_put(
                    sink, UInt32(image.width), UInt32(image.height),
                    UInt32(image.pixelWidth), UInt32(image.pixelHeight),
                    buffer.baseAddress, buffer.count
                )
            }
            return 1
        }, nil)
    }

    public static func uninstall() {
        egl_set_platform_decoder(nil, nil)
    }

    /// An image decoded for the core: its displayed size and RGBA pixels at most `maxPixels`.
    public struct Image: Equatable {
        public var width: Int
        public var height: Int
        public var pixelWidth: Int
        public var pixelHeight: Int
        public var rgba: [UInt8]
    }

    /// Decodes `data` with its orientation applied; nil when ImageIO cannot read it.
    public static func decode(_ data: Data) -> Image? {
        guard let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              CGImageSourceGetCount(source) > 0,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let rawWidth = properties[kCGImagePropertyPixelWidth] as? Int,
              let rawHeight = properties[kCGImagePropertyPixelHeight] as? Int,
              rawWidth > 0, rawHeight > 0
        else { return nil }
        // Orientations 5 to 8 turn the image a quarter: the displayed size swaps.
        let orientation = properties[kCGImagePropertyOrientation] as? Int ?? 1
        let turned = (5...8).contains(orientation)
        let width = turned ? rawHeight : rawWidth
        let height = turned ? rawWidth : rawHeight
        let options = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: min(max(width, height), maxPixels),
        ] as CFDictionary
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options) else { return nil }
        let pixelWidth = image.width
        let pixelHeight = image.height
        var rgba = [UInt8](repeating: 0, count: pixelWidth * pixelHeight * 4)
        let drawn = rgba.withUnsafeMutableBytes { buffer -> Bool in
            guard let context = CGContext(
                data: buffer.baseAddress, width: pixelWidth, height: pixelHeight, bitsPerComponent: 8,
                bytesPerRow: pixelWidth * 4, space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: pixelWidth, height: pixelHeight))
            return true
        }
        guard drawn else { return nil }
        return Image(width: width, height: height, pixelWidth: pixelWidth, pixelHeight: pixelHeight, rgba: rgba)
    }
}
