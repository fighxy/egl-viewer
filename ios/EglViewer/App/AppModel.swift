import EglCore
import Foundation
import Observation

/// The app's state: Welcome or one open library, the recent libraries, and the last error.
/// Every call into the core runs off the main thread (D-093).
@MainActor
@Observable
final class AppModel {
    enum Screen {
        case welcome
        case library(OpenLibrary)
    }

    private(set) var screen: Screen = .welcome
    private(set) var recents: [RecentLibrary] = []
    private(set) var localLibraries: [URL] = []
    /// Shown in an alert; cleared when the user dismisses it.
    var error: String?
    private(set) var busy = false

    let locations: LibraryLocations
    private let recentStore: RecentLibraries
    let appVersion: String

    init(
        locations: LibraryLocations = .standard(),
        recentStore: RecentLibraries = RecentLibraries(),
        arguments: [String] = ProcessInfo.processInfo.arguments
    ) {
        self.locations = locations
        self.recentStore = recentStore
        appVersion = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
            ?? "0.0.0"
        if arguments.contains("-uiTesting") {
            recentStore.clear()
        }
        reloadLists()
    }

    var openLibrary: OpenLibrary? {
        if case let .library(open) = screen { return open }
        return nil
    }

    func reloadLists() {
        recents = recentStore.load()
        localLibraries = locations.localLibraries()
    }

    // MARK: Create and open

    /// Creates `name.eglib` in On My iPhone and opens it.
    func create(named name: String) async {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else {
            error = String(localized: "error.nameRequired")
            return
        }
        await run {
            try self.locations.ensureRoot()
            let root = self.locations.root
            let version = self.appVersion
            let path = try await Task.detached {
                try Library.create(in: root, name: name, appVersion: version)
            }.value
            try await self.open(url: path, scoped: false, bookmark: nil)
        }
    }

    /// Opens a library the Files picker handed over; outside the app's container it is kept
    /// through a security-scoped bookmark.
    func open(picked url: URL) async {
        await run {
            let local = self.locations.isLocal(url)
            let scoped = !local && url.startAccessingSecurityScopedResource()
            let bookmark = local ? nil : try? url.bookmarkData()
            do {
                try await self.open(url: url, scoped: scoped, bookmark: bookmark)
            } catch {
                if scoped { url.stopAccessingSecurityScopedResource() }
                throw error
            }
        }
    }

    func open(recent: RecentLibrary) async {
        await run {
            var url = URL(fileURLWithPath: recent.path)
            var scoped = false
            if let bookmark = recent.bookmark {
                var stale = false
                url = try URL(resolvingBookmarkData: bookmark, bookmarkDataIsStale: &stale)
                scoped = url.startAccessingSecurityScopedResource()
            }
            do {
                try await self.open(url: url, scoped: scoped, bookmark: recent.bookmark)
            } catch {
                if scoped { url.stopAccessingSecurityScopedResource() }
                throw error
            }
        }
    }

    func openLocal(_ url: URL) async {
        await run { try await self.open(url: url, scoped: false, bookmark: nil) }
    }

    func forget(_ recent: RecentLibrary) {
        recents = recentStore.forget(path: recent.path)
    }

    /// Back to Welcome; imports and exports stop.
    func close() async {
        guard let open = openLibrary else { return }
        screen = .welcome
        await open.close()
        reloadLists()
    }

    // MARK: Plumbing

    private func open(url: URL, scoped: Bool, bookmark: Data?) async throws {
        if let current = openLibrary {
            screen = .welcome
            await current.close()
        }
        let synced = !locations.isLocal(url) && LibraryLocations.isSynced(url)
        let version = appVersion
        let library = try await Task.detached {
            try Library.open(at: url, appVersion: version, syncFolder: synced)
        }.value
        let open = try await OpenLibrary(library: library, url: url, scoped: scoped)
        screen = .library(open)
        recents = recentStore.remember(
            RecentLibrary(name: open.info.name, path: url.path, bookmark: bookmark, openedAt: Date())
        )
    }

    private func run(_ work: @escaping () async throws -> Void) async {
        busy = true
        defer { busy = false }
        do {
            try await work()
        } catch {
            self.error = Self.describe(error)
        }
    }

    static func describe(_ error: Error) -> String {
        if let error = error as? EglError {
            switch error.code {
            case .notALibrary: return String(localized: "error.notALibrary")
            case .readOnly: return String(localized: "error.readOnly")
            case .wrongPassword: return String(localized: "error.wrongPassword")
            default: return error.message.isEmpty ? String(localized: "error.generic") : error.message
            }
        }
        return error.localizedDescription
    }
}

/// One open library: its core handle, what the app shows of it, and the events that keep it
/// current.
@MainActor
@Observable
final class OpenLibrary {
    let library: Library
    let url: URL
    private(set) var info: LibraryInfo
    private(set) var counts: LibraryCounts?
    /// Imports that run now.
    private(set) var imports: [UInt64: ImportProgress] = [:]
    private let scoped: Bool

    init(library: Library, url: URL, scoped: Bool) async throws {
        self.library = library
        self.url = url
        self.scoped = scoped
        info = try await Task.detached { try library.info() }.value
        counts = try? await Task.detached { try library.counts() }.value
        try library.setEventHandler { [weak self] event in
            Task { @MainActor in self?.receive(event) }
        }
    }

    var readOnly: Bool { info.readOnly }

    func refresh() async {
        let library = library
        if let counts = try? await Task.detached(operation: { try library.counts() }).value {
            self.counts = counts
        }
        if let info = try? await Task.detached(operation: { try library.info() }).value {
            self.info = info
        }
    }

    func close() async {
        let library = library
        await Task.detached { library.close() }.value
        if scoped { url.stopAccessingSecurityScopedResource() }
    }

    private func receive(_ event: LibraryEvent) {
        switch event {
        case let .importProgress(progress):
            if progress.isFinished {
                imports[progress.id] = nil
            } else {
                imports[progress.id] = progress
            }
            Task { await refresh() }
        case .viewsInvalidated:
            Task { await refresh() }
        case .exportProgress:
            break
        }
    }
}
