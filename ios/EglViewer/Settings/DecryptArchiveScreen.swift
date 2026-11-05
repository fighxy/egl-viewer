import EglCore
import SwiftUI
import UniformTypeIdentifiers

/// Decrypt Archive (D-110): opens an encrypted export (`name.zip.age`) with its passphrase
/// into an ordinary ZIP in the app's temporary folder, then saves it to Files or shares it.
/// The temporary copy goes when the screen closes; the passphrase is never stored.
struct DecryptArchiveScreen: View {
    @State private var archive: URL? = nil
    @State private var picking = false
    @State private var passphrase = ""
    @State private var busy = false
    @State private var error: String? = nil
    @State private var result: DecryptedArchive? = nil
    @State private var folder: URL? = nil
    @State private var saving = false
    @State private var sharing: ShareFiles? = nil

    var body: some View {
        Form {
            Section {
                Button {
                    picking = true
                } label: {
                    LabeledContent("decrypt.archive") {
                        Text(verbatim: archive?.lastPathComponent ?? "—")
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                }
                .accessibilityIdentifier("decrypt.choose")
                SecureField("export.passphrase", text: $passphrase)
                    .accessibilityIdentifier("decrypt.passphrase")
            } footer: {
                Text("decrypt.hint")
            }
            Section {
                Button {
                    Task { await decrypt() }
                } label: {
                    if busy {
                        HStack {
                            ProgressView()
                            Text("decrypt.decrypting")
                        }
                    } else {
                        Text("decrypt.start")
                    }
                }
                .disabled(archive == nil || passphrase.isEmpty || busy)
                .accessibilityIdentifier("decrypt.start")
                if let error {
                    Text(verbatim: error)
                        .font(.footnote)
                        .foregroundStyle(.red)
                        .accessibilityIdentifier("decrypt.error")
                }
            }
            if let result {
                Section {
                    let size = ByteCountFormatter.string(fromByteCount: Int64(result.bytes), countStyle: .file)
                    Label {
                        Text(verbatim: "\(result.url.lastPathComponent) · \(size)")
                    } icon: {
                        Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
                    }
                    .accessibilityIdentifier("decrypt.done")
                    Button {
                        saving = true
                    } label: {
                        Label("export.saveToFiles", systemImage: "folder")
                    }
                    .accessibilityIdentifier("decrypt.saveToFiles")
                    Button {
                        sharing = ShareFiles(folder: folder ?? result.url, urls: [result.url])
                    } label: {
                        Label("action.share", systemImage: "square.and.arrow.up")
                    }
                    .accessibilityIdentifier("decrypt.share")
                }
            }
        }
        .navigationTitle("decrypt.title")
        .navigationBarTitleDisplayMode(.inline)
        .fileImporter(isPresented: $picking, allowedContentTypes: [.item]) { picked in
            if case let .success(url) = picked {
                archive = url
                error = nil
                result = nil
            }
        }
        .fileMover(isPresented: $saving, file: result?.url) { moved in
            if case .success = moved {
                result = nil
                clean()
            }
        }
        .sheet(item: $sharing) { files in
            ShareSheet(files: files)
                .presentationDetents([.medium, .large])
        }
        .onDisappear(perform: clean)
    }

    private func decrypt() async {
        guard let archive else { return }
        busy = true
        error = nil
        result = nil
        clean()
        let target = FileManager.default.temporaryDirectory
            .appendingPathComponent("decrypt-\(UUID().uuidString)", isDirectory: true)
        let secret = passphrase
        let outcome: Result<DecryptedArchive, Error> = await Task.detached {
            Result {
                try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
                // A file picked in Files is readable while its security scope is held.
                let scoped = archive.startAccessingSecurityScopedResource()
                defer { if scoped { archive.stopAccessingSecurityScopedResource() } }
                return try Library.decryptArchive(at: archive, into: target, passphrase: secret)
            }
        }.value
        busy = false
        switch outcome {
        case let .success(decrypted):
            folder = target
            result = decrypted
            passphrase = ""
        case let .failure(failure):
            try? FileManager.default.removeItem(at: target)
            error = (failure as? EglError)?.message ?? failure.localizedDescription
        }
    }

    /// Removes the temporary copy.
    private func clean() {
        if let folder {
            try? FileManager.default.removeItem(at: folder)
        }
        folder = nil
    }
}
