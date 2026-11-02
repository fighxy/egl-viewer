import Foundation

#if EGL_HAS_FFI
/// Live session over `crates/library-ffi` (C ABI). Enabled only when the Xcode
/// target defines `EGL_HAS_FFI` and links `libegl_library.a`.
final class NativeFfiSession {
    enum SessionError: LocalizedError {
        case openFailed(String)
        case status(String)

        var errorDescription: String? {
            switch self {
            case let .openFailed(message): message
            case let .status(message): message
            }
        }
    }

    private var handle: OpaquePointer?

    deinit {
        close()
    }

    func open(path: String, syncFolder: Bool) throws {
        close()
        handle = path.withCString { pathPtr in
            LibraryBridge.appVersion.withCString { versionPtr in
                egl_open(pathPtr, versionPtr, syncFolder ? 1 : 0)
            }
        }
        if handle == nil {
            let message = egl_last_error_global().map { String(cString: $0) } ?? "open failed"
            throw SessionError.openFailed(message)
        }
    }

    func close() {
        if let handle {
            egl_close(handle)
        }
        handle = nil
    }

    func libraryInfo() throws -> LibraryInfoDTO {
        try decode(LibraryInfoDTO.self) { out in egl_library_info(requireHandle(), out) }
    }

    func viewWindow(scopeJson: String, start: UInt32, len: UInt32) throws -> ViewWindowDTO {
        try decode(ViewWindowDTO.self) { out in
            scopeJson.withCString { scopePtr in
                egl_view_window(requireHandle(), scopePtr, start, len, out)
            }
        }
    }

    func item(id: String) throws -> ItemDTO {
        try decode(ItemDTO.self) { out in
            id.withCString { idPtr in
                egl_item(requireHandle(), idPtr, out)
            }
        }
    }

    func folderTree() throws -> [FolderDTO] {
        try decode([FolderDTO].self) { out in egl_folder_tree(requireHandle(), out) }
    }

    func tags() throws -> [TagDTO] {
        try decode([TagDTO].self) { out in egl_tags(requireHandle(), out) }
    }

    func applyAddTag(itemId: String, tag: String) throws {
        let escaped = tag
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        let edit = "{\"type\":\"addTags\",\"value\":[\"\(escaped)\"]}"
        _ = try decode(ItemDTO.self) { out in
            itemId.withCString { idPtr in
                edit.withCString { editPtr in
                    egl_apply_item_edit(requireHandle(), idPtr, editPtr, out)
                }
            }
        }
    }

    private func requireHandle() -> OpaquePointer {
        guard let handle else {
            fatalError("NativeFfiSession used before open")
        }
        return handle
    }

    private func decode<T: Decodable>(
        _ type: T.Type,
        _ call: (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> EglStatus
    ) throws -> T {
        var out: UnsafeMutablePointer<CChar>?
        let status = withUnsafeMutablePointer(to: &out, call)
        defer {
            if let out {
                egl_string_free(out)
            }
        }
        guard status == EGL_OK, let out else {
            let message = egl_last_error(handle).map { String(cString: $0) } ?? "FFI call failed"
            throw SessionError.status(message)
        }
        let json = String(cString: out)
        guard let data = json.data(using: .utf8) else {
            throw SessionError.status("invalid UTF-8 from FFI")
        }
        return try JSONDecoder().decode(type, from: data)
    }
}
#endif
