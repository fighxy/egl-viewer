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
    /// The sort the folder opens with; nil follows the library's sort.
    public var orderBy: SortKey?
    public var sortAscending: Bool?
}

/// The colours a folder or a tag group may be given (library-core `FOLDER_COLORS`); stored as
/// their raw names.
public enum LabelColor: String, Codable, CaseIterable, Sendable {
    case red, orange, yellow, green, aqua, blue, purple, pink
}

/// One edit of the folder tree; the answer is the new tree.
public enum FolderChange: Encodable, Equatable, Sendable {
    /// After the last child of `parent` (nil: the top level).
    case create(name: String, parent: String?, description: String = "", iconColor: String? = nil)
    /// The folder's own properties; folder tags are labels on the folder, not item tags.
    case update(
        id: String, name: String, description: String, tags: [String], iconColor: String?,
        orderBy: SortKey?, sortAscending: Bool?
    )
    /// Under `parent` (nil: the top level) at `index` among its other children.
    case move(id: String, parent: String?, index: Int)

    private enum Keys: String, CodingKey {
        case type, id, name, parent, description, tags, iconColor, orderBy, sortAscending, index
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case let .create(name, parent, description, iconColor):
            try c.encode("create", forKey: .type)
            try c.encode(name, forKey: .name)
            try c.encodeIfPresent(parent, forKey: .parent)
            try c.encode(description, forKey: .description)
            try c.encodeIfPresent(iconColor, forKey: .iconColor)
        case let .update(id, name, description, tags, iconColor, orderBy, sortAscending):
            try c.encode("update", forKey: .type)
            try c.encode(id, forKey: .id)
            try c.encode(name, forKey: .name)
            try c.encode(description, forKey: .description)
            try c.encode(tags, forKey: .tags)
            try c.encodeIfPresent(iconColor, forKey: .iconColor)
            try c.encodeIfPresent(orderBy, forKey: .orderBy)
            try c.encodeIfPresent(sortAscending, forKey: .sortAscending)
        case let .move(id, parent, index):
            try c.encode("move", forKey: .type)
            try c.encode(id, forKey: .id)
            try c.encodeIfPresent(parent, forKey: .parent)
            try c.encode(index, forKey: .index)
        }
    }
}

public struct SmartFolder: Codable, Equatable, Identifiable, Sendable {
    public var id: String
    public var name: String
    public var children: [SmartFolder]
    /// Eagle's rule language, as stored.
    public var conditions: JSONValue?
    /// Rules this build cannot evaluate: they never match.
    public var unsupported: [String]
}

/// A file type of the library with its item count, for the Type filter.
public struct ExtensionCount: Codable, Equatable, Hashable, Sendable {
    public var ext: String
    public var count: Int

    public init(ext: String, count: Int) {
        self.ext = ext
        self.count = count
    }
}

/// A smart folder edit; `conditions` is Eagle's array, stored unchanged.
public enum SmartFolderChange: Encodable, Equatable, Sendable {
    case create(name: String, parent: String?, conditions: JSONValue)
    case update(id: String, name: String, conditions: JSONValue)

    private enum Keys: String, CodingKey { case type, id, name, parent, conditions }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case let .create(name, parent, conditions):
            try c.encode("create", forKey: .type)
            try c.encode(name, forKey: .name)
            try c.encodeIfPresent(parent, forKey: .parent)
            try c.encode(conditions, forKey: .conditions)
        case let .update(id, name, conditions):
            try c.encode("update", forKey: .type)
            try c.encode(id, forKey: .id)
            try c.encode(name, forKey: .name)
            try c.encode(conditions, forKey: .conditions)
        }
    }
}

/// "Save as smart folder": the rules of a view's query.
public struct SmartRulesDraft: Codable, Equatable, Sendable {
    public var parent: String?
    public var conditions: JSONValue
}

/// A named set of tags, for the tag list.
public struct TagGroup: Codable, Equatable, Identifiable, Sendable {
    public var id: String
    public var name: String
    /// One of `LabelColor`, or what an imported library holds.
    public var color: String?
    public var tags: [String]
}

public enum TagGroupChange: Encodable, Equatable, Sendable {
    case create(name: String, color: String?, tags: [String])
    case update(id: String, name: String, color: String?, tags: [String])

