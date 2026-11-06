import Foundation
import UniformTypeIdentifiers

/// Where libraries live on the phone (D-093):
/// - **On My iPhone** (`Documents/Libraries`, shown in the Files app): the default for new
///   libraries, read-write;
/// - **anywhere the user picks in Files**, kept through a security-scoped bookmark; a library
///   in iCloud Drive or another synced provider opens read-only, like a synced folder on the
///   desktop (D-021).
struct LibraryLocations {
    /// The folder new libraries go to.
    let root: URL

    init(root: URL) {
        self.root = root
    }

    /// `Documents/Libraries`, or a fresh temporary folder for UI tests (`-uiTesting`).
    static func standard(arguments: [String] = ProcessInfo.processInfo.arguments) -> LibraryLocations {
        if arguments.contains("-uiTesting") {
            let temp = FileManager.default.temporaryDirectory
                .appendingPathComponent("ui-testing-\(UUID().uuidString)", isDirectory: true)
            return LibraryLocations(root: temp.appendingPathComponent("Libraries", isDirectory: true))
        }
        let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        return LibraryLocations(root: documents.appendingPathComponent("Libraries", isDirectory: true))
    }

    func ensureRoot() throws {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    }

    /// The libraries in the default folder, by name.
    func localLibraries() -> [URL] {
        let names = (try? FileManager.default.contentsOfDirectory(
            at: root, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles]
        )) ?? []
        return names
            .filter { $0.pathExtension == "eglib" }
            .sorted { $0.lastPathComponent.localizedStandardCompare($1.lastPathComponent) == .orderedAscending }
    }

    /// `name`, or `name 2`, `name 3`… when a library of that name is already here.
    func freeName(_ name: String) -> String {
        var candidate = name
        var number = 2
        while FileManager.default.fileExists(atPath: root.appendingPathComponent("\(candidate).eglib").path) {
            candidate = "\(name) \(number)"
            number += 1
        }
        return candidate
    }

    /// Inside the app's own container: no bookmark needed, and never synced.
    func isLocal(_ url: URL) -> Bool {
        url.standardizedFileURL.path.hasPrefix(root.standardizedFileURL.path)
    }

    /// A library somewhere a sync provider manages: iCloud Drive, or a File Provider folder
    /// whose path says so. Such a library opens read-only (D-021, D-093).
    static func isSynced(_ url: URL) -> Bool {
        if let values = try? url.resourceValues(forKeys: [.isUbiquitousItemKey]),
           values.isUbiquitousItem == true {
            return true
        }
        let path = url.standardizedFileURL.path
        return path.contains("/Mobile Documents/") || path.contains("/File Provider Storage/")
    }

    /// What the Files picker may hand over: a `.eglib` package or a folder.
    static var openTypes: [UTType] {
        [UTType(importedAs: "app.eglviewer.eglib", conformingTo: .package), .folder]
    }
}

/// One library the app opened before.
struct RecentLibrary: Codable, Equatable, Identifiable {
    var name: String
    var path: String
    /// A security-scoped bookmark for a library outside the app's container.
    var bookmark: Data?
    var openedAt: Date

    var id: String { path }
}

/// The recent libraries, newest first, in `UserDefaults`.
struct RecentLibraries {
    private let defaults: UserDefaults
    private let key = "egl.recent.libraries.v2"
    static let kept = 10

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func load() -> [RecentLibrary] {
        guard let data = defaults.data(forKey: key),
              let list = try? JSONDecoder().decode([RecentLibrary].self, from: data)
        else { return [] }
        return list
    }

    /// Puts `library` first; the same path is listed once.
    @discardableResult
    func remember(_ library: RecentLibrary) -> [RecentLibrary] {
        var list = load().filter { $0.path != library.path }
        list.insert(library, at: 0)
        list = Array(list.prefix(Self.kept))
        save(list)
        return list
    }

    @discardableResult
    func forget(path: String) -> [RecentLibrary] {
        let list = load().filter { $0.path != path }
        save(list)
        return list
    }

    func clear() {
        defaults.removeObject(forKey: key)
    }

    private func save(_ list: [RecentLibrary]) {
        if let data = try? JSONEncoder().encode(list) {
            defaults.set(data, forKey: key)
        }
    }
}
