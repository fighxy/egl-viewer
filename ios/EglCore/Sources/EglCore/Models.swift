// The JSON shapes of the Rust core, camelCase as on the desktop (ui/src/ipc/generated).
// Decoding ignores fields a model does not name, so the core can add fields freely.

import Foundation

public enum LibraryKind: String, Codable, Sendable {
    case native, eagleApi, eagleFolder
}

public enum ReadOnlyReason: String, Codable, Sendable {
    case otherInstance, syncFolder, readOnlyMedium
}

public struct LibraryInfo: Codable, Equatable, Sendable {
    public var kind: LibraryKind
    public var id: String?
    public var name: String
    public var path: String?
    public var readOnly: Bool
    public var readOnlyReason: ReadOnlyReason?
}

public struct LibraryCounts: Codable, Equatable, Sendable {
    public var all: Int
    public var uncategorized: Int
    public var untagged: Int
    public var trash: Int
}

/// One position of a view: what a grid cell shows.
public struct ItemCell: Codable, Equatable, Identifiable, Sendable {
    public var id: String
    public var name: String
    public var ext: String
    public var width: Int?
    public var height: Int?
    public var size: Int?
    public var star: Int
    public var palette0: [Int]?
    public var hasThumbnail: Bool
    public var thumbVersion: Int
    public var addedAt: Int?
    public var deletedAt: Int?
    public var tags: [String]
}

/// The full item, for the details.
public struct Item: Codable, Equatable, Identifiable, Sendable {
    public var id: String
    public var name: String
    public var ext: String
    public var width: Int?
    public var height: Int?
    public var size: Int?
    public var star: Int
    public var tags: [String]
    public var folders: [String]
    public var annotation: String
    public var url: String
    public var addedAt: Int?
    public var fileCreatedAt: Int?
    public var fileModifiedAt: Int?
    public var duration: Double?
    public var noThumbnail: Bool
    public var thumbVersion: Int
    public var isDeleted: Bool
    public var deletedAt: Int?
}

public struct Folder: Codable, Equatable, Identifiable, Sendable {
    public var id: String
    public var name: String
    public var description: String
    public var iconColor: String?
    public var tags: [String]
    public var children: [Folder]
    public var imageCount: Int?
    public var descendantImageCount: Int?
    public var isLocked: Bool
    public var isUnlocked: Bool
    public var hasPassword: Bool
    public var lockLabel: String?
    public var lockColor: String?
}

public struct Tag: Codable, Equatable, Hashable, Sendable {
    /// The tag is the exact, case-sensitive string.
    public var name: String
    public var imageCount: Int?
}

public struct ViewOpened: Codable, Equatable, Sendable {
    public var viewId: UInt64
    public var total: Int
}

public struct ImportTicket: Codable, Equatable, Sendable {
    public var id: UInt64
}

public enum ImportKind: String, Codable, Sendable {
    case files, previews, eagle
}

public enum ImportState: String, Codable, Sendable {
    case scanning, adding, waitingForDecision, previews, done, cancelled
}

public struct ImportProgress: Codable, Equatable, Identifiable, Sendable {
    public var id: UInt64
    public var kind: ImportKind
    public var state: ImportState
    public var total: Int
    public var processed: Int
    public var added: Int
    public var duplicates: Int
    public var failed: Int
    public var previewsTotal: Int
    public var previewsDone: Int
    public var current: String?

    public var isFinished: Bool { state == .done || state == .cancelled }
}

public enum ExportState: String, Codable, Sendable {
    case preparing, copying, encrypting, done, failed, cancelled
}

public struct ExportSummary: Codable, Equatable, Sendable {
    public var items: Int
    public var files: Int
    public var bytes: Int
    public var missing: [String]
    public var path: String
}

public struct ExportProgress: Codable, Equatable, Identifiable, Sendable {
    public var id: UInt64
    public var state: ExportState
    public var total: Int
    public var done: Int
    public var bytesTotal: Int
    public var bytesDone: Int
    public var current: String?
    public var summary: ExportSummary?
    public var error: String?
}

/// Shown once when a folder is locked or the key renewed; never stored.
public struct RecoveryKey: Codable, Equatable, Sendable {
    public var key: String
    public var phrase: String
}

public struct UndoStatus: Codable, Equatable, Sendable {
    public var kind: String
    public var items: Int
}

public struct TrashInfo: Codable, Equatable, Sendable {
    public var items: Int
    public var bytes: Int
}

public struct Backup: Codable, Equatable, Identifiable, Sendable {
    public var name: String
    public var createdAt: Int
    public var size: Int
    public var id: String { name }
}

/// Where a view looks.
public enum Scope: Codable, Equatable, Hashable, Sendable {
    case all
    case unfiled
    case untagged
    case trash
    case folder(id: String, includeDescendants: Bool)
    case smart(id: String)

