import Foundation
import UIKit

/// Thin façade used by the SwiftUI screens.
///
/// Without `libegl_library.a` the UI runs on **mock data**. When the Rust static
/// library is linked and `EGL_HAS_FFI` is set (Config/WithFfi.xcconfig), open /
/// create / thumbs call the C ABI (D-085).
@MainActor
final class LibraryBridge: ObservableObject {
    static let appVersion = "0.2.1-ios"

    @Published private(set) var info: LibraryInfoDTO?
    @Published private(set) var items: [ItemCellDTO] = []
    @Published private(set) var total: Int = 0
    @Published private(set) var folders: [FolderDTO] = []
    @Published private(set) var tags: [TagDTO] = []
    @Published private(set) var usingMockData: Bool = true
    @Published var lastError: String?

    private var mockStore: MockLibraryStore?
    private var thumbCache: [String: UIImage] = [:]
    #if EGL_HAS_FFI
    private var ffi = NativeFfiSession()
    #endif

    var isOpen: Bool { info != nil }
    var libraryName: String { info?.name ?? "Library" }
    var ffiLinked: Bool {
        #if EGL_HAS_FFI
        true
        #else
        false
        #endif
    }

    func openRecent(_ recent: RecentLibrary) {
        let path = LibraryAccess.shared.resolveForOpen(path: recent.path)
        open(path: path, syncFolder: recent.locationHint.contains("iCloud") || recent.locationHint == "Files")
    }

    func open(path: String, syncFolder: Bool) {
        close(keepScoped: true)
        lastError = nil

        #if EGL_HAS_FFI
        do {
            try ffi.open(path: path, syncFolder: syncFolder)
            usingMockData = false
            try reloadFromFfi()
            thumbCache.removeAll()
            return
        } catch {
            lastError = error.localizedDescription
        }
        #endif

        let name = (path as NSString).lastPathComponent
        mockStore = MockLibraryStore.sample(named: name)
        usingMockData = true
        publishMock()
        if lastError == nil {
            lastError = ffiLinked
                ? "Could not open library — mock fallback"
                : "Rust FFI not linked — mock library"
        }
    }

    func openPickedLibrary(url: URL) {
        let accessed = url.startAccessingSecurityScopedResource()
        defer {
            // Bookmark retains access for later; stop the picker grant after remember.
            if accessed {
                // Keep access via LibraryAccess bookmark resolve on next open.
            }
        }
        let hint = LibraryAccess.shared.locationHint(for: url)
        LibraryAccess.shared.remember(url: url, locationHint: hint)
        let path = LibraryAccess.shared.resolveForOpen(path: url.path)
        open(path: path, syncFolder: LibraryAccess.shared.isSyncFolder(url))
        if accessed {
            url.stopAccessingSecurityScopedResource()
        }
    }

    func createLibrary(named name: String, parentURL: URL?) {
        close()
        lastError = nil

        #if EGL_HAS_FFI
        if let parentURL {
            let accessed = parentURL.startAccessingSecurityScopedResource()
            defer {
                if accessed {
                    parentURL.stopAccessingSecurityScopedResource()
                }
            }
            do {
                let created = try NativeFfiSession.createLibrary(
                    parent: parentURL.path,
                    name: name
                )
                let url = URL(fileURLWithPath: created, isDirectory: true)
                LibraryAccess.shared.remember(
                    url: url,
                    locationHint: LibraryAccess.shared.locationHint(for: parentURL)
                )
                open(path: created, syncFolder: LibraryAccess.shared.isSyncFolder(parentURL))
                return
            } catch {
                lastError = error.localizedDescription
            }
        }
        #endif

        mockStore = MockLibraryStore.sample(named: name)
        usingMockData = true
        publishMock()
    }

    func close() {
        close(keepScoped: false)
    }

    private func close(keepScoped: Bool) {
        #if EGL_HAS_FFI
        ffi.close()
        #endif
        if !keepScoped {
            LibraryAccess.shared.stopScopedAccess()
        }
        mockStore = nil
        info = nil
        items = []
        total = 0
        folders = []
        tags = []
        thumbCache.removeAll()
        usingMockData = true
    }

    func refresh() {
        #if EGL_HAS_FFI
        if !usingMockData {
            try? reloadFromFfi()
            return
        }
        #endif
        publishMock()
    }

