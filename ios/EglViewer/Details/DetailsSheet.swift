import EglCore
import SwiftUI

/// An item's details, as the desktop's right bar (D-095): name, rating, tags, folders, notes,
/// link and the file's facts. Text fields save when the user submits or leaves the sheet.
struct DetailsSheet: View {
    let cell: ItemCell
    @State private var model: ItemDetailsModel
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var notes = ""
    @State private var link = ""
    @State private var newTag = ""
    @State private var filing = false

    init(open: OpenLibrary, cell: ItemCell) {
        self.cell = cell
        _model = State(initialValue: ItemDetailsModel(open: open, id: cell.id))
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    HStack {
                        Spacer()
                        ItemThumbnail(cell: cell, cache: model.open.thumbnails, pixels: 480)
                            .frame(height: 160)
                        Spacer()
                    }
                    .listRowBackground(Color.clear)
                }
                if let item = model.item {
                    editing(item)
                    facts(item)
                } else {
                    ProgressView()
                        .frame(maxWidth: .infinity)
                }
            }
            .navigationTitle("details.title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.done") {
                        Task {
                            await save()
                            dismiss()
                        }
                    }
                    .accessibilityIdentifier("details.done")
                }
            }
            .task {
                await model.load()
                name = model.item?.name ?? ""
                notes = model.item?.annotation ?? ""
                link = model.item?.url ?? ""
            }
            .onDisappear {
                Task { await save() }
            }
            .sheet(isPresented: $filing) {
                FolderPicker(open: model.open, title: "details.folders", chosen: Set(model.item?.folders ?? [])) { folder in
                    Task { await model.toggleFolder(folder) }
                }
            }
        }
        .presentationDetents([.large])
    }

    @ViewBuilder
    private func editing(_ item: Item) -> some View {
        Section("details.name") {
            TextField("details.name", text: $name)
                .submitLabel(.done)
                .onSubmit { Task { await model.rename(name) } }
                .accessibilityIdentifier("details.name")
        }
        .disabled(model.readOnly)

        Section("details.rating") {
            HStack(spacing: 14) {
                ForEach(1...5, id: \.self) { stars in
                    Button {
                        Task { await model.rate(stars) }
                    } label: {
                        Image(systemName: stars <= item.star ? "star.fill" : "star")
                            .font(.title2)
                            .foregroundStyle(stars <= item.star ? EglTokens.accent : EglTokens.mutedForeground)
                    }
                    .buttonStyle(.borderless)
                    .accessibilityLabel(Text("details.stars \(stars)"))
                    .accessibilityAddTraits(stars <= item.star ? .isSelected : [])
                    .accessibilityIdentifier("details.star.\(stars)")
                }
            }
        }
        .disabled(model.readOnly)

        Section("details.tags") {
            ForEach(item.tags, id: \.self) { tag in
                HStack {
                    Text(verbatim: tag)
                    Spacer()
                    Button(role: .destructive) {
                        Task { await model.removeTag(tag) }
                    } label: {
                        Label("action.removeTag", systemImage: "minus.circle")
                            .labelStyle(.iconOnly)
                    }
                    .buttonStyle(.borderless)
                    .accessibilityIdentifier("details.removeTag.\(tag)")
                }
            }
            TagEntry(text: $newTag, vocabulary: model.vocabulary, taken: Set(item.tags)) { name in
                Task { await model.addTag(name) }
            }
        }
        .disabled(model.readOnly)

        Section("details.folders") {
            ForEach(model.folderNames, id: \.id) { folder in
                HStack {
                    Label {
                        Text(verbatim: folder.name)
                    } icon: {
                        Image(systemName: "folder")
                    }
                    Spacer()
                    Button(role: .destructive) {
                        Task { await model.toggleFolder(folder.id) }
                    } label: {
                        Label("action.removeFromFolder", systemImage: "minus.circle")
                            .labelStyle(.iconOnly)
                    }
                    .buttonStyle(.borderless)
                }
            }
            Button("action.addToFolder") { filing = true }
                .accessibilityIdentifier("details.addToFolder")
        }
        .disabled(model.readOnly)

        Section("details.notes") {
            TextField("details.notes", text: $notes, axis: .vertical)
                .lineLimit(2...8)
                .accessibilityIdentifier("details.notes")
        }
        .disabled(model.readOnly)

        Section("details.link") {
            TextField("details.link", text: $link)
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .submitLabel(.done)
                .onSubmit { Task { await model.setLink(link) } }
                .accessibilityIdentifier("details.link")
            if let url = URL(string: item.url), url.scheme == "http" || url.scheme == "https" {
                Link(destination: url) {
                    Label("details.openLink", systemImage: "safari")
                }
            }
        }
        .disabled(model.readOnly)
    }

    @ViewBuilder
    private func facts(_ item: Item) -> some View {
        Section("details.info") {
            LabeledContent("details.type") {
                Text(verbatim: item.ext.uppercased())
            }
            if let width = item.width, let height = item.height {
                LabeledContent("details.dimensions") {
                    Text(verbatim: "\(width) × \(height)")
                }
            }
            if let size = item.size {
                LabeledContent("details.size") {
                    Text(verbatim: ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file))
                }
            }
            if let duration = item.duration, duration > 0 {
                LabeledContent("details.duration") {
                    Text(verbatim: Duration.seconds(duration).formatted(.time(pattern: .minuteSecond)))
                }
            }
            date("details.added", item.addedAt)
            date("details.created", item.fileCreatedAt)
            date("details.modified", item.fileModifiedAt)
        }
    }

    @ViewBuilder
    private func date(_ title: LocalizedStringKey, _ milliseconds: Int?) -> some View {
        if let milliseconds {
            LabeledContent(title) {
                Text(Date(timeIntervalSince1970: Double(milliseconds) / 1000), format: .dateTime.year().month().day().hour().minute())
            }
        }
    }

    /// Saves the text fields that changed.
    private func save() async {
        guard model.item != nil, !model.readOnly else { return }
        await model.rename(name)
        await model.setNotes(notes)
        await model.setLink(link)
    }
}
