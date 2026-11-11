import EglCore
import Foundation
import ImageIO
import UIKit

/// Decodes originals for the preview at screen size (D-095, D-127): a stored original is read
/// by path, an item of an unlocked folder through the core's decrypted bytes.
///
/// - The memory it keeps is a byte budget, not a count: a 48-megapixel photo decoded at screen
///   size is ~30 MB, and six of them were enough for iOS to end the app.
/// - At most two decodes run at once. A page that scrolls away cancels its decode if nobody
///   else waits for it; two pages asking for the same original share one decode.
/// - A memory warning empties it.
final class OriginalCache: @unchecked Sendable {
    /// Decoded originals kept in memory.
    static let budget = 96 * 1024 * 1024

    private let library: Library
    private let cache = NSCache<NSString, UIImage>()
    private let lock = NSLock()
    private var running: [String: Decode] = [:]
    private let gate = DecodeGate(limit: 2)
    private var warning: NSObjectProtocol?

    /// One decode and how many callers wait for it.
    private final class Decode {
        let task: Task<UIImage?, Never>
        var waiters = 0

        init(task: Task<UIImage?, Never>) {
            self.task = task
        }
    }

    init(library: Library, budget: Int = OriginalCache.budget) {
        self.library = library
        cache.totalCostLimit = budget
        warning = NotificationCenter.default.addObserver(
            forName: UIApplication.didReceiveMemoryWarningNotification, object: nil, queue: nil
        ) { [cache] _ in
            cache.removeAllObjects()
        }
    }

    deinit {
        if let warning { NotificationCenter.default.removeObserver(warning) }
    }

    private static func key(_ id: String, _ version: Int, _ pixels: Int) -> String {
        "\(id)#\(version)#\(max(pixels, 1))"
    }

    /// The original already in memory, for the first frame of a page.
    func cached(for id: String, version: Int, pixels: Int) -> UIImage? {
        cache.object(forKey: Self.key(id, version, pixels) as NSString)
    }

    /// The original of `id` at most `pixels` on its long side; nil when ImageIO cannot read it
    /// (a video, a PDF, a format the phone does not know) or when the caller was cancelled.
    func image(for id: String, version: Int, pixels: Int) async -> UIImage? {
        let key = Self.key(id, version, pixels)
        if let image = cache.object(forKey: key as NSString) {
            return image
        }
        let decode = join(key) {
            let library = library
            let capped = max(pixels, 1)
            let gate = gate
            return Task.detached(priority: .userInitiated) {
                await gate.acquire()
                let image = Task.isCancelled ? nil : Self.decode(id, from: library, pixels: capped)
                await gate.release()
                return image
            }
        }
        let image = await withTaskCancellationHandler {
            await decode.task.value
        } onCancel: {
            self.leave(key)
        }
        if Task.isCancelled { return nil }
        lock.withLock {
            if running[key] === decode { running[key] = nil }
        }
        if let image, let cgImage = image.cgImage {
            cache.setObject(image, forKey: key as NSString, cost: cgImage.bytesPerRow * cgImage.height)
        }
        return image
    }

    /// Warms the cache for a page the user is likely to swipe to; cancel the task when it is
    /// no longer next.
    func prefetch(_ id: String, version: Int, pixels: Int) -> Task<Void, Never> {
        Task(priority: .utility) { _ = await self.image(for: id, version: version, pixels: pixels) }
    }

    func removeAll() {
        cache.removeAllObjects()
    }

    /// Decodes running now; for tests.
    var runningCount: Int { lock.withLock { running.count } }

    private func join(_ key: String, start: () -> Task<UIImage?, Never>) -> Decode {
        lock.withLock {
            let decode = running[key] ?? {
                let made = Decode(task: start())
                running[key] = made
                return made
            }()
            decode.waiters += 1
            return decode
        }
    }

    /// A caller stopped waiting; the last one cancels the decode.
    private func leave(_ key: String) {
        lock.withLock {
            guard let decode = running[key] else { return }
            decode.waiters -= 1
            if decode.waiters <= 0 {
                decode.task.cancel()
                running[key] = nil
            }
        }
    }

    private static func decode(_ id: String, from library: Library, pixels: Int) -> UIImage? {
        let options = [kCGImageSourceShouldCache: false] as CFDictionary
        let source: CGImageSource?
        if let path = try? library.originalPath(id) {
            source = CGImageSourceCreateWithURL(path as CFURL, options)
        } else if let bytes = try? library.originalBytes(id) {
            source = CGImageSourceCreateWithData(bytes as CFData, options)
        } else {
            source = nil
        }
        guard let source, CGImageSourceGetCount(source) > 0, !Task.isCancelled else { return nil }
        let thumbnail = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: pixels,
        ] as CFDictionary
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, thumbnail) else { return nil }
        return UIImage(cgImage: image)
    }
}

/// At most `limit` decodes at once, so a fast swipe does not start ten full decodes together.
actor DecodeGate {
    private let limit: Int
    private var busy = 0
    private var queue: [CheckedContinuation<Void, Never>] = []

    init(limit: Int) {
        self.limit = limit
    }

    func acquire() async {
        if busy < limit {
            busy += 1
            return
        }
        await withCheckedContinuation { queue.append($0) }
    }

    func release() {
        if queue.isEmpty {
            busy = max(busy - 1, 0)
        } else {
            queue.removeFirst().resume()
        }
    }
}
