import EglCore
import SwiftUI
import UniformTypeIdentifiers

/// Decrypts a passphrase `.zip.age` export to a ZIP and offers Save to Files (D-102).
struct DecryptAgeScreen: View {
    let open: OpenLibrary
    @State private var source: URL? = nil
    @State private var passphrase = ""
    @State private var picking = false
    @State private var working = false
    @State private var result: URL? = nil
    @State private var saving = false

    var body: some View {
        Form {
            if let result {
                Section {
                    Label("decrypt.done", systemImage: "checkmark.circle")
                        .foregroundStyle(.green)
                    Button {
                        saving = true
                    } label: {
                        Label("export.saveToFiles", systemImage: "folder")
                    }
                    .accessibilityIdentifier("decrypt.saveToFiles")
                    Button("decrypt.another") {
                        clean(result)
                        self.result = nil
                    }
                }
            } else {
                Section {
                    Button("decrypt.chooseArchive") { picking = true }
                        .accessibilityIdentifier("decrypt.choose")
                    if let source {
                        Text(verbatim: source.lastPathComponent)
                            .font(.footnote)
                            .foregroundStyle(EglTokens.mutedForeground)
                    }
                    SecureField("export.passphrase", text: $passphrase)
                    Button("decrypt.start") {
                        Task { await start() }
                    }
                    .disabled(!ready || working)
                    .accessibilityIdentifier("decrypt.start")
                } footer: {
                    Text("decrypt.hint")
                }
                if working {
                    Section {
                        ProgressView("decrypt.working")
                        Button("export.cancel", role: .destructive) {
                            let library = open.library
                            Task.detached { try? library.cancelDecryptAge() }
                        }
                    }
                }
            }
        }
        .navigationTitle("decrypt.title")
        .navigationBarTitleDisplayMode(.inline)
        .fileImporter(isPresented: $picking, allowedContentTypes: [.data], allowsMultipleSelection: false) { outcome in
            if case let .success(urls) = outcome, let url = urls.first {
                source = url
            }
        }
        .fileMover(isPresented: $saving, file: result) { outcome in
            if case .success = outcome {
                open.show(String(localized: "export.saved"), undoable: false)
            }
        }
        .onDisappear {
            if let result { clean(result) }
        }
    }

    private var ready: Bool {
        guard source != nil, passphrase.count >= 8 else { return false }
        return true
    }

    private func start() async {
        guard let source else { return }
        working = true
        defer { working = false }
        let access = source.startAccessingSecurityScopedResource()
        defer { if access { source.stopAccessingSecurityScopedResource() } }
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("decrypt-\(UUID().uuidString)", isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        } catch {
            open.report(error)
            return
        }
        let name = source.deletingPathExtension().lastPathComponent
        let zipName = name.lowercased().hasSuffix(".zip") ? name : "\(name).zip"
        let destination = folder.appendingPathComponent(zipName)
        let password = passphrase
        passphrase = ""
        let written = await open.perform {
            try $0.decryptAgeArchive(source: source, destination: destination, password: password)
        }
        if let written {
            result = written
        } else if open.error == nil {
            try? FileManager.default.removeItem(at: folder)
        }
    }

    private func clean(_ url: URL) {
        try? FileManager.default.removeItem(at: url.deletingLastPathComponent())
    }
}
