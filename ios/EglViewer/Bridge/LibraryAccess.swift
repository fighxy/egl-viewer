import Foundation
import UniformTypeIdentifiers

/// Persisted recent `.eglib` libraries via security-scoped bookmarks (D-085).
@MainActor
final class LibraryAccess: ObservableObject {
    static let shared = LibraryAccess()

    @Published private(set) var recents: [RecentLibrary] = []

    private let defaultsKey = "egl.recent.bookmarks.v1"
    private var bookmarkByPath: [String: Data] = [:]
    private var activeScopedURLs: [URL] = []

    init() {
        reloadFromDefaults()
    }

    /// UTI for the native library package (folder with `.eglib` extension).
    static var eglibType: UTType {
        UTType(exportedAs: "app.eglviewer.eglib")
    }

    static var openContentTypes: [UTType] {
        [eglibType, .folder, .directory]
    }

    func reloadFromDefaults() {
        guard let raw = UserDefaults.standard.array(forKey: defaultsKey) as? [[String: Any]] else {
            recents = []
            bookmarkByPath = [:]
            return
        }
        var nextBookmarks: [String: Data] = [:]
        var next: [RecentLibrary] = []
        for entry in raw {
            guard let name = entry["name"] as? String,
                  let path = entry["path"] as? String,
                  let hint = entry["locationHint"] as? String
            else { continue }
            if let b64 = entry["bookmark"] as? String, let data = Data(base64Encoded: b64) {
                nextBookmarks[path] = data
            }
            let missing = !FileManager.default.fileExists(atPath: path)
            next.append(
                RecentLibrary(
                    name: name,
                    path: path,
                    locationHint: hint,
                    missing: missing
                )
            )
        }
        bookmarkByPath = nextBookmarks
        recents = next
    }

    #if os(macOS)
    private static let resolveOptions: URL.BookmarkResolutionOptions = [.withoutUI, .withSecurityScope]
    private static let createOptions: URL.BookmarkCreationOptions = .withSecurityScope
    #else
    // iOS bookmarks to picked files carry their scope implicitly.
    private static let resolveOptions: URL.BookmarkResolutionOptions = [.withoutUI]
    private static let createOptions: URL.BookmarkCreationOptions = []
    #endif

    /// Starts security-scoped access when a bookmark exists; returns the path to open.
    func resolveForOpen(path: String) -> String {
        guard let data = bookmarkByPath[path] else { return path }
        var stale = false
        do {
            let url = try URL(
                resolvingBookmarkData: data,
                options: Self.resolveOptions,
                relativeTo: nil,
                bookmarkDataIsStale: &stale
            )
            if url.startAccessingSecurityScopedResource() {
                activeScopedURLs.append(url)
            }
            if stale {
                remember(url: url, locationHint: locationHint(for: url))
            }
            return url.path
        } catch {
            return path
        }
    }

    func stopScopedAccess() {
        for url in activeScopedURLs {
            url.stopAccessingSecurityScopedResource()
        }
        activeScopedURLs.removeAll()
    }

    func remember(url: URL, locationHint: String) {
        let path = url.path
        let name = url.lastPathComponent.replacingOccurrences(of: ".eglib", with: "")
        var bookmark: Data?
        do {
            bookmark = try url.bookmarkData(
                options: Self.createOptions,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )
        } catch {
            bookmark = try? url.bookmarkData(
                options: .minimalBookmark,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )
        }
        if let bookmark {
            bookmarkByPath[path] = bookmark
        }
        recents.removeAll { $0.path == path }
        recents.insert(
            RecentLibrary(
                name: name.isEmpty ? url.lastPathComponent : name,
                path: path,
                locationHint: locationHint,
                missing: false
            ),
            at: 0
        )
        if recents.count > 12 {
            let dropped = recents.suffix(from: 12)
            for item in dropped {
                bookmarkByPath.removeValue(forKey: item.path)
            }
            recents = Array(recents.prefix(12))
        }
        persist()
    }

    func forget(path: String) {
        bookmarkByPath.removeValue(forKey: path)
        recents.removeAll { $0.path == path }
        persist()
    }

    func locationHint(for url: URL) -> String {
        let path = url.path
        if path.contains("/Mobile Documents/") || path.contains("/iCloud") {
            return "iCloud"
        }
        if path.contains("/File Provider Storage") {
            return "Files"
        }
        return "On My iPhone"
    }

    func isSyncFolder(_ url: URL) -> Bool {
        let hint = locationHint(for: url)
        return hint == "iCloud" || hint == "Files"
    }

    private func persist() {
        let payload: [[String: Any]] = recents.map { recent in
            var entry: [String: Any] = [
                "name": recent.name,
                "path": recent.path,
                "locationHint": recent.locationHint,
            ]
            if let data = bookmarkByPath[recent.path] {
                entry["bookmark"] = data.base64EncodedString()
            }
            return entry
        }
        UserDefaults.standard.set(payload, forKey: defaultsKey)
    }
}
