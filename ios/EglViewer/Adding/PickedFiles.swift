import CoreTransferable
import Foundation
import UIKit
import UniformTypeIdentifiers

/// A file handed over by Photos, a drop or the clipboard, copied into a folder of its own in
/// the app's temporary directory (D-096). The core copies it into the library; the folder goes
/// when the import ends.
struct PickedFile: Transferable, Sendable {
    let url: URL

    static var transferRepresentation: some TransferRepresentation {
        FileRepresentation(importedContentType: .item) { received in
            try PickedFile(copying: received.file)
        }
    }

    /// Copies `file` under its own name (`fallback` when it has none worth keeping).
    init(copying file: URL, named fallback: String? = nil) throws {
        let folder = Self.newFolder()
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let name = fallback ?? file.lastPathComponent
        let target = folder.appendingPathComponent(name.isEmpty ? "Imported" : name)
        try FileManager.default.copyItem(at: file, to: target)
        url = target
    }

    /// Writes `data` as `name`.
    init(writing data: Data, named name: String) throws {
        let folder = Self.newFolder()
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let target = folder.appendingPathComponent(name)
        try data.write(to: target)
        url = target
    }

    /// Removes the temporary copy.
    func remove() {
        try? FileManager.default.removeItem(at: url.deletingLastPathComponent())
    }

    static let folderPrefix = "picked-"

    private static func newFolder() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("\(folderPrefix)\(UUID().uuidString)", isDirectory: true)
    }

    /// Copies left over by a crash or a kill: removed at launch.
    static func removeLeftovers() {
        let temp = FileManager.default.temporaryDirectory
        let names = (try? FileManager.default.contentsOfDirectory(atPath: temp.path)) ?? []
        for name in names where name.hasPrefix(folderPrefix) {
            try? FileManager.default.removeItem(at: temp.appendingPathComponent(name))
        }
    }
}

/// What the clipboard holds that can be added: files and images, by their best type.
enum Clipboard {
    /// True when the clipboard has an image or a file (does not read it, so no paste prompt).
    static var hasSomething: Bool {
        UIPasteboard.general.hasImages || UIPasteboard.general.hasURLs
            || UIPasteboard.general.contains(pasteboardTypes: [UTType.image.identifier, UTType.movie.identifier])
    }

    /// Copies what the clipboard holds into temporary files.
    static func files() async -> [PickedFile] {
        var files: [PickedFile] = []
        for (index, provider) in UIPasteboard.general.itemProviders.enumerated() {
            guard let type = bestType(of: provider) else { continue }
            let name = fileName(suggested: provider.suggestedName, type: type, index: index)
            if let file = try? await load(provider, type: type, name: name) {
                files.append(file)
            }
        }
        return files
    }

    /// The first image or movie type the provider offers, the original before conversions.
    static func bestType(of provider: NSItemProvider) -> UTType? {
        provider.registeredContentTypes.first { $0.conforms(to: .image) || $0.conforms(to: .movie) }
    }

    /// `name.ext`: the suggested name, or `Pasted`/`Pasted 2` …, with the type's extension.
    static func fileName(suggested: String?, type: UTType, index: Int) -> String {
        let base = suggested?.trimmingCharacters(in: .whitespacesAndNewlines)
        let stem = (base?.isEmpty == false ? base! : (index == 0 ? "Pasted" : "Pasted \(index + 1)"))
            .replacingOccurrences(of: "/", with: "-")
        guard let ext = type.preferredFilenameExtension else { return stem }
        return stem.lowercased().hasSuffix(".\(ext.lowercased())") ? stem : "\(stem).\(ext)"
    }

    private static func load(_ provider: NSItemProvider, type: UTType, name: String) async throws -> PickedFile {
        try await withCheckedThrowingContinuation { continuation in
            _ = provider.loadFileRepresentation(forTypeIdentifier: type.identifier) { url, error in
                // The file exists only during the handler: copy it now.
                if let url {
                    continuation.resume(with: Result(catching: { try PickedFile(copying: url, named: name) }))
                } else {
                    continuation.resume(throwing: error ?? CocoaError(.fileNoSuchFile))
                }
            }
        }
    }
}
