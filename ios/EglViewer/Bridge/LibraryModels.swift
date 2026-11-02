import Foundation

/// Wire shapes mirrored from `library-core` ItemCell / LibraryInfo JSON (camelCase).
struct LibraryInfoDTO: Codable, Hashable {
    var kind: String
    var id: String?
    var name: String
    var path: String?
    var readOnly: Bool
}

struct ItemCellDTO: Codable, Identifiable, Hashable {
    var id: String
    var name: String
    var ext: String
    var width: UInt32?
    var height: UInt32?
    var star: UInt8
    var hasThumbnail: Bool
    var thumbVersion: Int64
    var tags: [String]
    var pending: Bool?
}

struct ViewWindowDTO: Codable {
    var total: Int
    var start: UInt32
    var items: [ItemCellDTO]
}

struct ItemDTO: Codable, Identifiable, Hashable {
    var id: String
    var name: String
    var ext: String
    var star: UInt8
    var tags: [String]
    var folders: [String]
    var annotation: String
    var isDeleted: Bool
}

struct FolderDTO: Codable, Identifiable, Hashable {
    var id: String
    var name: String
    var children: [FolderDTO]
    var imageCount: UInt32?
    var isLocked: Bool
}

struct TagDTO: Codable, Hashable {
    var name: String
    var imageCount: UInt32?
}

struct RecentLibrary: Identifiable, Hashable {
    var id: String { path }
    var name: String
    var path: String
    var locationHint: String
    var missing: Bool
}