    private enum Keys: String, CodingKey { case type, id, name, color, tags }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case let .create(name, color, tags):
            try c.encode("create", forKey: .type)
            try c.encode(name, forKey: .name)
            try c.encodeIfPresent(color, forKey: .color)
            try c.encode(tags, forKey: .tags)
        case let .update(id, name, color, tags):
            try c.encode("update", forKey: .type)
            try c.encode(id, forKey: .id)
            try c.encode(name, forKey: .name)
            try c.encodeIfPresent(color, forKey: .color)
            try c.encode(tags, forKey: .tags)
        }
    }
}

public struct Tag: Codable, Equatable, Hashable, Sendable {
    /// The tag is the exact, case-sensitive string.
    public var name: String
    public var imageCount: Int?

    public init(name: String, imageCount: Int? = nil) {
        self.name = name
        self.imageCount = imageCount
    }
}

/// The tags of a selection with how many of its items carry each, and its size.
public struct SelectionTags: Codable, Equatable, Sendable {
    public var items: Int
    /// `imageCount` is the number of selected items with the tag.
    public var tags: [Tag]
    public var size: Int
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

/// A file whose content the library has already: the import waits for an answer.
public struct DuplicateNotice: Codable, Equatable, Sendable {
    public var path: String
    /// The id of the item with the same content.
    public var existing: String
    public var inTrash: Bool
    /// The folder the file was going into; the existing item can join it instead.
    public var targetFolder: String?
}

/// The answer to a duplicate; the first answer holds for the rest of the batch.
public enum DuplicateAnswer: String, Sendable {
    case skip, addAnyway, addToFolder, cancel
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
    /// The duplicate that waits for an answer (state `waitingForDecision`).
    public var duplicate: DuplicateNotice?
    /// What an Eagle import did, on its last progress (D-116).
    public var report: EagleImportReport?

    public var isFinished: Bool { state == .done || state == .cancelled }
}

/// An item an import could not take, and why.
public struct ImportFailure: Codable, Equatable, Sendable {
    public var path: String
    public var reason: String
}

/// What an Eagle import did (docs/spec.md §8.2 step 5).
public struct EagleImportReport: Codable, Equatable, Sendable {
    /// Items copied into the library, trash included when asked for.
    public var imported: Int
    /// Items whose content another item of this import already had (kept, as in Eagle).
    public var duplicates: Int
    /// Trashed Eagle items left out.
    public var trashSkipped: Int
    /// Items that could not be imported, with the reason (the first 200).
    public var skipped: [ImportFailure]
    /// Locked Eagle folders: imported locked with no password; their content stays hidden
    /// until a password is set or the lock is removed.
    public var lockedFolders: [String]
}

/// What the import screen shows for a picked Eagle library (D-116).
public struct EagleLibrarySummary: Codable, Equatable, Sendable {
    public var path: String
    public var name: String
    /// Items including Eagle's trash.
    public var itemCount: Int
    /// Bytes of the originals and Eagle's thumbnails.
    public var bytes: Int
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

    public var isFinished: Bool { state == .done || state == .failed || state == .cancelled }
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

public enum BackupKind: String, Codable, Sendable {
    case daily, manual, beforeMigration, beforeRestore
}

/// A copy of `library.db` in `backups/`; restoring names it by `name`.
public struct Backup: Codable, Equatable, Identifiable, Sendable {
    public var name: String
    public var kind: BackupKind?
    public var createdAt: Int
    public var size: Int
    public var id: String { name }
}

/// Bytes on disk, for Library Settings.
public struct LibraryStats: Codable, Equatable, Sendable {
    public var originals: Int
    public var thumbnails: Int
    public var database: Int
    public var backups: Int
    public var total: Int { originals + thumbnails + database + backups }
}

public struct MissingOriginal: Codable, Equatable, Sendable {
    public var id: String
    /// Empty for an item of a closed locked folder.
    public var name: String
    public var inTrash: Bool
}

/// What Check Library found.
public struct LibraryCheck: Codable, Equatable, Sendable {
    public var items: Int
    public var databaseOk: Bool
    public var databaseProblems: [String]
    public var missingCount: Int
    public var missing: [MissingOriginal]
    public var orphanCount: Int
    public var orphanBytes: Int
    public var orphans: [String]
    public var orphanThumbnails: Int

