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
    /// An Eagle library picked for import, read and waiting for the user's go (D-116).
    var eagleImport: EaglePlan?
    /// A library picked in a File Provider folder: is that folder on this iPhone or synced? (D-132)
    var askingWhere: PickedLibrary?

    let locations: LibraryLocations
    private let recentStore: RecentLibraries
    private let savedViews: SavedViews
    let appVersion: String

    init(
        locations: LibraryLocations = .standard(),
        recentStore: RecentLibraries = RecentLibraries(),
        savedViews: SavedViews = SavedViews(),
        arguments: [String] = ProcessInfo.processInfo.arguments
    ) {
        self.locations = locations
        self.recentStore = recentStore
        self.savedViews = savedViews
        appVersion = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
            ?? "0.0.0"
        if arguments.contains("-uiTesting") {
            recentStore.clear()
            savedViews.clear()
        }
        // HEIC thumbnails through ImageIO (D-096); temporary copies of a killed run go.
        PlatformDecoder.install()
        PickedFile.removeLeftovers()
        reloadLists()
        #if DEBUG
        if let count = SampleLibrary.requested(in: arguments) {
            Task { await self.createSample(images: count) }
        }
        #endif
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

    /// Creates `name.eglib` and opens it: in the app's folder, or in `folder`, a folder picked in
    /// Files that outlives the app (D-132). The user picked that folder as one on this iPhone or
    /// a drive; iCloud Drive is refused, as no library is created in a synced folder.
    func create(named name: String, in folder: URL? = nil) async {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else {
            error = String(localized: "error.nameRequired")
            return
        }
        await run {
            let version = self.appVersion
            guard let folder, self.locations.storage(of: folder) != .app else {
                try self.locations.ensureRoot()
                let root = self.locations.root
                let path = try await Task.detached {
                    try Library.create(in: root, name: name, appVersion: version)
                }.value
                try await self.open(url: path, scoped: false, bookmark: nil, onDevice: nil)
                return
            }
            guard self.locations.storage(of: folder) != .cloud else { throw LocationRefused.cloud }
            let scoped = folder.startAccessingSecurityScopedResource()
            defer { if scoped { folder.stopAccessingSecurityScopedResource() } }
            let path = try await Task.detached {
                try Library.create(in: folder, name: name, appVersion: version)
            }.value
            try await self.open(kept: path, onDevice: true)
        }
    }

    /// Moves a library out of the app's folder into `folder`, picked in Files, so that it
    /// outlives the app (D-132): a copy first, opened once to check it, then the original goes.
    func move(_ library: URL, to folder: URL) async {
        await run {
            guard self.locations.isLocal(library) else { return }
            switch self.locations.storage(of: folder) {
            case .app: return
            case .cloud: throw LocationRefused.cloud
            case .provider, .device: break
            }
            let scoped = folder.startAccessingSecurityScopedResource()
            defer { if scoped { folder.stopAccessingSecurityScopedResource() } }
            let target = folder.appendingPathComponent(library.lastPathComponent, isDirectory: true)
            guard !FileManager.default.fileExists(atPath: target.path) else { throw LocationRefused.exists }
            let version = self.appVersion
            try await Task.detached {
                do {
                    try FileManager.default.copyItem(at: library, to: target)
                    try Library.open(at: target, appVersion: version).close()
                } catch {
                    try? FileManager.default.removeItem(at: target)
                    throw error
                }
                try FileManager.default.removeItem(at: library)
            }.value
            let same = LibraryLocations.resolvedPath(library)
            let previous = self.recents.filter {
                LibraryLocations.resolvedPath(URL(fileURLWithPath: $0.path)) == same
            }
            let before = previous.first
            for recent in previous { self.recents = self.recentStore.forget(path: recent.path) }
            let name = before?.name ?? library.deletingPathExtension().lastPathComponent
            self.recents = self.recentStore.remember(RecentLibrary(
                name: name,
                path: target.path,
                bookmark: try target.bookmarkData(),
                openedAt: before?.openedAt ?? Date(),
                onDevice: true
            ))
            self.reloadLists()
        }
    }

    /// Opens a library the Files picker handed over; outside the app's container it is kept
    /// through a security-scoped bookmark. In a File Provider folder the user is asked first
    /// whether that folder is on this iPhone (read-write) or synced (read-only) (D-132).
    func open(picked url: URL) async {
        if locations.storage(of: url) == .provider {
            askingWhere = PickedLibrary(url: url)
            return
        }
        await open(picked: url, onDevice: nil)
    }

    /// The answer to `askingWhere`.
    func open(picked url: URL, onDevice: Bool?) async {
        askingWhere = nil
        await run {
            let local = self.locations.isLocal(url)
            let scoped = !local && url.startAccessingSecurityScopedResource()
            let bookmark = local ? nil : try? url.bookmarkData()
            do {
                try await self.open(url: url, scoped: scoped, bookmark: bookmark, onDevice: onDevice)
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
                if recent.onDevice == nil, self.locations.storage(of: url) == .provider {
                    // Opened read-only before anyone asked (D-132): ask now.
                    self.askingWhere = PickedLibrary(url: url)
                    return
                }
                scoped = url.startAccessingSecurityScopedResource()
            }
            do {
                try await self.open(url: url, scoped: scoped, bookmark: recent.bookmark, onDevice: recent.onDevice)
            } catch {
                if scoped { url.stopAccessingSecurityScopedResource() }
                throw error
            }
        }
    }

    /// Opens a library the app just created or moved outside its folder, through a bookmark
    /// taken now, while the folder's scope is held.
    private func open(kept url: URL, onDevice: Bool) async throws {
        let bookmark = try url.bookmarkData()
        var stale = false
        let resolved = try URL(resolvingBookmarkData: bookmark, bookmarkDataIsStale: &stale)
        let scoped = resolved.startAccessingSecurityScopedResource()
        do {
            try await open(url: resolved, scoped: scoped, bookmark: bookmark, onDevice: onDevice)
        } catch {
            if scoped { resolved.stopAccessingSecurityScopedResource() }
            throw error
        }
    }

    // MARK: Importing an Eagle library (D-116)

    /// Reads what the picked Eagle library holds and asks before importing it. Its security
    /// scope is held until the import ends or the user cancels.
    func pickEagle(_ url: URL) async {
        let scoped = !locations.isLocal(url) && url.startAccessingSecurityScopedResource()
        busy = true
        defer { busy = false }
        do {
            let summary = try await Task.detached { try Library.eagleLibraryInfo(at: url) }.value
            eagleImport = EaglePlan(url: url, scoped: scoped, summary: summary)
        } catch {
            if scoped { url.stopAccessingSecurityScopedResource() }
            self.error = Self.describeEagle(error)
        }
    }

    func cancelEagle() {
        guard let plan = eagleImport else { return }
        eagleImport = nil
        if plan.scoped { plan.url.stopAccessingSecurityScopedResource() }
    }

    /// Creates `name.eglib` in On My iPhone (with a number when the name is taken), opens it
    /// and imports the Eagle library into it in the background. The Eagle folder is only read.
    func importEagle(_ plan: EaglePlan, named name: String, importTrash: Bool) async {
        eagleImport = nil
        let release = { if plan.scoped { plan.url.stopAccessingSecurityScopedResource() } }
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else {
            release()
            error = String(localized: "error.nameRequired")
            return
        }
        var started = false
        await run {
            try self.locations.ensureRoot()
            let root = self.locations.root
            let version = self.appVersion
            let unique = self.locations.freeName(name)
            let path = try await Task.detached {
                try Library.create(in: root, name: unique, appVersion: version)
            }.value
            try await self.open(url: path, scoped: false, bookmark: nil, onDevice: nil)
            guard let open = self.openLibrary else { return }
            started = true
            await open.importEagle(from: plan.url, importTrash: importTrash, cleanup: release)
        }
        if !started { release() }
    }

    static func describeEagle(_ error: Error) -> String {
        if let error = error as? EglError, error.code == .notALibrary {
            return String(localized: "eagle.notALibrary")
        }
        return describe(error)
    }

    func openLocal(_ url: URL) async {
        await run { try await self.open(url: url, scoped: false, bookmark: nil, onDevice: nil) }
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

    private func open(url: URL, scoped: Bool, bookmark: Data?, onDevice: Bool?) async throws {
        if let current = openLibrary {
            screen = .welcome
            await current.close()
        }
        let synced = locations.opensReadOnly(url, onDevice: onDevice)
        let version = appVersion
        let library = try await Task.detached {
            try Library.open(at: url, appVersion: version, syncFolder: synced)
        }.value
        let open = try await OpenLibrary(library: library, url: url, scoped: scoped, savedViews: savedViews)
        screen = .library(open)
        recents = recentStore.remember(
            RecentLibrary(
                name: open.info.name, path: url.path, bookmark: bookmark, openedAt: Date(), onDevice: onDevice
            )
        )
    }

    #if DEBUG
    /// `-sampleLibrary N` (UI tests): a library named Sample with N generated images, a few
    /// of them tagged, opened at launch.
    func createSample(images count: Int) async {
        await create(named: SampleLibrary.name)
        guard let open = openLibrary else { return }
        await run {
            try await SampleLibrary.fill(open.library, images: count)
            await open.refresh()
        }
    }
    #endif

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
        if let error = error as? LocationRefused {
            switch error {
            case .cloud: return String(localized: "error.cloudFolder")
            case .exists: return String(localized: "error.libraryExists")
            }
        }
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

/// One open library: its core handle, what the app shows of it (counts, folders, smart
/// folders, the saved view), and the events that keep it current.
@MainActor
@Observable
final class OpenLibrary {
    let library: Library
    let url: URL
    let thumbnails: ThumbnailCache
    let originals: OriginalCache
    private(set) var info: LibraryInfo
    private(set) var counts: LibraryCounts?
    private(set) var folders: [Folder] = []
    private(set) var smartFolders: [SmartFolder] = []
    private(set) var tagGroups: [TagGroup] = []
    /// Imports that run now.
    private(set) var imports: [UInt64: ImportProgress] = [:]
    /// Exports, by id, with their last progress (finished ones too, until the sheet clears them).
    private(set) var exports: [UInt64: ExportProgress] = [:]
    /// The sort, look and last place, saved per library (D-094).
    private(set) var savedView: SavedView
    /// Shown in an alert over the library; cleared when the user dismisses it.
    var error: String?
    /// What the last action did, with Undo when the core can take it back (D-095).
    private(set) var toast: Toast?
    private let scoped: Bool
    @ObservationIgnored private let savedViews: SavedViews
    @ObservationIgnored private var browsers: [WeakBrowser] = []
    @ObservationIgnored private var refreshing: Task<Void, Never>?
    @ObservationIgnored private var hidingToast: Task<Void, Never>?
    /// What to undo when an import ends: temporary copies to remove, scopes to give back.
    @ObservationIgnored private var afterImport: [UInt64: @MainActor () -> Void] = [:]
    /// The duplicate an import waits on, shown as a question.
    var duplicateQuestion: ImportProgress?
    /// How the last Eagle import ended, shown in an alert (D-116).
    var eagleOutcome: EagleOutcome?
    @ObservationIgnored private var answered: Set<UInt64> = []

    init(library: Library, url: URL, scoped: Bool, savedViews: SavedViews = SavedViews()) async throws {
        self.library = library
        self.url = url
        self.scoped = scoped
        self.savedViews = savedViews
        thumbnails = ThumbnailCache(library: library)
        originals = OriginalCache(library: library)
        savedView = savedViews.load(for: url.standardizedFileURL.path)
        info = try await Task.detached { try library.info() }.value
        try library.setEventHandler { [weak self] event in
            Task { @MainActor in self?.receive(event) }
        }
        await refresh()
    }

    var readOnly: Bool { info.readOnly }

    /// Counts, info, folders and smart folders, read again.
    func refresh() async {
        let library = library
        let read = await Task.detached { () -> (LibraryCounts?, LibraryInfo?, [Folder]?, [SmartFolder]?, [TagGroup]?) in
            (try? library.counts(), try? library.info(), try? library.folders(), try? library.smartFolders(),
             try? library.tagGroups())
        }.value
        if let counts = read.0 { self.counts = counts }
        if let info = read.1 { self.info = info }
        if let folders = read.2 { self.folders = folders }
        if let smart = read.3 { smartFolders = smart }
        if let groups = read.4 { tagGroups = groups }
    }

    func close() async {
        browsers = []
        let library = library
        await Task.detached { library.close() }.value
        // Closing stopped the imports: their copies and scopes go now.
        let cleanups = afterImport.values
        afterImport = [:]
        cleanups.forEach { $0() }
        duplicateQuestion = nil
        thumbnails.removeAll()
        originals.removeAll()
        if scoped { url.stopAccessingSecurityScopedResource() }
    }

    // MARK: Work and undo

    /// Runs `work` with the core off the main thread; a failure goes to the alert.
    func perform<T: Sendable>(_ work: @escaping @Sendable (Library) throws -> T) async -> T? {
        let library = library
        do {
            return try await Task.detached { try work(library) }.value
        } catch {
            report(error)
            return nil
        }
    }

    /// Shows what happened for a few seconds.
    func show(_ message: String, undoable: Bool) {
        let toast = Toast(message: message, undoable: undoable)
        self.toast = toast
        hidingToast?.cancel()
        hidingToast = Task {
            try? await Task.sleep(for: .seconds(5))
            guard !Task.isCancelled, self.toast == toast else { return }
            self.toast = nil
        }
    }

    func dismissToast() {
        hidingToast?.cancel()
        toast = nil
    }

    /// Takes back the last edit, as Edit > Undo does on the desktop.
    func undo() async {
        dismissToast()
        if let status = await perform({ try $0.undo() }), status != nil {
            show(String(localized: "toast.undone"), undoable: false)
        }
    }

    // MARK: Adding (D-096)

    /// Adds files and folders into `folder` (or none); `cleanup` runs when the import ends,
    /// or at once when it cannot start.
    func add(_ urls: [URL], into folder: String?, cleanup: @escaping @MainActor () -> Void = {}) async {
        guard !urls.isEmpty, !readOnly else {
            cleanup()
            return
        }
        guard let ticket = await perform({ try $0.addFiles(urls, into: folder) }) else {
            cleanup()
            return
        }
        afterImport[ticket.id] = cleanup
        // It may have ended before its last event reached this model.
        if let progress = await perform({ try $0.importStatus(ticket.id) }) ?? nil, progress.isFinished {
            finished(progress)
        }
    }

    /// Imports the Eagle library at `url` into this library (D-116); `cleanup` runs when the
    /// import ends, or at once when it cannot start. The report comes as `eagleOutcome`.
    func importEagle(from url: URL, importTrash: Bool, cleanup: @escaping @MainActor () -> Void) async {
        guard !readOnly,
              let ticket = await perform({ try $0.importEagle(at: url, importTrash: importTrash) })
        else {
            cleanup()
            return
        }
        afterImport[ticket.id] = cleanup
        if let progress = await perform({ try $0.importStatus(ticket.id) }) ?? nil, progress.isFinished {
            finished(progress)
        }
    }

    /// Adds files picked from Photos, the clipboard or a drop; their copies go afterwards.
    func add(_ files: [PickedFile], into folder: String?) async {
        await add(files.map(\.url), into: folder) {
            files.forEach { $0.remove() }
        }
    }

    /// Adds files the Files picker handed over, holding their security scopes until the
    /// import ends.
    func add(picked urls: [URL], into folder: String?) async {
        let scoped = urls.filter { $0.startAccessingSecurityScopedResource() }
        await add(urls, into: folder) {
            scoped.forEach { $0.stopAccessingSecurityScopedResource() }
        }
    }

    func answer(_ answer: DuplicateAnswer) async {
        guard let question = duplicateQuestion else { return }
        duplicateQuestion = nil
        answered.insert(question.id)
        _ = await perform { try $0.answerImport(question.id, answer) }
    }

    private func progressed(_ progress: ImportProgress) {
        if progress.isFinished {
            finished(progress)
        } else if progress.state == .waitingForDecision, progress.duplicate != nil,
                  !answered.contains(progress.id), duplicateQuestion?.id != progress.id {
            duplicateQuestion = progress
        }
    }

    private func finished(_ progress: ImportProgress) {
        if duplicateQuestion?.id == progress.id {
            duplicateQuestion = nil
        }
        answered.remove(progress.id)
        guard let cleanup = afterImport.removeValue(forKey: progress.id) else { return }
        cleanup()
        if progress.kind == .eagle, let report = progress.report {
            eagleOutcome = EagleOutcome(id: progress.id, report: report, cancelled: progress.state == .cancelled)
            return
        }
        show(Self.summary(of: progress), undoable: false)
    }

    /// `Added 3 items · 1 duplicate skipped · 1 failed`.
    static func summary(of progress: ImportProgress) -> String {
        var parts = [String(localized: "add.done \(progress.added)")]
        if progress.duplicates > 0 {
            parts.append(String(localized: "add.duplicates \(progress.duplicates)"))
        }
        if progress.failed > 0 {
            parts.append(String(localized: "add.failed \(progress.failed)"))
        }
        if progress.state == .cancelled {
            parts.append(String(localized: "add.cancelled"))
        }
        return parts.joined(separator: " · ")
    }

    // MARK: Organizing (D-097)

    /// One edit of the folder tree; false when the core refused it (the alert says why).
    @discardableResult
    func saveFolder(_ change: FolderChange) async -> Bool {
        guard let tree = await perform({ try $0.saveFolder(change) }) else { return false }
        folders = tree
        return true
    }

    func deleteFolder(_ id: String, trashItems: Bool) async {
        guard let tree = await perform({ try $0.deleteFolder(id, trashItems: trashItems) }) else { return }
        folders = tree
        forgetIfSaved(.folder(id: id, name: ""))
        await refresh()
        show(String(localized: "toast.folderDeleted"), undoable: false)
    }

    /// The new parent's position for a move: after its last child.
    func endIndex(under parent: String?) -> Int {
        guard let parent else { return folders.count }
        return Self.find(parent, in: folders)?.children.count ?? 0
    }

    /// The folder and its descendants: where it cannot move.
    func subtree(of id: String) -> Set<String> {
        guard let folder = Self.find(id, in: folders) else { return [id] }
        var ids: Set<String> = []
        func walk(_ folder: Folder) {
            ids.insert(folder.id)
            folder.children.forEach(walk)
        }
        walk(folder)
        return ids
    }

    func parent(of id: String) -> String? {
        func search(_ folders: [Folder], _ parent: String?) -> String?? {
            for folder in folders {
                if folder.id == id { return .some(parent) }
                if let found = search(folder.children, folder.id) { return found }
            }
            return nil
        }
        return search(folders, nil) ?? nil
    }

    @discardableResult
    func renameTag(_ from: String, to: String) async -> Bool {
        let to = to.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !to.isEmpty, to != from else { return false }
        guard await perform({ try $0.renameTag(from, to: to) }) != nil else { return false }
        await refresh()
        return true
    }

    @discardableResult
    func mergeTag(_ source: String, into target: String) async -> Bool {
        guard source != target, await perform({ try $0.mergeTag(source, into: target) }) != nil else { return false }
        await refresh()
        return true
    }

    func deleteTag(_ name: String) async {
        guard await perform({ try $0.deleteTag(name) }) != nil else { return }
        await refresh()
        show(String(localized: "toast.tagDeleted"), undoable: false)
    }

    @discardableResult
    func saveTagGroup(_ change: TagGroupChange) async -> Bool {
        guard let groups = await perform({ try $0.saveTagGroup(change) }) else { return false }
        tagGroups = groups
        return true
    }

    func deleteTagGroup(_ id: String) async {
        if let groups = await perform({ try $0.deleteTagGroup(id) }) {
            tagGroups = groups
        }
    }

    @discardableResult
    func saveSmartFolder(_ change: SmartFolderChange) async -> Bool {
        guard let tree = await perform({ try $0.saveSmartFolder(change) }) else { return false }
        smartFolders = tree
        return true
    }

    func deleteSmartFolder(_ id: String) async {
        guard let tree = await perform({ try $0.deleteSmartFolder(id) }) else { return }
        smartFolders = tree
        forgetIfSaved(.smart(id: id, name: ""))
        show(String(localized: "toast.smartFolderDeleted"), undoable: false)
    }

    /// The rules of a view's query, for "Save as Smart Folder".
    func smartDraft(for query: LibraryQuery) async -> SmartRulesDraft? {
        await perform { try $0.smartRules(of: query) }
    }

    func smartFolder(_ id: String) -> SmartFolder? {
        Self.find(id, in: smartFolders)
    }

    /// Smart folders in tree order with their depth, for the parent picker.
    var smartFolderList: [(folder: SmartFolder, depth: Int)] {
        var list: [(folder: SmartFolder, depth: Int)] = []
        func walk(_ folders: [SmartFolder], _ depth: Int) {
            for folder in folders {
                list.append((folder, depth))
                walk(folder.children, depth + 1)
            }
        }
        walk(smartFolders, 0)
        return list
    }

    /// A deleted folder is not the place the library opens in next time.
    private func forgetIfSaved(_ place: Place) {
        guard let saved = savedView.place, saved.key == place.key else { return }
        forgetPlace()
    }

    // MARK: Maintenance and export (D-099)

    func forgetExport(_ id: UInt64) {
        exports[id] = nil
    }

    /// Starts an export; its progress arrives as events.
    func startExport(_ request: ExportRequest) async -> ExportProgress? {
        guard let started = await perform({ try $0.startExport(request) }) else { return nil }
        if exports[started.id] == nil {
            exports[started.id] = started
        }
        return started
    }

    /// A restored backup is another database: views and undo start over, so every open
    /// screen asks for its view again and the tree is read anew.
    func restoreBackup(_ name: String) async -> Bool {
        guard let info = await perform({ try $0.restoreBackup(name) }) else { return false }
        self.info = info
        for browser in browsers {
            if let model = browser.model {
                Task { await model.reload() }
            }
        }
        thumbnails.removeAll()
        originals.removeAll()
        await refresh()
        show(String(localized: "toast.restored"), undoable: false)
        return true
    }

    /// Renames the library: its folder keeps its name, the library's own name changes.
    func rename(to name: String) async -> Bool {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, let info = await perform({ try $0.rename(to: name) }) else { return false }
        self.info = info
        return true
    }

    // MARK: Locked folders (D-098)

    /// Any folder open now: what "Lock All" and leaving the app lock again.
    var hasUnlockedFolders: Bool {
        func any(_ folders: [Folder]) -> Bool {
            folders.contains { ($0.isLocked && $0.isUnlocked) || any($0.children) }
        }
        return any(folders)
    }

    /// Runs a lock operation (Argon2id: slow, off the main thread) and reads the tree again,
    /// since a lock changes what the views show. A wrong password is reported to the caller,
    /// not the alert.
    func lockWork<T: Sendable>(_ work: @escaping @Sendable (Library) throws -> T) async -> Result<T, EglError> {
        let library = library
        let result: Result<T, Error> = await Task.detached {
            Result(catching: { try work(library) })
        }.value
        await refresh()
        switch result {
        case let .success(value):
            return .success(value)
        case let .failure(error as EglError) where error.code == .wrongPassword:
            return .failure(error)
        case let .failure(error):
            report(error)
            return .failure(error as? EglError ?? EglError(code: .other, message: error.localizedDescription))
        }
    }

    /// Locks every open folder again: the Lock All button, and when the app leaves the screen.
    func relockAll() async {
        guard hasUnlockedFolders else { return }
        _ = await lockWork { try $0.relock() }
        originals.removeAll()
        thumbnails.removeAll()
    }

    // MARK: Folders

    /// Folders in tree order with their depth, for pickers; locked ones only when open.
    var pickableFolders: [(folder: Folder, depth: Int)] {
        var list: [(folder: Folder, depth: Int)] = []
        func walk(_ folders: [Folder], _ depth: Int) {
            for folder in folders where !folder.isLocked || folder.isUnlocked {
                list.append((folder, depth))
                walk(folder.children, depth + 1)
            }
        }
        walk(folders, 0)
        return list
    }

    func folderName(_ id: String) -> String? {
        Self.find(id, in: folders)?.name
    }

    // MARK: Saved view

    func remember(_ view: SavedView) {
        guard view != savedView else { return }
        savedView = view
        savedViews.save(view, for: url.standardizedFileURL.path)
    }

    /// Back on the list of places: the next launch starts there.
    func forgetPlace() {
        var view = savedView
        view.place = nil
        view.filters = BrowseFilters()
        view.search = ""
        remember(view)
    }

    /// Whether `place` still exists: a saved folder may have been deleted since.
    func exists(_ place: Place) -> Bool {
        switch place {
        case .all, .uncategorized, .untagged, .trash, .tag:
            return true
        case let .folder(id, _):
            return Self.find(id, in: folders).map { !$0.isLocked || $0.isUnlocked } ?? false
        case let .smart(id, _):
            return Self.find(id, in: smartFolders) != nil
        }
    }

    private static func find(_ id: String, in folders: [Folder]) -> Folder? {
        for folder in folders {
            if folder.id == id { return folder }
            if let found = find(id, in: folder.children) { return found }
        }
        return nil
    }

    private static func find(_ id: String, in folders: [SmartFolder]) -> SmartFolder? {
        for folder in folders {
            if folder.id == id { return folder }
            if let found = find(id, in: folder.children) { return found }
        }
        return nil
    }

    // MARK: Events

    /// A browse screen hears when its view is recomputed.
    func register(_ browser: BrowseModel) {
        browsers.removeAll { $0.model == nil || $0.model === browser }
        browsers.append(WeakBrowser(model: browser))
    }

    func unregister(_ browser: BrowseModel) {
        browsers.removeAll { $0.model == nil || $0.model === browser }
    }

    func report(_ error: Error) {
        self.error = AppModel.describe(error)
    }

    private func receive(_ event: LibraryEvent) {
        switch event {
        case let .importProgress(progress):
            if progress.isFinished {
                imports[progress.id] = nil
            } else {
                imports[progress.id] = progress
            }
            progressed(progress)
            scheduleRefresh()
        case let .viewsInvalidated(views):
            for browser in browsers {
                browser.model?.invalidated(views)
            }
            scheduleRefresh()
        case let .exportProgress(progress):
            exports[progress.id] = progress
        }
    }

    /// Imports report often: the counts and the tree are read once per burst.
    private func scheduleRefresh() {
        guard refreshing == nil else { return }
        refreshing = Task {
            try? await Task.sleep(for: .milliseconds(200))
            await refresh()
            refreshing = nil
        }
    }
}

/// An Eagle library picked for import: where it is, whether its scope is held, what it holds.
struct EaglePlan: Identifiable {
    let id = UUID()
    let url: URL
    let scoped: Bool
    let summary: EagleLibrarySummary
}

/// How an Eagle import ended, shown once (D-116).
struct EagleOutcome: Identifiable {
    let id: UInt64
    let report: EagleImportReport
    let cancelled: Bool
}

/// A short note over the library after an action.
struct Toast: Equatable, Identifiable {
    let id = UUID()
    let message: String
    let undoable: Bool
}

private struct WeakBrowser {
    weak var model: BrowseModel?
}

/// A library picked in a File Provider folder, waiting for the user to say where that folder is.
struct PickedLibrary: Identifiable, Equatable {
    let url: URL
    var id: String { url.path }
}

/// Why a library is not created or moved where the user picked (D-132).
enum LocationRefused: Error {
    /// iCloud Drive: no library is created in a synced folder.
    case cloud
    /// That folder already holds a library of this name.
    case exists
}
