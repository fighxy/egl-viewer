import EglCore
import PhotosUI
import SwiftUI

/// Where the user adds from (D-096).
enum AddSource: Identifiable {
    case photos, files
    var id: Self { self }
}

/// The + menu: Photos, Files, the clipboard. It only sets `source`; the pickers live on the
/// screen (`addPickers`), since a menu in a toolbar is a poor host for presentations.
struct AddMenu: View {
    let open: OpenLibrary
    let folder: String?
    @Binding var source: AddSource?

    var body: some View {
        Menu {
            Button {
                source = .photos
            } label: {
                Label("add.photos", systemImage: "photo.on.rectangle.angled")
            }
            .accessibilityIdentifier("add.photos")
            Button {
                source = .files
            } label: {
                Label("add.files", systemImage: "folder")
            }
            .accessibilityIdentifier("add.files")
            Button {
                let folder = folder
                Task { await open.add(await Clipboard.files(), into: folder) }
            } label: {
                Label("add.paste", systemImage: "doc.on.clipboard")
            }
            .disabled(!Clipboard.hasSomething)
            .accessibilityIdentifier("add.paste")
        } label: {
            Label("add.title", systemImage: "plus")
        }
        .accessibilityIdentifier("add.menu")
    }
}

/// The Photos and Files pickers for `AddMenu`, the duplicate question, and drops.
private struct AddPickers: ViewModifier {
    let open: OpenLibrary
    let folder: String?
    @Binding var source: AddSource?
    @State private var items: [PhotosPickerItem] = []
    @State private var loading = false

    func body(content: Content) -> some View {
        content
            .photosPicker(
                isPresented: Binding(get: { source == .photos }, set: { if !$0 { source = nil } }),
                selection: $items,
                matching: .any(of: [.images, .videos]),
                preferredItemEncoding: .current
            )
            .fileImporter(
                isPresented: Binding(get: { source == .files }, set: { if !$0 { source = nil } }),
                allowedContentTypes: [.image, .movie, .folder],
                allowsMultipleSelection: true
            ) { result in
                let folder = folder
                switch result {
                case let .success(urls):
                    Task { await open.add(picked: urls, into: folder) }
                case let .failure(error):
                    open.report(error)
                }
            }
            .onChange(of: items) { _, picked in
                guard !picked.isEmpty else { return }
                items = []
                Task { await addPhotos(picked) }
            }
            .overlay {
                if loading {
                    ProgressView("add.preparing")
                        .padding(20)
                        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
                        .accessibilityIdentifier("add.preparing")
                }
            }
    }

    /// Photos hands over each item as a file (HEIC stays HEIC); the copies are added together.
    private func addPhotos(_ picked: [PhotosPickerItem]) async {
        loading = true
        var files: [PickedFile] = []
        for item in picked {
            if let file = try? await item.loadTransferable(type: PickedFile.self) {
                files.append(file)
            }
        }
        loading = false
        let unreadable = picked.count - files.count
        await open.add(files, into: folder)
        if unreadable > 0 {
            open.show(String(localized: "add.unreadable \(unreadable)"), undoable: false)
        }
    }
}

/// What an import is doing: adding files, or copying an Eagle library (D-116).
struct ImportLabel: View {
    let progress: ImportProgress

    var body: some View {
        if progress.kind == .eagle {
            Text("eagle.importing \(progress.processed) \(progress.total)")
        } else {
            Text("library.adding \(progress.processed) \(progress.total)")
        }
    }
}

/// How an Eagle import ended: what came in, what did not, and the locked folders that came in
/// without a password (D-116).
struct EagleOutcomeAlert: ViewModifier {
    let open: OpenLibrary