    public var isClean: Bool {
        databaseOk && missingCount == 0 && orphanCount == 0 && orphanThumbnails == 0
    }
}

/// What Repair removes after a check.
public struct RepairOptions: Codable, Equatable, Sendable {
    /// Files and thumbnails no item points at.
    public var orphans: Bool
    /// Items whose original is gone, removed for good.
    public var missing: Bool

    public init(orphans: Bool, missing: Bool) {
        self.orphans = orphans
        self.missing = missing
    }
}

/// A background task making thumbnails; `id` is nil when one already runs.
public struct PreviewsTicket: Codable, Equatable, Sendable {
    public var id: UInt64?
    public var waiting: Int
}

/// What an export covers: the whole library, some folders, some tags, or a mix.
public struct ExportSelection: Codable, Equatable, Sendable {
    public var whole: Bool
    public var folders: [String]
    public var tags: [String]

    public init(whole: Bool = false, folders: [String] = [], tags: [String] = []) {
        self.whole = whole
        self.folders = folders
        self.tags = tags
    }

    public var isEmpty: Bool { !whole && folders.isEmpty && tags.isEmpty }
}

public enum ExportCompression: String, Codable, CaseIterable, Sendable {
    case none, fast, balanced, smallest
}

/// How a ZIP export with a passphrase is protected (D-112): the whole ZIP in an age file, or a
/// standard ZIP with AES-256 entries that 7-Zip, WinRAR and Keka open (names stay visible).
public enum ZipEncryption: String, Codable, CaseIterable, Sendable {
    case age, aes
}

/// A folder tree that mirrors the library's folders, or one ZIP or 7z archive of the same tree.
public enum ExportFormat: Codable, Equatable, Sendable {
    case mirror
    case zip(ExportCompression, encryption: ZipEncryption = .age)
    /// 7z (D-113): with a passphrase the contents and the names are AES-256 encrypted.
    case sevenZip(ExportCompression)

    private enum Keys: String, CodingKey { case kind, compression, encryption }

    /// An archive, which a passphrase can protect.
    public var isArchive: Bool {
        if case .mirror = self { return false }
        return true
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .mirror:
            try c.encode("mirror", forKey: .kind)
        case let .zip(compression, encryption):
            try c.encode("zip", forKey: .kind)
            try c.encode(compression, forKey: .compression)
            try c.encode(encryption, forKey: .encryption)
        case let .sevenZip(compression):
            try c.encode("sevenZip", forKey: .kind)
            try c.encode(compression, forKey: .compression)
        }
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(String.self, forKey: .kind) {
        case "zip":
            self = .zip(
                try c.decode(ExportCompression.self, forKey: .compression),
                encryption: try c.decodeIfPresent(ZipEncryption.self, forKey: .encryption) ?? .age
            )
        case "sevenZip":
            self = .sevenZip(try c.decode(ExportCompression.self, forKey: .compression))
        default:
            self = .mirror
        }
    }
}

/// One export into a folder or an archive; Eagle is not a destination on the phone.
public struct ExportRequest: Encodable, Equatable, Sendable {
    public var selection: ExportSelection
    public var includeSubfolders: Bool
    public var format: ExportFormat
    /// An existing folder the export is written into.
    public var destination: String
    /// An age passphrase protecting the archive; ZIP only. Never stored.
    public var encrypt: String?

    public init(selection: ExportSelection, includeSubfolders: Bool, format: ExportFormat, destination: String, encrypt: String? = nil) {
        self.selection = selection
        self.includeSubfolders = includeSubfolders
        self.format = format
        self.destination = destination
        self.encrypt = encrypt
    }
}

/// What an export would write, before it starts.
public struct ExportPreview: Codable, Equatable, Sendable {
    public var items: Int
    public var bytes: Int
    /// Items a closed locked folder hides; they are not exported.
    public var hidden: Int
}

/// An age archive opened with its passphrase (D-110): the plaintext file it was written to.
public struct DecryptedArchive: Codable, Equatable, Sendable {
    public var path: String
    public var bytes: Int

    public var url: URL { URL(fileURLWithPath: path) }
}

/// The shortest passphrase an encrypted export takes (library-core `MIN_PASSPHRASE_CHARS`,
/// D-110); the core refuses a shorter one.
public let minPassphraseCharacters = 8

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
