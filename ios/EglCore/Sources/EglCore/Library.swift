// A typed Swift face of the C ABI (crates/library-ffi, D-092): one `Library` per open
// `.eglib`. Every call is synchronous and may block (Argon2id on unlock, file copies, the
// first evaluation of a view): call it off the main thread. Events arrive on a background
// thread through `setEventHandler`.

import CEglLibrary
import Foundation

/// A failure reported by the core, with its reason.
public struct EglError: Error, Equatable, CustomStringConvertible, Sendable {
    public enum Code: Int, Sendable {
        case invalid = 1, notFound, readOnly, notALibrary, storage, unsupported, other,
            wrongPassword, locked
    }

    public let code: Code
    public let message: String

    public var description: String { message.isEmpty ? "\(code)" : message }

    init(status: EglStatus, message: String) {
        code = Code(rawValue: Int(status.rawValue)) ?? .other
        self.message = message
    }
}

/// One open library. Close it with `close()`; it also closes when released.
public final class Library: @unchecked Sendable {
    private var handle: OpaquePointer?
    private let lock = NSLock()
    /// Kept alive while the core may call it.
    private var events: EventBox?

    private init(handle: OpaquePointer) {
        self.handle = handle
    }

    deinit {
        close()
    }

    // MARK: Opening

    /// Creates `<parent>/<name>.eglib` and returns its path.
    public static func create(in parent: URL, name: String, appVersion: String) throws -> URL {
        var out: UnsafeMutablePointer<CChar>?
        let status = parent.path.withCString { parent in
            name.withCString { name in
                appVersion.withCString { version in
                    egl_create_library(parent, name, version, &out)
                }
            }
        }
        guard status == EGL_OK, let out else { throw globalError(status) }
        defer { egl_string_free(out) }
        return URL(fileURLWithPath: String(cString: out))
    }

    /// Opens a library folder; `syncFolder` opens it read-only (iCloud and other synced places).
    public static func open(at path: URL, appVersion: String, syncFolder: Bool = false) throws -> Library {
        let handle = path.path.withCString { path in
            appVersion.withCString { version in
                egl_open(path, version, syncFolder ? 1 : 0)
            }
        }
        guard let handle else { throw globalError(EGL_ERR_OTHER) }
        return Library(handle: handle)
    }

    /// Stops imports and exports and closes the library. Further calls fail.
    public func close() {
        lock.lock()
        let closing = handle
        handle = nil
        lock.unlock()
        guard let closing else { return }
        egl_set_event_callback(closing, nil, nil)
        egl_close(closing)
        events = nil
    }

    // MARK: Events

    /// Sets what hears the core's events (nil stops them). Runs on a background thread.
    public func setEventHandler(_ handler: (@Sendable (LibraryEvent) -> Void)?) throws {
        let lib = try live()
        guard let handler else {
            egl_set_event_callback(lib, nil, nil)
            events = nil
            return
        }
        let box = EventBox(handler)
        events = box
        let status = egl_set_event_callback(lib, { ctx, json in
            guard let ctx, let json else { return }
            Unmanaged<EventBox>.fromOpaque(ctx).takeUnretainedValue().receive(json)
        }, Unmanaged.passUnretained(box).toOpaque())
        try check(status)
    }

    // MARK: Library

    public func info() throws -> LibraryInfo { try json { egl_library_info($0, $1) } }

    public func counts() throws -> LibraryCounts { try json { egl_counts($0, $1) } }

    public func rename(to name: String) throws -> LibraryInfo {
        try name.withCString { name in try json { egl_rename_library($0, name, $1) } }
    }

    // MARK: Views

    public func openView(_ query: LibraryQuery = LibraryQuery()) throws -> ViewOpened {
        let text = try encode(query)
        return try text.withCString { query in try json { egl_open_view($0, query, $1) } }
    }

    public func range(of view: UInt64, start: Int, count: Int) throws -> [ItemCell] {
        try json { egl_view_range($0, view, UInt32(start), UInt32(count), $1) }
    }

