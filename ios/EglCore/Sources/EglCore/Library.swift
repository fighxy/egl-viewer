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

    public init(code: Code, message: String) {
        self.code = code
        self.message = message
    }

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
    /// Opens an age archive (an encrypted export, `name.zip.age`) with its passphrase into
    /// the existing folder `destination`, as `name.zip` (D-110). Needs no open library.
    /// Blocking: call it off the main thread.
    public static func decryptArchive(at source: URL, into destination: URL, passphrase: String) throws
        -> DecryptedArchive
    {
        var out: UnsafeMutablePointer<CChar>?
        let status = source.path.withCString { source in
            destination.path.withCString { destination in
                passphrase.withCString { passphrase in
                    egl_decrypt_archive(source, destination, passphrase, &out)
                }
            }
        }
        guard status == EGL_OK, let out else { throw globalError(status) }
        defer { egl_string_free(out) }
        return try JSONDecoder().decode(DecryptedArchive.self, from: Data(String(cString: out).utf8))
    }

    /// The name, item count and size of the Eagle library folder at `url` (D-116); writes
    /// nothing. Throws `notALibrary` for a folder that is not one. Walks the originals for the
    /// size: call it off the main thread.
    public static func eagleLibraryInfo(at url: URL) throws -> EagleLibrarySummary {
        var out: UnsafeMutablePointer<CChar>?
        let status = url.path.withCString { egl_eagle_library_info($0, &out) }
        guard status == EGL_OK, let out else { throw globalError(status) }
        defer { egl_string_free(out) }
        return try JSONDecoder().decode(EagleLibrarySummary.self, from: Data(String(cString: out).utf8))
    }

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

    // MARK: Maintenance

    public func stats() throws -> LibraryStats { try json { egl_library_stats($0, $1) } }

    /// Newest first.
    public func backups() throws -> [Backup] { try json { egl_backups($0, $1) } }

    @discardableResult
    public func backUpNow() throws -> Backup { try json { egl_back_up_now($0, $1) } }

    /// Puts a backup's database back (the current one is kept as a backup first). Views and
    /// undo start over.
    @discardableResult
    public func restoreBackup(_ name: String) throws -> LibraryInfo {
        try name.withCString { name in try json { egl_restore_backup($0, name, $1) } }
    }

    /// Reads only.
    public func check() throws -> LibraryCheck { try json { egl_check_library($0, $1) } }

    public func repair(_ options: RepairOptions) throws -> LibraryCheck {
        let options = try encode(options)
        return try options.withCString { options in try json { egl_repair_library($0, options, $1) } }
    }

    /// Makes missing thumbnails in the background; `rebuild` makes all of them again.
    @discardableResult
    public func generatePreviews(rebuild: Bool) throws -> PreviewsTicket {
        try json { egl_generate_previews($0, rebuild ? 1 : 0, $1) }
    }

    // MARK: Export

    public func exportPreview(_ selection: ExportSelection, includeSubfolders: Bool) throws -> ExportPreview {
        let selection = try encode(selection)
        return try selection.withCString { selection in
            try json { egl_export_preview($0, selection, includeSubfolders ? 1 : 0, $1) }
        }
    }

    /// Starts an export in the background; progress comes as events.
    public func startExport(_ request: ExportRequest) throws -> ExportProgress {
        let request = try encode(request)
        return try request.withCString { request in try json { egl_start_export($0, request, $1) } }
    }

    public func exports() throws -> [ExportProgress] { try json { egl_exports($0, $1) } }

    public func cancelExport(_ id: UInt64) throws {
        try check(egl_cancel_export(try live(), id))
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

    /// The tags of a selection, counted within its view.
    public func selectionTags(_ target: ItemsTarget) throws -> SelectionTags {
        let target = try encode(target)
        return try target.withCString { target in try json { egl_view_selection_tags($0, target, $1) } }
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
        try saveFolder(.create(name: name, parent: parent))
    }

    /// One edit of the folder tree; returns the new tree.
    @discardableResult
    public func saveFolder(_ change: FolderChange) throws -> [Folder] {
        let change = try encode(change)
        return try change.withCString { change in try json { egl_save_folder($0, change, $1) } }
    }

    /// Deletes a folder with its subfolders; `trashItems` also moves their items to the trash.
    @discardableResult
    public func deleteFolder(_ id: String, trashItems: Bool) throws -> [Folder] {
        try id.withCString { id in try json { egl_delete_folder($0, id, trashItems ? 1 : 0, $1) } }
    }

    /// Renames a tag on every item; onto an existing name the two merge.
    public func renameTag(_ from: String, to: String) throws {
        let lib = try live()
        try check(from.withCString { from in to.withCString { egl_rename_tag(lib, from, $0) } })
    }

    /// Moves `source` onto `target` on every item.
    public func mergeTag(_ source: String, into target: String) throws {
        let lib = try live()
        try check(source.withCString { source in target.withCString { egl_merge_tags(lib, source, $0) } })
    }

    /// Removes a tag from every item.
    public func deleteTag(_ name: String) throws {
        let lib = try live()
        try check(name.withCString { egl_delete_tag(lib, $0) })
    }

    public func tagGroups() throws -> [TagGroup] { try json { egl_tag_groups($0, $1) } }

    @discardableResult
    public func saveTagGroup(_ change: TagGroupChange) throws -> [TagGroup] {
        let change = try encode(change)
        return try change.withCString { change in try json { egl_save_tag_group($0, change, $1) } }
    }

    @discardableResult
    public func deleteTagGroup(_ id: String) throws -> [TagGroup] {
        try id.withCString { id in try json { egl_delete_tag_group($0, id, $1) } }
    }

    @discardableResult
    public func saveSmartFolder(_ change: SmartFolderChange) throws -> [SmartFolder] {
        let change = try encode(change)
        return try change.withCString { change in try json { egl_save_smart_folder($0, change, $1) } }
    }

    @discardableResult
    public func deleteSmartFolder(_ id: String) throws -> [SmartFolder] {
        try id.withCString { id in try json { egl_delete_smart_folder($0, id, $1) } }
    }

    /// The rules of `query`, for "Save as smart folder".
    public func smartRules(of query: LibraryQuery) throws -> SmartRulesDraft {
        let query = try encode(query)
        return try query.withCString { query in try json { egl_smart_rules($0, query, $1) } }
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

    /// Imports the Eagle library folder at `url` into this library in the background
    /// (D-116); progress comes as events like adding files, the last one with the report.
    /// `underRootFolder` puts the Eagle folders under a new folder named after it.
    public func importEagle(at url: URL, importTrash: Bool = false, underRootFolder: Bool = false) throws
        -> ImportTicket
    {
        struct Request: Encodable {
            let path: String
            let importTrash: Bool
            let underRootFolder: Bool
        }
        let request = try encode(Request(path: url.path, importTrash: importTrash, underRootFolder: underRootFolder))
        return try request.withCString { request in try json { egl_import_eagle($0, request, $1) } }
    }

    /// The progress of import `id`; nil while it has not reported yet.
    public func importStatus(_ id: UInt64) throws -> ImportProgress? {
        try json { egl_import_status($0, id, $1) }
    }

    /// Answers the duplicate question of import `id`.
    public func answerImport(_ id: UInt64, _ answer: DuplicateAnswer) throws {
        let lib = try live()
        try check(answer.rawValue.withCString { egl_answer_import(lib, id, $0) })
    }

    public func cancelImport(_ id: UInt64) throws {
        try check(egl_cancel_import(try live(), id))
    }

    // MARK: Locked folders

    /// Locks a folder (or sets the first password of a lock imported without one) and returns
    /// its recovery key, to be shown once. The label and its colour stay readable while locked.
    public func lockFolder(_ id: String, password: String, label: String? = nil, labelColor: String? = nil) throws -> RecoveryKey {
        struct Request: Encodable {
            let password: String
            let label: String?
            let labelColor: String?
        }
        let request = try encode(Request(password: password, label: label, labelColor: labelColor))
        return try id.withCString { id in
            try request.withCString { request in try json { egl_lock_folder($0, id, request, $1) } }
        }
    }

    /// Opens a folder with its recovery key or phrase and sets a new password.
    public func recoverFolder(_ id: String, recovery: String, newPassword: String) throws {
        let lib = try live()
        try check(id.withCString { id in
            recovery.withCString { recovery in
                newPassword.withCString { egl_recover_folder(lib, id, recovery, $0) }
            }
        })
    }

    public func changeFolderPassword(_ id: String, current: String, new: String) throws {
        let lib = try live()
        try check(id.withCString { id in
            current.withCString { current in new.withCString { egl_change_folder_password(lib, id, current, $0) } }
        })
    }

    /// A new recovery key; the old one stops working.
    public func renewRecoveryKey(_ id: String, password: String) throws -> RecoveryKey {
        try id.withCString { id in
            try password.withCString { password in try json { egl_renew_recovery_key($0, id, password, $1) } }
        }
    }

    /// Decrypts the folder and takes the lock off ("" for a lock imported without password).
    public func removeFolderLock(_ id: String, password: String) throws {
        let lib = try live()
        try check(id.withCString { id in password.withCString { egl_remove_folder_lock(lib, id, $0) } })
    }

    /// The plaintext label of a locked folder; nil clears a part.
    public func setLockLabel(_ id: String, label: String?, color: String?) throws {
        let lib = try live()
        let status: EglStatus = id.withCString { id in
            Self.withOptionalCString(label) { label in
                Self.withOptionalCString(color) { egl_set_lock_label(lib, id, label, $0) }
            }
        }
        try check(status)
    }

    private static func withOptionalCString<T>(_ text: String?, _ body: (UnsafePointer<CChar>?) -> T) -> T {
        guard let text else { return body(nil) }
        return text.withCString { body($0) }
    }

    // MARK: YubiKey (D-126)

    /// The YubiKeys bound to locked folder `id`.
    public func pivWraps(_ id: String) throws -> [PivWrap] {
        try id.withCString { id in try json { egl_piv_wraps($0, id, $1) } }
    }

    /// What card `serial` must compute to open folder `id`.
    public func pivChallenge(_ id: String, serial: String) throws -> PivChallenge {
        struct Wire: Decodable {
            let slot: UInt8
            let policy: PivPolicy
            let ephemeral: String
        }
        let wire: Wire = try id.withCString { id in
            try serial.withCString { serial in try json { egl_piv_challenge($0, id, serial, $1) } }
        }
        guard let ephemeral = Data(hex: wire.ephemeral) else {
            throw EglError(code: .other, message: "unreadable YubiKey challenge")
        }
        return PivChallenge(slot: wire.slot, policy: wire.policy, ephemeral: ephemeral)
    }

    /// Opens folder `id` with the card's 32-byte ECDH answer to `pivChallenge`.
    public func unlockWithPiv(_ id: String, serial: String, sharedSecret: Data) throws {
        let lib = try live()
        let shared = sharedSecret.hexString
        try check(id.withCString { id in
            serial.withCString { serial in shared.withCString { egl_unlock_with_piv(lib, id, serial, $0) } }
        })
    }

    /// Binds the key a card made in retired `slot` (SEC1 `publicKey`) to folder `id`.
    public func bindPiv(
        _ id: String, password: String, serial: String, slot: UInt8, policy: PivPolicy, publicKey: Data
    ) throws {
        struct Request: Encodable {
            let password: String
            let serial: String
            let slot: UInt8
            let policy: PivPolicy
            let publicKey: String
        }
        let request = try encode(Request(
            password: password, serial: serial, slot: slot, policy: policy, publicKey: publicKey.hexString
        ))
        let lib = try live()
        try check(id.withCString { id in request.withCString { egl_bind_piv(lib, id, $0) } })
    }

    /// Drops card `serial` from folder `id`; the password still opens it.
    public func unbindPiv(_ id: String, password: String, serial: String) throws {
        let lib = try live()
        try check(id.withCString { id in
            password.withCString { password in serial.withCString { egl_unbind_piv(lib, id, password, $0) } }
        })
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

extension Data {
    /// Lowercase hex, as the C ABI passes bytes (D-126).
    var hexString: String { map { String(format: "%02x", $0) }.joined() }

    /// Bytes from hex; nil for an odd length or a character that is not hex.
    init?(hex: String) {
        let digits = Array(hex.utf8)
        guard digits.count % 2 == 0 else { return nil }
        var bytes = [UInt8]()
        bytes.reserveCapacity(digits.count / 2)
        func nibble(_ c: UInt8) -> UInt8? {
            switch c {
            case 48...57: return c - 48
            case 97...102: return c - 87
            case 65...70: return c - 55
            default: return nil
            }
        }
        var index = 0
        while index < digits.count {
            guard let high = nibble(digits[index]), let low = nibble(digits[index + 1]) else { return nil }
            bytes.append(high << 4 | low)
            index += 2
        }
        self.init(bytes)
    }
}
