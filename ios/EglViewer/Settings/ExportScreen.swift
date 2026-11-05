import EglCore
import SwiftUI
import UIKit

/// Export (D-099): the whole library, folders or tags, as a folder tree, a ZIP or a 7z archive,
/// optionally encrypted with a passphrase: age or AES-256 for a ZIP (D-112), AES-256 with hidden
/// names for a 7z (D-113). It is written into the app's temporary
/// folder, then saved to Files or shared; the temporary copy goes when the screen closes.
struct ExportScreen: View {
    let open: OpenLibrary
    @State private var whole = true
    @State private var folders: Set<String> = []
    @State private var tags: Set<String> = []
    @State private var includeSubfolders = true
    @State private var kind: Kind = .zip
    @State private var zipEncryption: ZipEncryption = .age
    @State private var compression: ExportCompression = .balanced
    @State private var encrypt = false
    @State private var passphrase = ""
    @State private var confirm = ""
    @State private var vocabulary: [Tag] = []
    @State private var preview: ExportPreview? = nil
    @State private var exportID: UInt64? = nil
    @State private var folder: URL? = nil
    @State private var saving = false
    @State private var sharing: ShareFiles? = nil

    /// What the export writes.
    enum Kind: Hashable { case zip, sevenZip, folder }

    private var archive: Bool { kind != .folder }

    private var format: ExportFormat {
        switch kind {
        case .zip: .zip(compression, encryption: zipEncryption)
        case .sevenZip: .sevenZip(compression)
        case .folder: .mirror
        }
    }

    private var selection: ExportSelection {
        ExportSelection(whole: whole, folders: whole ? [] : Array(folders), tags: whole ? [] : Array(tags))
    }

    private var progress: ExportProgress? { exportID.flatMap { open.exports[$0] } }

    var body: some View {
        Form {
            if let progress {
                running(progress)
            } else {
                choices
            }
        }
        .navigationTitle("export.title")
        .navigationBarTitleDisplayMode(.inline)
        .task {
            vocabulary = await open.perform { try $0.tags() } ?? []
            await updatePreview()
        }
        .onChange(of: whole) { Task { await updatePreview() } }
        .onChange(of: folders) { Task { await updatePreview() } }
        .onChange(of: tags) { Task { await updatePreview() } }
        .onChange(of: includeSubfolders) { Task { await updatePreview() } }
        .onDisappear(perform: clean)
        .fileMover(isPresented: $saving, file: progress?.summary.map { URL(fileURLWithPath: $0.path) }) { result in
            if case .success = result {
                open.show(String(localized: "export.saved"), undoable: false)
                finish()
            }
        }
        .sheet(item: $sharing) { files in
            ShareSheet(files: files)
                .presentationDetents([.medium, .large])
        }
    }

    // MARK: Choosing