    public func ids(of view: UInt64, start: Int, count: Int) throws -> [String] {
        try json { egl_view_ids($0, view, UInt32(start), UInt32(count), $1) }
    }

    public func dimensions(of view: UInt64) throws -> [[Int]] {
        try json { egl_view_dimensions($0, view, $1) }
    }

    public func closeView(_ view: UInt64) {
        guard let lib = try? live() else { return }
        egl_close_view(lib, view)
    }

    // MARK: Items

    public func item(_ id: String) throws -> Item {
        try id.withCString { id in try json { egl_item($0, id, $1) } }
    }

    @discardableResult
    public func edit(_ id: String, _ edit: ItemEdit) throws -> Item {
        let text = try encode(edit)
        return try id.withCString { id in
            try text.withCString { edit in try json { egl_apply_item_edit($0, id, edit, $1) } }
        }
    }

    @discardableResult
    public func edit(_ target: ItemsTarget, _ edit: ItemEdit) throws -> Int {
        let target = try encode(target)
        let edit = try encode(edit)
        return try target.withCString { target in
            try edit.withCString { edit in try count { egl_edit_items($0, target, edit, $1) } }
        }
    }

    /// Takes back the last edit; nil when there was nothing to undo.
    @discardableResult
    public func undo() throws -> UndoStatus? { try json { egl_undo($0, $1) } }

    public func undoStatus() throws -> UndoStatus? { try json { egl_undo_status($0, $1) } }

    @discardableResult
    public func deleteForGood(_ target: ItemsTarget) throws -> Int {
        let target = try encode(target)
        return try target.withCString { target in try count { egl_delete_items($0, target, $1) } }
    }

    @discardableResult
    public func restoreTrash() throws -> Int { try count { egl_restore_trash($0, $1) } }

    @discardableResult
    public func emptyTrash() throws -> Int { try count { egl_empty_trash($0, $1) } }

    public func trashInfo() throws -> TrashInfo { try json { egl_trash_info($0, $1) } }

    /// WebP bytes of the item's thumbnail; nil when it has none yet.
    public func thumbnail(_ id: String) throws -> Data? {
        try bytes(id) { egl_thumbnail_bytes($0, $1, $2, $3, nil) }
    }

    /// The original's bytes, decrypted for an item of an unlocked folder.
    public func originalBytes(_ id: String) throws -> Data? {
        try bytes(id) { egl_original_bytes($0, $1, $2, $3, nil) }
    }

    /// The stored original, for sharing; refused for an item of a locked folder.
    public func originalPath(_ id: String) throws -> URL {
        try id.withCString { id in
            URL(fileURLWithPath: try string { egl_original_path($0, id, $1) })
        }
    }

    // MARK: Folders and tags

    public func folders() throws -> [Folder] { try json { egl_folder_tree($0, $1) } }

    @discardableResult
    public func createFolder(named name: String, parent: String? = nil) throws -> [Folder] {
        struct Create: Encodable {
            let type = "create"
            let name: String
            let parent: String?
            let description = ""
            let iconColor: String? = nil
        }
        let change = try encode(Create(name: name, parent: parent))
        return try change.withCString { change in try json { egl_save_folder($0, change, $1) } }
    }

    public func tags() throws -> [Tag] { try json { egl_tags($0, $1) } }

    public func smartFolders() throws -> [SmartFolder] { try json { egl_smart_folders($0, $1) } }

    /// File types with their item counts, for the Type filter.
    public func extensions() throws -> [ExtensionCount] { try json { egl_extensions($0, $1) } }

    // MARK: Adding files

    /// Starts adding files and folders in the background; progress comes as events.
    public func addFiles(_ urls: [URL], into folder: String? = nil) throws -> ImportTicket {
        struct Request: Encodable {
            let paths: [String]
            let folder: String?
        }
        let request = try encode(Request(paths: urls.map(\.path), folder: folder))
        return try request.withCString { request in try json { egl_add_files($0, request, $1) } }
    }

