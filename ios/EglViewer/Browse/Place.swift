import EglCore
import Foundation

/// Where the user browses: one of the fixed places, a folder or a smart folder.
enum Place: Codable, Hashable {
    case all
    case uncategorized
    case untagged
    case trash
    case folder(id: String, name: String)
    case smart(id: String, name: String)

    var scope: Scope {
        switch self {
        case .all: .all
        case .uncategorized: .unfiled
        case .untagged: .untagged
        case .trash: .trash
        // A folder shows its subfolders' items too, as on the desktop.
        case let .folder(id, _): .folder(id: id, includeDescendants: true)
        case let .smart(id, _): .smart(id: id)
        }
    }

    /// A stable name for the place, for accessibility identifiers and the saved view.
    var key: String {
        switch self {
        case .all: "all"
        case .uncategorized: "uncategorized"
        case .untagged: "untagged"
        case .trash: "trash"
        case let .folder(id, _): "folder.\(id)"
        case let .smart(id, _): "smart.\(id)"
        }
    }
}

/// How the items of a place are laid out.
enum BrowseLayout: String, Codable, CaseIterable {
    case grid, list
}

/// The thumbnail size of the grid: the narrowest a column may be, in points.
enum ThumbnailSize: String, Codable, CaseIterable {
    case small, medium, large

    var minimumWidth: CGFloat {
        switch self {
        case .small: 76
        case .medium: 110
        case .large: 170
        }
    }
}

/// The filters of a view: tags, file type, rating and shape, as on the desktop filter bar.
struct BrowseFilters: Codable, Equatable {
    var tags: [String] = []
    var tagMode: TagMode = .any
    var ext: String?
    /// nil: any rating; 0: unrated; 1...5: exactly that many stars.
    var rating: Int?
    var shape: Shape?

    var isActive: Bool { !tags.isEmpty || ext != nil || rating != nil || shape != nil }

    /// The shapes the core knows (library-core `shape_of`).
    enum Shape: String, Codable, CaseIterable {
        case square
        case landscape
        case portrait
        case panoramicLandscape = "panoramic-landscape"
        case panoramicPortrait = "panoramic-portrait"
    }
}

/// What a view remembers between launches: the sort and the look for the whole library, and
/// the last place with its filters and search (as D-078 on the desktop).
struct SavedView: Codable, Equatable {
    var place: Place?
    var sort: SortKey = .added
    var ascending = false
    var layout: BrowseLayout = .grid
    var size: ThumbnailSize = .medium
    var filters = BrowseFilters()
    var search = ""
}

/// The saved views, one per library path, in `UserDefaults`.
struct SavedViews {
    private let defaults: UserDefaults
    private let key = "egl.saved.views.v1"

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func load(for library: String) -> SavedView {
        all()[library] ?? SavedView()
    }

    func save(_ view: SavedView, for library: String) {
        var list = all()
        list[library] = view
        if let data = try? JSONEncoder().encode(list) {
            defaults.set(data, forKey: key)
        }
    }

    func clear() {
        defaults.removeObject(forKey: key)
    }

    private func all() -> [String: SavedView] {
        guard let data = defaults.data(forKey: key),
              let list = try? JSONDecoder().decode([String: SavedView].self, from: data)
        else { return [:] }
        return list
    }
}

/// The sort keys the phone offers; manual order, tags and duration stay on the desktop.
extension SortKey {
    static let offered: [SortKey] = [.added, .name, .ext, .size, .resolution, .star, .mtime, .btime, .random]
}