    func itemDetail(id: String) -> ItemDTO? {
        #if EGL_HAS_FFI
        if !usingMockData, let item = try? ffi.item(id: id) {
            return item
        }
        #endif
        return mockStore?.item(id: id)
    }

    func applyAddTag(itemId: String, tag: String) {
        #if EGL_HAS_FFI
        if !usingMockData {
            try? ffi.applyAddTag(itemId: itemId, tag: tag)
            try? reloadFromFfi()
            return
        }
        #endif
        guard var mock = mockStore else { return }
        mock.addTag(itemId: itemId, tag: tag)
        mockStore = mock
        publishMock()
    }

    /// WebP thumbnail bytes decoded to a UIImage when FFI is live; nil for mock / missing.
    func thumbnailImage(id: String) -> UIImage? {
        if let cached = thumbCache[id] {
            return cached
        }
        #if EGL_HAS_FFI
        if !usingMockData, let payload = try? ffi.thumbnailBytes(id: id) {
            if let image = UIImage(data: payload.bytes) {
                thumbCache[id] = image
                return image
            }
        }
        #endif
        return nil
    }

    private func publishMock() {
        guard let mock = mockStore else { return }
        info = mock.info
        items = mock.cells
        total = mock.cells.count
        folders = mock.folders
        tags = mock.tags
    }

    #if EGL_HAS_FFI
    private func reloadFromFfi() throws {
        info = try ffi.libraryInfo()
        let window = try ffi.viewWindow(scopeJson: #"{"kind":"all"}"#, start: 0, len: 200)
        items = window.items
        total = window.total
        folders = try ffi.folderTree()
        tags = try ffi.tags()
    }
    #endif
}

struct MockLibraryStore {
    var info: LibraryInfoDTO
    var cells: [ItemCellDTO]
    var folders: [FolderDTO]
    var tags: [TagDTO]
    var details: [String: ItemDTO]

    static func sample(named name: String) -> MockLibraryStore {
        let clean = name.replacingOccurrences(of: ".eglib", with: "")
        let cells: [ItemCellDTO] = (1 ... 12).map { index in
            ItemCellDTO(
                id: String(format: "mock%02d", index),
                name: "Reference \(index)",
                ext: index % 3 == 0 ? "png" : "jpg",
                width: 1600,
                height: index % 2 == 0 ? 900 : 1200,
                star: UInt8(index % 6),
                hasThumbnail: false,
                thumbVersion: 1,
                tags: index % 2 == 0 ? ["Ref"] : ["Mood"],
                pending: false
            )
        }
        let folders = [
            FolderDTO(
                id: "fld-clients",
                name: "Clients",
                children: [
                    FolderDTO(id: "fld-acme", name: "Acme", children: [], imageCount: 4, isLocked: false),
                ],
                imageCount: 4,
                isLocked: false
            ),
            FolderDTO(id: "fld-mood", name: "Moodboards", children: [], imageCount: 8, isLocked: false),
        ]
        let tags = [
            TagDTO(name: "Ref", imageCount: 6),
            TagDTO(name: "Mood", imageCount: 6),
        ]
        var details: [String: ItemDTO] = [:]
        for cell in cells {
            details[cell.id] = ItemDTO(
                id: cell.id,
                name: cell.name,
                ext: cell.ext,
                star: cell.star,
                tags: cell.tags,
                folders: cell.tags.contains("Ref") ? ["fld-clients"] : ["fld-mood"],
                annotation: "",
                isDeleted: false
            )
        }
        return MockLibraryStore(
            info: LibraryInfoDTO(
                kind: "native",
                id: "mock-library",
                name: clean.isEmpty ? "Demo" : clean,
                path: nil,
                readOnly: false
            ),
            cells: cells,
            folders: folders,
            tags: tags,
            details: details
        )
    }

    func item(id: String) -> ItemDTO? {
        details[id]
    }

    mutating func addTag(itemId: String, tag: String) {
        guard var item = details[itemId] else { return }
        if !item.tags.contains(where: { $0 == tag }) {
            item.tags.append(tag)
        }
        details[itemId] = item
        if let index = cells.firstIndex(where: { $0.id == itemId }) {
            cells[index].tags = item.tags
        }
        if !tags.contains(where: { $0.name == tag }) {
            tags.append(TagDTO(name: tag, imageCount: 1))
        }
    }
}
