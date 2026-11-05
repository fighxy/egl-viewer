import EglCore
import SwiftUI

/// The bar under a selection: its count and what can be done with it (D-095). In the Trash:
/// Restore and Delete Permanently; elsewhere: rate, tag, add to a folder, share, trash.
struct SelectionBar: View {
    let model: BrowseModel
    @State private var tagging = false
    @State private var filing = false
    @State private var deleting = false

    var body: some View {
        let none = model.selectedCount == 0
        let readOnly = model.open.readOnly
        VStack(spacing: 8) {
            Text("browse.selected \(model.selectedCount)")
                .font(.footnote.weight(.medium))
                .accessibilityIdentifier("browse.selectedCount")
            HStack {
                if model.place == .trash {
                    Button {
                        Task { await model.restore(model.target) }
                    } label: {
                        Label("action.restore", systemImage: "arrow.uturn.backward")
                    }
                    .accessibilityIdentifier("selection.restore")
                    Spacer()
                    Button(role: .destructive) {
                        deleting = true
                    } label: {
                        Label("action.deleteForGood", systemImage: "trash.slash")
                    }
                    .accessibilityIdentifier("selection.delete")
                } else {
                    RatingMenu { stars in
                        Task { await model.rate(model.target, stars: stars) }
                    }
                    .disabled(readOnly)
                    .accessibilityIdentifier("selection.rate")
                    Spacer()
                    Button {
                        tagging = true
                    } label: {
                        Label("action.tags", systemImage: "tag")
                    }
                    .disabled(readOnly)
                    .accessibilityIdentifier("selection.tags")
                    Spacer()
                    Button {
                        filing = true
                    } label: {
                        Label("action.addToFolder", systemImage: "folder.badge.plus")
                    }
                    .disabled(readOnly)
                    .accessibilityIdentifier("selection.folder")
                    Spacer()
                    Button {
                        Task { await model.share(model.target) }
                    } label: {
                        Label("action.share", systemImage: "square.and.arrow.up")
                    }
                    .accessibilityIdentifier("selection.share")
                    Spacer()
                    Button(role: .destructive) {
                        Task { await model.trash(model.target) }
                    } label: {
                        Label("action.trash", systemImage: "trash")
                    }
                    .disabled(readOnly)
                    .accessibilityIdentifier("selection.trash")
                }
            }
            .labelStyle(.iconOnly)
            .font(.title3)
            .disabled(none)
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity)
        .background(.bar)
        .sheet(isPresented: $tagging) {
            SelectionTagsSheet(model: model, target: model.target)
        }
        .sheet(isPresented: $filing) {
            FolderPicker(open: model.open, title: "action.addToFolder", chosen: []) { folder in
                filing = false
                Task { await model.addToFolder(model.target, folder) }
            }
        }
        .confirmationDialog("trash.deleteConfirm \(model.selectedCount)", isPresented: $deleting, titleVisibility: .visible) {
            Button("action.deleteForGood", role: .destructive) {
                Task { await model.deleteForGood(model.target) }
            }
            .accessibilityIdentifier("selection.deleteConfirmed")
        } message: {
            Text("trash.noUndo")
        }
    }
}

/// No stars to five, as a menu.
struct RatingMenu: View {
    let rate: (Int) -> Void

    var body: some View {
        Menu {
            ForEach([5, 4, 3, 2, 1, 0], id: \.self) { stars in
                Button {
                    rate(stars)
                } label: {
                    if stars == 0 {
                        Text("rating.none")
                    } else {
                        StarsText(stars: stars)
                    }
                }
                .accessibilityIdentifier("rating.\(stars)")
            }
        } label: {
            Label("action.rate", systemImage: "star")
        }
    }
}

/// A rating as stars, read by VoiceOver as "3 stars" rather than the symbols.
struct StarsText: View {
    let stars: Int

    var body: some View {
        Text(verbatim: String(repeating: "★", count: stars))
            .accessibilityLabel(Text("details.stars \(stars)"))
    }
}

/// The long-press menu of one item.
struct ItemMenu: View {
    let model: BrowseModel
    let cell: ItemCell

    var body: some View {
        let target = ItemsTarget(ids: [cell.id])
        Button {
            model.details = cell
        } label: {
            Label("action.info", systemImage: "info.circle")
        }
        Button {
            Task { await model.share(target) }
        } label: {
            Label("action.share", systemImage: "square.and.arrow.up")
        }
        Button {
            model.startSelecting(with: cell)
        } label: {
            Label("browse.select", systemImage: "checkmark.circle")
        }
        if !model.open.readOnly {
            if model.place == .trash {
                Button {
                    Task { await model.restore(target) }
                } label: {
                    Label("action.restore", systemImage: "arrow.uturn.backward")
                }
            } else {
                RatingMenu { stars in
                    Task { await model.rate(target, stars: stars) }
                }
                Button(role: .destructive) {
                    Task { await model.trash(target) }
                } label: {
                    Label("action.trash", systemImage: "trash")
                }
            }
        }
    }
}