    @ViewBuilder
    private var choices: some View {
        Section {
            Toggle("export.whole", isOn: $whole)
                .accessibilityIdentifier("export.whole")
        } footer: {
            Text("export.wholeHint")
        }
        if !whole {
            Section("export.folders") {
                ForEach(open.pickableFolders, id: \.folder.id) { entry in
                    check(entry.folder.name, depth: entry.depth, on: folders.contains(entry.folder.id)) {
                        toggle(entry.folder.id, in: &folders)
                    }
                }
                Toggle("export.subfolders", isOn: $includeSubfolders)
            }
            Section("export.tags") {
                ForEach(vocabulary, id: \.name) { tag in
                    check(tag.name, depth: 0, on: tags.contains(tag.name)) {
                        toggle(tag.name, in: &tags)
                    }
                }
            }
        }
        Section("export.format") {
            Picker("export.format", selection: $kind) {
                Text("export.zip").tag(Kind.zip)
                Text("export.sevenZip").tag(Kind.sevenZip)
                Text("export.folder").tag(Kind.folder)
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("export.format")
            if archive {
                Picker("export.compression", selection: $compression) {
                    Text("export.compression.none").tag(ExportCompression.none)
                    Text("export.compression.fast").tag(ExportCompression.fast)
                    Text("export.compression.balanced").tag(ExportCompression.balanced)
                    Text("export.compression.smallest").tag(ExportCompression.smallest)
                }
                Toggle("export.encrypt", isOn: $encrypt)
                    .accessibilityIdentifier("export.encrypt")
                if encrypt, kind == .zip {
                    Picker("export.protection", selection: $zipEncryption) {
                        Text("export.protection.age").tag(ZipEncryption.age)
                        Text("export.protection.aes").tag(ZipEncryption.aes)
                    }
                    .accessibilityIdentifier("export.protection")
                }
                if encrypt {
                    SecureField("export.passphrase", text: $passphrase)
                    SecureField("export.confirmPassphrase", text: $confirm)
                }
            }
        }
        Section {
            if let preview {
                let size = ByteCountFormatter.string(fromByteCount: Int64(preview.bytes), countStyle: .file)
                (Text("export.summary \(preview.items)") + Text(verbatim: " · \(size)"))
                    .accessibilityIdentifier("export.preview")
                if preview.hidden > 0 {
                    Text("export.hidden \(preview.hidden)")
                        .font(.footnote)
                        .foregroundStyle(EglTokens.mutedForeground)
                }
            }
            Button("export.start") {
                Task { await start() }
            }
            .disabled(!ready)
            .accessibilityIdentifier("export.start")
        } footer: {
            if archive, encrypt {
                if !passphrase.isEmpty, passphrase.count < minPassphraseCharacters {
                    Text("export.passphraseTooShort \(minPassphraseCharacters)")
                        .foregroundStyle(.orange)
                } else if kind == .sevenZip {
                    Text("export.sevenZipHint")
                } else if zipEncryption == .aes {
                    Text("export.aesHint")
                } else {
                    Text("export.encryptHint")
                }
            }
        }
    }

    private var ready: Bool {
        guard !selection.isEmpty, (preview?.items ?? 0) > 0 else { return false }
        if archive, encrypt {
            return passphrase.count >= minPassphraseCharacters && passphrase == confirm
        }
        return true
    }

    private func check(_ name: String, depth: Int, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack {
                Text(verbatim: name)
                    .foregroundStyle(EglTokens.foreground)
                    .padding(.leading, CGFloat(depth) * 16)
                Spacer()
                if on {
                    Image(systemName: "checkmark").foregroundStyle(EglTokens.accent)
                }
            }
        }
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private func toggle(_ value: String, in set: inout Set<String>) {
        if set.contains(value) { set.remove(value) } else { set.insert(value) }
    }

    private func updatePreview() async {
        let selection = selection
        guard !selection.isEmpty else {
            preview = nil
            return
        }
        let include = includeSubfolders
        preview = await open.perform { try $0.exportPreview(selection, includeSubfolders: include) }
    }

    private func start() async {
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("export-\(UUID().uuidString)", isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        } catch {
            open.report(error)
            return
        }
        self.folder = folder
        let request = ExportRequest(
            selection: selection, includeSubfolders: includeSubfolders,
            format: format, destination: folder.path,
            encrypt: archive && encrypt ? passphrase : nil
        )
        passphrase = ""
        confirm = ""
        if let started = await open.startExport(request) {
            exportID = started.id
        } else {
            clean()
        }
    }

    // MARK: Running and done

    @ViewBuilder
    private func running(_ progress: ExportProgress) -> some View {
        Section {
            switch progress.state {
            case .preparing, .copying, .encrypting:
                ProgressView(value: Double(progress.done), total: Double(max(progress.total, 1))) {
                    stateText(progress.state)
                }
                Text(verbatim: "\(progress.done) / \(progress.total)")
                    .font(.footnote.monospacedDigit())
                    .foregroundStyle(EglTokens.mutedForeground)
                Button("export.cancel", role: .destructive) {
                    let library = open.library
                    Task.detached { try? library.cancelExport(progress.id) }
                }
            case .done:
                Label("export.done", systemImage: "checkmark.circle")
                    .foregroundStyle(.green)
                    .accessibilityIdentifier("export.done")
                if let summary = progress.summary {
                    let size = ByteCountFormatter.string(fromByteCount: Int64(summary.bytes), countStyle: .file)
                    Text("export.written \(summary.items)") + Text(verbatim: " · \(size)")
                    if !summary.missing.isEmpty {
                        Text("export.missing \(summary.missing.count)")
                            .font(.footnote)
                            .foregroundStyle(.orange)
                    }
                    Button {
                        saving = true
                    } label: {
                        Label("export.saveToFiles", systemImage: "folder")
                    }
                    .accessibilityIdentifier("export.saveToFiles")
                    Button {
                        sharing = ShareFiles(folder: folder ?? URL(fileURLWithPath: summary.path), urls: [URL(fileURLWithPath: summary.path)])
                    } label: {
                        Label("action.share", systemImage: "square.and.arrow.up")
                    }
                    .accessibilityIdentifier("export.share")
                }
                Button("export.another", action: finish)
            case .failed:
                Label("export.failed", systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.red)
                Text(verbatim: progress.error ?? "")
                    .font(.footnote)
                Button("export.another", action: finish)
            case .cancelled:
                Label("export.cancelled", systemImage: "xmark.circle")
                Button("export.another", action: finish)
            }
        }
    }

    private func stateText(_ state: ExportState) -> Text {
        switch state {
        case .preparing: Text("export.preparing")
        case .copying: Text("export.copying")
        case .encrypting: Text("export.encrypting")
        default: Text("export.title")
        }
    }

    /// Back to the choices; what was written goes.
    private func finish() {
        clean()
        if let exportID { open.forgetExport(exportID) }
        exportID = nil
    }

    private func clean() {
        if let folder {
            try? FileManager.default.removeItem(at: folder)
        }
        folder = nil
    }
}