    /// The progress of import `id`; nil while it has not reported yet.
    public func importStatus(_ id: UInt64) throws -> ImportProgress? {
        try json { egl_import_status($0, id, $1) }
    }

    public func cancelImport(_ id: UInt64) throws {
        try check(egl_cancel_import(try live(), id))
    }

    // MARK: Locked folders

    /// Locks a folder and returns its recovery key, to be shown once.
    public func lockFolder(_ id: String, password: String) throws -> RecoveryKey {
        struct Request: Encodable { let password: String }
        let request = try encode(Request(password: password))
        return try id.withCString { id in
            try request.withCString { request in try json { egl_lock_folder($0, id, request, $1) } }
        }
    }

    public func unlockFolder(_ id: String, password: String) throws {
        let lib = try live()
        try check(id.withCString { id in password.withCString { egl_unlock_folder(lib, id, $0) } })
    }

    /// Locks an open folder again; nil locks every open folder.
    public func relock(_ id: String? = nil) throws {
        let lib = try live()
        let status: EglStatus
        if let id {
            status = id.withCString { egl_relock_folder(lib, $0) }
        } else {
            status = egl_relock_folder(lib, nil)
        }
        try check(status)
    }

    // MARK: Plumbing

    private func live() throws -> OpaquePointer {
        lock.lock()
        defer { lock.unlock() }
        guard let handle else {
            throw EglError(status: EGL_ERR_INVALID, message: "The library is closed.")
        }
        return handle
    }

    private func error(_ status: EglStatus) -> EglError {
        let message = handle.flatMap { egl_last_error($0) }.map { String(cString: $0) } ?? ""
        return EglError(status: status, message: message)
    }

    private static func globalError(_ status: EglStatus) -> EglError {
        EglError(status: status, message: String(cString: egl_last_error_global()))
    }

    private func check(_ status: EglStatus) throws {
        guard status == EGL_OK else { throw error(status) }
    }

    private func string(
        _ call: (OpaquePointer, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> EglStatus
    ) throws -> String {
        let lib = try live()
        var out: UnsafeMutablePointer<CChar>?
        try check(call(lib, &out))
        guard let out else { throw EglError(status: EGL_ERR_OTHER, message: "no answer") }
        defer { egl_string_free(out) }
        return String(cString: out)
    }

    private func json<T: Decodable>(
        _ call: (OpaquePointer, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> EglStatus
    ) throws -> T {
        let text = try string(call)
        return try JSONDecoder().decode(T.self, from: Data(text.utf8))
    }

    private func count(_ call: (OpaquePointer, UnsafeMutablePointer<UInt32>) -> EglStatus) throws -> Int {
        let lib = try live()
        var out: UInt32 = 0
        try check(call(lib, &out))
        return Int(out)
    }

    private func bytes(
        _ id: String,
        _ call: (
            OpaquePointer, UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>,
            UnsafeMutablePointer<Int>
        ) -> EglStatus
    ) throws -> Data? {
        let lib = try live()
        var out: UnsafeMutablePointer<UInt8>?
        var length = 0
        let status = id.withCString { call(lib, $0, &out, &length) }
        if status == EGL_ERR_NOT_FOUND { return nil }
        try check(status)
        guard let out else { return nil }
        defer { egl_bytes_free(out, length) }
        return Data(bytes: out, count: length)
    }

    private func encode<T: Encodable>(_ value: T) throws -> String {
        String(decoding: try JSONEncoder().encode(value), as: UTF8.self)
    }
}

/// Holds the app's handler for the C callback.
private final class EventBox: @unchecked Sendable {
    let handler: @Sendable (LibraryEvent) -> Void

    init(_ handler: @escaping @Sendable (LibraryEvent) -> Void) {
        self.handler = handler
    }

    func receive(_ json: UnsafePointer<CChar>) {
        let data = Data(bytes: json, count: strlen(json))
        guard let event = try? JSONDecoder().decode(LibraryEvent.self, from: data) else { return }
        handler(event)
    }
}
