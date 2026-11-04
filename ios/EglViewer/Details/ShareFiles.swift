import EglCore
import Foundation
import SwiftUI
import UIKit

/// Originals handed to the share sheet under their item names (D-095). A stored original is
/// linked (or copied, across volumes) into a temporary folder; an item of an unlocked folder is
/// decrypted by the core into it. The folder goes when the sheet closes.
struct ShareFiles: Identifiable, Sendable {
    /// At most this many originals go to one share sheet.
    static let limit = 100

    let id = UUID()
    let folder: URL
    let urls: [URL]

    /// Prepares the originals of `ids` (blocking: run off the main thread).
    static func prepare(_ ids: [String], from library: Library) throws -> ShareFiles {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("share-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        var used = Set<String>()
        var urls: [URL] = []
        do {
            for id in ids.prefix(limit) {
                let item = try library.item(id)
                let target = folder.appendingPathComponent(unique(fileName(item), in: &used))
                if let original = try? library.originalPath(id) {
                    do {
                        try FileManager.default.linkItem(at: original, to: target)
                    } catch {
                        try FileManager.default.copyItem(at: original, to: target)
                    }
                } else if let bytes = try library.originalBytes(id) {
                    try bytes.write(to: target, options: .completeFileProtection)
                } else {
                    continue
                }
                urls.append(target)
            }
        } catch {
            try? FileManager.default.removeItem(at: folder)
            throw error
        }
        return ShareFiles(folder: folder, urls: urls)
    }

    func clean() {
        try? FileManager.default.removeItem(at: folder)
    }

    /// `name.ext`, with characters a file name cannot hold replaced.
    static func fileName(_ item: Item) -> String {
        let name = item.name
            .replacingOccurrences(of: "/", with: "-")
            .replacingOccurrences(of: ":", with: "-")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let base = name.isEmpty ? item.id : name
        return item.ext.isEmpty ? base : "\(base).\(item.ext)"
    }

    /// `name (2).ext` when `name.ext` is taken already.
    static func unique(_ name: String, in used: inout Set<String>) -> String {
        var candidate = name
        var n = 2
        let url = URL(fileURLWithPath: name)
        let ext = url.pathExtension
        let base = url.deletingPathExtension().lastPathComponent
        while used.contains(candidate.lowercased()) {
            candidate = ext.isEmpty ? "\(base) (\(n))" : "\(base) (\(n)).\(ext)"
            n += 1
        }
        used.insert(candidate.lowercased())
        return candidate
    }
}

/// The system share sheet over prepared files.
struct ShareSheet: UIViewControllerRepresentable {
    let files: ShareFiles

    func makeUIViewController(context: Context) -> UIActivityViewController {
        let controller = UIActivityViewController(activityItems: files.urls, applicationActivities: nil)
        let files = files
        controller.completionWithItemsHandler = { _, _, _, _ in files.clean() }
        return controller
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