    func body(content: Content) -> some View {
        let title: LocalizedStringKey = open.eagleOutcome?.cancelled == true ? "eagle.stopped" : "eagle.done"
        return content.alert(
            title,
            isPresented: Binding(get: { open.eagleOutcome != nil }, set: { if !$0 { open.eagleOutcome = nil } }),
            presenting: open.eagleOutcome
        ) { _ in
            Button("common.ok") { open.eagleOutcome = nil }
        } message: { outcome in
            Text(Self.message(outcome))
        }
    }

    static func message(_ outcome: EagleOutcome) -> String {
        let report = outcome.report
        var lines = [String(localized: "eagle.imported \(report.imported)")]
        if report.duplicates > 0 {
            lines.append(String(localized: "eagle.duplicates \(report.duplicates)"))
        }
        if !report.skipped.isEmpty {
            lines.append(String(localized: "eagle.skipped \(report.skipped.count)"))
        }
        if report.trashSkipped > 0 {
            lines.append(String(localized: "eagle.trashLeft \(report.trashSkipped)"))
        }
        if !report.lockedFolders.isEmpty {
            lines.append(String(localized: "eagle.locked") + " " + report.lockedFolders.joined(separator: ", "))
        }
        if outcome.cancelled {
            lines.append(String(localized: "eagle.resume"))
        }
        return lines.joined(separator: "\n")
    }
}

/// While files are added: their progress, with a way to stop.
struct ImportBanner: View {
    let open: OpenLibrary

    var body: some View {
        if !open.imports.isEmpty {
            VStack(spacing: 6) {
                ForEach(open.imports.values.sorted { $0.id < $1.id }) { progress in
                    HStack(spacing: 12) {
                        ProgressView(value: Double(progress.processed), total: Double(max(progress.total, 1))) {
                            ImportLabel(progress: progress)
                                .font(.caption)
                        }
                        Button {
                            let library = open.library
                            Task.detached { try? library.cancelImport(progress.id) }
                        } label: {
                            Label("add.stop", systemImage: "xmark.circle.fill")
                                .labelStyle(.iconOnly)
                        }
                        .buttonStyle(.borderless)
                        .accessibilityIdentifier("add.stop")
                    }
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(.bar)
        }
    }
}

/// The duplicate an import waits on: skip it, add it anyway, add the existing item to the
/// folder instead, or stop. The answer holds for the rest of the batch.
private struct DuplicateQuestion: ViewModifier {
    let open: OpenLibrary

    func body(content: Content) -> some View {
        content.alert(
            "duplicate.title",
            isPresented: Binding(get: { open.duplicateQuestion != nil }, set: { _ in }),
            presenting: open.duplicateQuestion
        ) { question in
            Button("duplicate.skip") { Task { await open.answer(.skip) } }
            Button("duplicate.addAnyway") { Task { await open.answer(.addAnyway) } }
            if question.duplicate?.targetFolder != nil {
                Button("duplicate.addToFolder") { Task { await open.answer(.addToFolder) } }
            }
            Button("duplicate.stop", role: .cancel) { Task { await open.answer(.cancel) } }
        } message: { question in
            let name = URL(fileURLWithPath: question.duplicate?.path ?? "").lastPathComponent
            Text(verbatim: "\(name)\n") + (question.duplicate?.inTrash == true
                ? Text("duplicate.inTrash")
                : Text("duplicate.message"))
        }
    }
}

extension View {
    /// The pickers of the + menu, adding into `folder`, and drops of files onto this view.
    func addPickers(_ open: OpenLibrary, folder: String?, source: Binding<AddSource?>) -> some View {
        modifier(AddPickers(open: open, folder: folder, source: source))
            .dropDestination(for: PickedFile.self) { files, _ in
                guard !open.readOnly, !files.isEmpty else { return false }
                Task { await open.add(files, into: folder) }
                return true
            }
    }

    func duplicateQuestion(_ open: OpenLibrary) -> some View {
        modifier(DuplicateQuestion(open: open))
    }

    func eagleOutcome(_ open: OpenLibrary) -> some View {
        modifier(EagleOutcomeAlert(open: open))
    }
}
