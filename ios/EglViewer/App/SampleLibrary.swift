#if DEBUG
import EglCore
import Foundation
import UIKit

/// Generated images for tests and UI tests (`-sampleLibrary N`): PNGs of their own colour,
/// so none is a duplicate, named `sample-0` … `sample-<N-1>`, the first four tagged `Red`.
enum SampleLibrary {
    static let name = "Sample"
    static let tag = "Red"
    static let tagged = 4

    /// N from `-sampleLibrary N` among the launch arguments.
    static func requested(in arguments: [String]) -> Int? {
        guard let index = arguments.firstIndex(of: "-sampleLibrary"), index + 1 < arguments.count else {
            return nil
        }
        return Int(arguments[index + 1])
    }

    /// Writes `count` PNGs into a new temporary folder; wide, tall and square in turn.
    static func images(_ count: Int) throws -> [URL] {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("sample-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        return try (0..<count).map { index in
            let sizes = [CGSize(width: 64, height: 40), CGSize(width: 40, height: 64), CGSize(width: 48, height: 48)]
            let size = sizes[index % sizes.count]
            let colour = UIColor(
                hue: CGFloat(index % 24) / 24,
                saturation: 0.4 + CGFloat(index % 5) / 10,
                brightness: 0.5 + CGFloat(index % 3) / 6,
                alpha: 1
            )
            let data = UIGraphicsImageRenderer(size: size, format: format).pngData { context in
                colour.setFill()
                context.fill(CGRect(origin: .zero, size: size))
                UIColor(white: CGFloat(index % 7) / 7, alpha: 1).setFill()
                context.fill(CGRect(x: 0, y: 0, width: CGFloat(index % 13 + 1), height: 4))
            }
            let url = folder.appendingPathComponent("sample-\(index).png")
            try data.write(to: url)
            return url
        }
    }

    /// Adds `count` images to `library`, waits for the import and tags the first few.
    static func fill(_ library: Library, images count: Int) async throws {
        guard count > 0 else { return }
        try await Task.detached {
            let files = try SampleLibrary.images(count)
            let ticket = try library.addFiles(files)
            let deadline = Date().addingTimeInterval(60)
            while true {
                if let progress = try library.importStatus(ticket.id), progress.isFinished {
                    break
                }
                if Date() > deadline { throw CancellationError() }
                Thread.sleep(forTimeInterval: 0.05)
            }
            try? FileManager.default.removeItem(at: files[0].deletingLastPathComponent())
            let view = try library.openView(LibraryQuery(sort: .name, ascending: true))
            defer { library.closeView(view.viewId) }
            let names = try library.range(of: view.viewId, start: 0, count: count)
            let first = names
                .filter { (Int($0.name.dropFirst("sample-".count)) ?? .max) < SampleLibrary.tagged }
                .map(\.id)
            if !first.isEmpty {
                try library.edit(ItemsTarget(ids: first), .addTags([SampleLibrary.tag]))
            }
        }.value
    }
}
#endif