/// The tags of a selection with their counts: remove one from every selected item, or add one.
struct SelectionTagsSheet: View {
    let model: BrowseModel
    let target: ItemsTarget
    @Environment(\.dismiss) private var dismiss
    @State private var tags: SelectionTags? = nil
    @State private var text = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TagEntry(text: $text, vocabulary: model.vocabulary, taken: Set(tags?.tags.map(\.name) ?? [])) { name in
                        Task {
                            await model.addTags(target, [name])
                            await reload()
                        }
                    }
                }
                if let tags, !tags.tags.isEmpty {
                    Section("selection.tagsPresent") {
                        ForEach(tags.tags, id: \.name) { tag in
                            HStack {
                                Text(verbatim: tag.name)
                                Spacer()
                                Text("selection.tagCount \(tag.imageCount ?? 0) \(tags.items)")
                                    .font(.caption)
                                    .foregroundStyle(EglTokens.mutedForeground)
                                Button(role: .destructive) {
                                    Task {
                                        await model.removeTags(target, [tag.name])
                                        await reload()
                                    }
                                } label: {
                                    Label("action.removeTag", systemImage: "minus.circle")
                                        .labelStyle(.iconOnly)
                                }
                                .buttonStyle(.borderless)
                                .accessibilityIdentifier("selectionTags.remove.\(tag.name)")
                            }
                        }
                    }
                }
            }
            .navigationTitle("action.tags")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.done") { dismiss() }
                        .accessibilityIdentifier("selectionTags.done")
                }
            }
            .task {
                await model.loadVocabulary()
                await reload()
            }
        }
        .presentationDetents([.medium, .large])
    }

    private func reload() async {
        let target = target
        tags = await model.open.perform { try $0.selectionTags(target) }
    }
}

/// A text field that adds a tag, with suggestions from the library's tags. Tags are compared
/// exactly; only the suggestions ignore case.
struct TagEntry: View {
    @Binding var text: String
    let vocabulary: [Tag]
    let taken: Set<String>
    let add: (String) -> Void

    var body: some View {
        TextField("tags.add", text: $text)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .submitLabel(.done)
            .onSubmit { commit(text) }
            .accessibilityIdentifier("tags.add")
        ForEach(TagSuggestions.matching(text, in: vocabulary, excluding: taken), id: \.self) { name in
            Button {
                commit(name)
            } label: {
                Label {
                    Text(verbatim: name)
                } icon: {
                    Image(systemName: "tag")
                }
            }
            .accessibilityIdentifier("tags.suggestion.\(name)")
        }
    }

    private func commit(_ value: String) {
        let name = value.trimmingCharacters(in: .whitespacesAndNewlines)
        text = ""
        guard !name.isEmpty, !taken.contains(name) else { return }
        add(name)
    }
}

enum TagSuggestions {
    /// Library tags that contain `text` (ignoring case), those starting with it first; at most
    /// `limit`, without the ones already present.
    static func matching(_ text: String, in vocabulary: [Tag], excluding taken: Set<String>, limit: Int = 6) -> [String] {
        let needle = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !needle.isEmpty else { return [] }
        let names = vocabulary.map(\.name).filter { !taken.contains($0) && $0.localizedCaseInsensitiveContains(needle) }
        let starting = names.filter { $0.lowercased().hasPrefix(needle.lowercased()) }
        let rest = names.filter { !$0.lowercased().hasPrefix(needle.lowercased()) }
        return Array((starting + rest).prefix(limit))
    }
}

/// The folder tree to pick from; `chosen` folders show a checkmark.
struct FolderPicker: View {
    let open: OpenLibrary
    let title: LocalizedStringKey
    let chosen: Set<String>
    let pick: (String) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                ForEach(open.pickableFolders, id: \.folder.id) { entry in
                    Button {
                        pick(entry.folder.id)
                    } label: {
                        HStack {
                            Label {
                                Text(verbatim: entry.folder.name)
                            } icon: {
                                Image(systemName: "folder")
                            }
                            .padding(.leading, CGFloat(entry.depth) * 18)
                            Spacer()
                            if chosen.contains(entry.folder.id) {
                                Image(systemName: "checkmark")
                                    .foregroundStyle(EglTokens.accent)
                            }
                        }
                        .foregroundStyle(EglTokens.foreground)
                    }
                    .accessibilityAddTraits(chosen.contains(entry.folder.id) ? .isSelected : [])
                    .accessibilityIdentifier("folderPicker.\(entry.folder.name)")
                }
            }
            .overlay {
                if open.pickableFolders.isEmpty {
                    ContentUnavailableView("folders.none", systemImage: "folder")
                }
            }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.done") { dismiss() }
                        .accessibilityIdentifier("folderPicker.done")
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}

/// The note after an action, with Undo when the core can take it back.
private struct ToastModifier: ViewModifier {
    let open: OpenLibrary
    let alignment: Alignment
    let padding: CGFloat
    let shown: Bool

    func body(content: Content) -> some View {
        content.overlay(alignment: alignment) {
            if shown, let toast = open.toast {
                HStack(spacing: 16) {
                    Text(verbatim: toast.message)
                        .font(.subheadline)
                        .accessibilityIdentifier("toast.message")
                    if toast.undoable, !open.readOnly {
                        Button("toast.undo") {
                            Task { await open.undo() }
                        }
                        .fontWeight(.semibold)
                        .accessibilityIdentifier("toast.undo")
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(.regularMaterial, in: Capsule())
                .shadow(radius: 4, y: 2)
                .padding(alignment == .top ? .top : .bottom, padding)
                .transition(.move(edge: alignment == .top ? .top : .bottom).combined(with: .opacity))
                .id(toast.id)
            }
        }
        .animation(.snappy, value: open.toast)
    }
}

extension View {
    /// The note after an action; `shown` is false while a screen is covered by another one
    /// that shows the note itself.
    func toast(_ open: OpenLibrary, alignment: Alignment = .bottom, padding: CGFloat = 16, shown: Bool = true) -> some View {
        modifier(ToastModifier(open: open, alignment: alignment, padding: padding, shown: shown))
    }
}
