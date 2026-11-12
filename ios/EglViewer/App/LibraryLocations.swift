import Foundation
import UniformTypeIdentifiers

/// Where libraries live on the phone (D-093, D-132):
/// - **the app's folder** (`Documents/Libraries`, shown in Files under On My iPhone → EglViewer):
///   the default for new libraries, read-write, but iOS removes it with the app;
/// - **a folder the user picks in Files** (On My iPhone outside the app, a connected drive),
///   kept through a security-scoped bookmark; it outlives the app;
/// - a library in iCloud Drive, or in a provider folder the user has not said is on the device,
///   opens read-only, like a synced folder on the desktop (D-021).
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
        let path = Self.resolvedPath(url)
        let root = Self.resolvedPath(root)
        return path == root || path.hasPrefix(root + "/")
    }

    /// The path with symlinks resolved: `/var` and `/private/var` are one folder.
    static func resolvedPath(_ url: URL) -> String {
        url.standardizedFileURL.resolvingSymlinksInPath().path
    }

    /// Where a library or folder is, as far as the app can tell (D-132).
    enum Storage: Equatable {
        /// The app's own folder: removed with the app.
        case app
        /// iCloud Drive: synced; never written.
        case cloud
        /// A File Provider folder: On My iPhone outside the app and the folders of Dropbox,
        /// Google Drive and the like share this kind of path, so only the user can say which.
        case provider
        /// Elsewhere on the device, such as a connected drive.
        case device
    }

    func storage(of url: URL) -> Storage {
        if isLocal(url) { return .app }
        if Self.isICloud(url) { return .cloud }
        if url.standardizedFileURL.path.contains("/File Provider Storage/") { return .provider }
        return .device
    }

    /// iCloud Drive, by its resource value or its path.
    static func isICloud(_ url: URL) -> Bool {
        if let values = try? url.resourceValues(forKeys: [.isUbiquitousItemKey]),
           values.isUbiquitousItem == true {
            return true
        }
        return url.standardizedFileURL.path.contains("/Mobile Documents/")
    }

    /// Opens read-only, as a synced folder does (D-021): iCloud always, a provider folder unless
    /// the user said it is on this iPhone or a drive (`onDevice`).
    func opensReadOnly(_ url: URL, onDevice: Bool?) -> Bool {
        switch storage(of: url) {
        case .app, .device: false
        case .cloud: true
        case .provider: onDevice != true
        }
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
    /// The user said its folder is on this iPhone or a drive, not synced (D-132); `nil` when
    /// nobody asked. Absent in lists saved before.
    var onDevice: Bool? = nil

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

/// One library on Welcome: a recent one, or one in On My iPhone the app has not opened yet.
struct WelcomeEntry: Identifiable, Equatable {
    enum Source: Equatable {
        case recent(RecentLibrary)
        case local(URL)
    }

    let source: Source
    let name: String
    let path: String
    /// The folder that holds it, or `nil` for On My iPhone.
    let folder: String?
    /// When the app last opened it; `nil` for one it has not opened.
    let openedAt: Date?

    var id: String { path }
}

/// What Welcome lists above its actions (D-130): the recent libraries, newest first, then the
/// libraries in On My iPhone that are not among them, by name.
struct WelcomeEntries: Equatable {
    let recent: [WelcomeEntry]
    let local: [WelcomeEntry]

    var isEmpty: Bool { recent.isEmpty && local.isEmpty }

    init(recents: [RecentLibrary], local: [URL], locations: LibraryLocations) {
        // Paths compare with symlinks resolved: `/var` and `/private/var` are one folder.
        let root = Self.resolved(locations.root) + "/"
        recent = recents.map { library in
            let url = URL(fileURLWithPath: library.path)
            return WelcomeEntry(
                source: .recent(library),
                name: library.name,
                path: library.path,
                folder: Self.resolved(url).hasPrefix(root) ? nil : url.deletingLastPathComponent().lastPathComponent,
                openedAt: library.openedAt
            )
        }
        let seen = Set(recents.map { Self.resolved(URL(fileURLWithPath: $0.path)) })
        self.local = local
            .filter { !seen.contains(Self.resolved($0)) }
            .map { url in
                WelcomeEntry(
                    source: .local(url),
                    name: url.deletingPathExtension().lastPathComponent,
                    path: url.path,
                    folder: nil,
                    openedAt: nil
                )
            }
    }

    private static func resolved(_ url: URL) -> String {
        url.standardizedFileURL.resolvingSymlinksInPath().path
    }
}