    private enum Keys: String, CodingKey { case kind, id, includeDescendants }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .all: try c.encode("all", forKey: .kind)
        case .unfiled: try c.encode("unfiled", forKey: .kind)
        case .untagged: try c.encode("untagged", forKey: .kind)
        case .trash: try c.encode("trash", forKey: .kind)
        case let .folder(id, includeDescendants):
            try c.encode("folder", forKey: .kind)
            try c.encode(id, forKey: .id)
            try c.encode(includeDescendants, forKey: .includeDescendants)
        case let .smart(id):
            try c.encode("smart", forKey: .kind)
            try c.encode(id, forKey: .id)
        }
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(String.self, forKey: .kind) {
        case "unfiled": self = .unfiled
        case "untagged": self = .untagged
        case "trash": self = .trash
        case "folder":
            self = .folder(
                id: try c.decode(String.self, forKey: .id),
                includeDescendants: try c.decode(Bool.self, forKey: .includeDescendants)
            )
        case "smart": self = .smart(id: try c.decode(String.self, forKey: .id))
        default: self = .all
        }
    }
}

public enum SortKey: String, Codable, Sendable {
    case added, btime, mtime, name, ext, size, resolution, star, duration, tag, random, manual
}

public enum TagMode: String, Codable, Sendable {
    case any, all, none
}

/// What a view shows: a scope, filter chips, search text and the sort. Fields left nil take
/// the core's defaults (All, newest first).
public struct LibraryQuery: Codable, Equatable, Hashable, Sendable {
    public var scope: Scope?
    public var tags: [String]?
    public var tagMode: TagMode?
    public var extraFolderIds: [String]?
    public var folderMode: TagMode?
    public var ext: String?
    public var rating: Int?
    public var shape: String?
    public var text: String?
    public var sort: SortKey?
    public var ascending: Bool?

    public init(
        scope: Scope? = nil, tags: [String]? = nil, tagMode: TagMode? = nil,
        extraFolderIds: [String]? = nil, folderMode: TagMode? = nil, ext: String? = nil,
        rating: Int? = nil, shape: String? = nil, text: String? = nil, sort: SortKey? = nil,
        ascending: Bool? = nil
    ) {
        self.scope = scope
        self.tags = tags
        self.tagMode = tagMode
        self.extraFolderIds = extraFolderIds
        self.folderMode = folderMode
        self.ext = ext
        self.rating = rating
        self.shape = shape
        self.text = text
        self.sort = sort
        self.ascending = ascending
    }
}

/// A selection of a view: listed ids, or with `all` every item of the view except `ids`.
public struct ItemsTarget: Codable, Equatable, Sendable {
    public var viewId: UInt64?
    public var all: Bool
    public var ids: [String]

    public init(viewId: UInt64? = nil, all: Bool = false, ids: [String]) {
        self.viewId = viewId
        self.all = all
        self.ids = ids
    }
}

/// One edit of one or many items; undo can take it back.
public enum ItemEdit: Encodable, Equatable, Sendable {
    case addTags([String])
    case removeTags([String])
    case addToFolders([String])
    case removeFromFolders([String])
    case setFolders([String])
    case setStar(Int)
    case setAnnotation(String)
    case setUrl(String)
    case rename(String)
    case trash
    case restore

    private enum Keys: String, CodingKey { case type, value }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case let .addTags(v): try c.encode("addTags", forKey: .type); try c.encode(v, forKey: .value)
        case let .removeTags(v): try c.encode("removeTags", forKey: .type); try c.encode(v, forKey: .value)
        case let .addToFolders(v): try c.encode("addToFolders", forKey: .type); try c.encode(v, forKey: .value)
        case let .removeFromFolders(v): try c.encode("removeFromFolders", forKey: .type); try c.encode(v, forKey: .value)
        case let .setFolders(v): try c.encode("setFolders", forKey: .type); try c.encode(v, forKey: .value)
        case let .setStar(v): try c.encode("setStar", forKey: .type); try c.encode(v, forKey: .value)
        case let .setAnnotation(v): try c.encode("setAnnotation", forKey: .type); try c.encode(v, forKey: .value)
        case let .setUrl(v): try c.encode("setUrl", forKey: .type); try c.encode(v, forKey: .value)
        case let .rename(v): try c.encode("rename", forKey: .type); try c.encode(v, forKey: .value)
        case .trash: try c.encode("trash", forKey: .type)
        case .restore: try c.encode("restore", forKey: .type)
        }
    }
}

/// What the core tells the app while it works.
public enum LibraryEvent: Decodable, Equatable, Sendable {
    case importProgress(ImportProgress)
    case exportProgress(ExportProgress)
    /// Views recomputed under their ids, with their new totals.
    case viewsInvalidated([ViewOpened])

    private enum Keys: String, CodingKey { case type, progress, views }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "importProgress": self = .importProgress(try c.decode(ImportProgress.self, forKey: .progress))
        case "exportProgress": self = .exportProgress(try c.decode(ExportProgress.self, forKey: .progress))
        default: self = .viewsInvalidated(try c.decode([ViewOpened].self, forKey: .views))
        }
    }
}
