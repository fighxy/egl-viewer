import EglCore
import SwiftUI

/// The library's tags in their groups (D-097): open one to see its items, rename it (onto an
/// existing name the two merge), merge it into another, delete it; make, edit and delete tag
/// groups. Tags stay exactly as typed: names are compared as they are.
struct TagsScreen: View {
    let open: OpenLibrary
    @State private var tags: [Tag] = []
    @State private var search = ""
    @State private var renaming: String? = nil
    @State private var newName = ""
    @State private var merging: String? = nil
    @State private var deleting: String? = nil
    @State private var editingGroup: TagGroupEditing? = nil
    @State private var deletingGroup: TagGroup? = nil

    var body: some View {
        List {
            ForEach(open.tagGroups) { group in
                let members = shown(group.tags)
                if !members.isEmpty || search.isEmpty {
                    Section {
                        ForEach(members, id: \.name) { tag in
                            row(tag, group: group)
                        }
                    } header: {
                        GroupHeader(group: group) {
                            editingGroup = .edit(group)
                        } delete: {
                            deletingGroup = group
                        }
                    }
                }
            }
            let grouped = Set(open.tagGroups.flatMap(\.tags))
            let loose = shown(tags.map(\.name).filter { !grouped.contains($0) })
            if !loose.isEmpty {
                Section {
                    ForEach(loose, id: \.name) { tag in
                        row(tag, group: nil)
                    }
                } header: {
                    open.tagGroups.isEmpty ? Text("tags.all") : Text("tags.ungrouped")
                }
            }
        }
        .overlay {
            if tags.isEmpty {
                ContentUnavailableView("tags.none", systemImage: "tag")
            }
        }
        .searchable(text: $search, prompt: Text("tags.find"))
        .navigationTitle("library.tags")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if !open.readOnly {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        editingGroup = .create
                    } label: {
                        Label("tags.newGroup", systemImage: "folder.badge.plus")
                    }
                    .accessibilityIdentifier("tags.newGroup")
                }
            }
        }
        .task { await reload() }
        // Edits elsewhere (an import, a tag added in the details) change the counts.
        .onChange(of: open.counts) { Task { await reload() } }
        .refreshable { await reload() }
        .alert("tags.renameTitle", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField("tags.newName", text: $newName)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .accessibilityIdentifier("tags.newName")
            Button("common.cancel", role: .cancel) {}
            Button("tags.rename") {
                guard let from = renaming else { return }
                let to = newName
                Task {
                    await open.renameTag(from, to: to)
                    await reload()
                }
            }
            .accessibilityIdentifier("tags.renameConfirm")
        } message: {
            Text("tags.renameHint")
        }
        .confirmationDialog(
            "tags.deleteTitle",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            titleVisibility: .visible,
            presenting: deleting
        ) { name in
            Button("tags.delete", role: .destructive) {
                Task {
                    await open.deleteTag(name)
                    await reload()
                }
            }
            .accessibilityIdentifier("tags.deleteConfirm")
        } message: { name in
            Text(verbatim: name)
        }
        .confirmationDialog(
            "tags.deleteGroupTitle",
            isPresented: Binding(get: { deletingGroup != nil }, set: { if !$0 { deletingGroup = nil } }),
            titleVisibility: .visible,
            presenting: deletingGroup
        ) { group in
            Button("tags.deleteGroup", role: .destructive) {
                Task { await open.deleteTagGroup(group.id) }
            }
        } message: { _ in
            Text("tags.deleteGroupHint")
        }
        .sheet(item: Binding(get: { merging.map(TagName.init) }, set: { merging = $0?.name })) { source in
            MergeTagPicker(open: open, source: source.name, tags: tags.map(\.name)) {
                await reload()
            }
        }
        .sheet(item: $editingGroup) { editing in
            TagGroupEditor(open: open, editing: editing, vocabulary: tags.map(\.name))
        }
    }

    private func shown(_ names: [String]) -> [Tag] {
        let counts = Dictionary(tags.map { ($0.name, $0.imageCount) }, uniquingKeysWith: { first, _ in first })
        let needle = search.trimmingCharacters(in: .whitespaces)
        return names
            .filter { needle.isEmpty || $0.localizedCaseInsensitiveContains(needle) }
            .map { Tag(name: $0, imageCount: counts[$0] ?? nil) }
    }

    @ViewBuilder
    private func row(_ tag: Tag, group: TagGroup?) -> some View {
        NavigationLink(value: Place.tag(name: tag.name)) {
            HStack {
                Text(verbatim: tag.name)
                Spacer()
                if let count = tag.imageCount {
                    Text(count, format: .number)
                        .foregroundStyle(EglTokens.mutedForeground)
                        .monospacedDigit()
                }
            }
        }
        .accessibilityIdentifier("tag.row.\(tag.name)")
        .swipeActions {
            if !open.readOnly {
                Button("tags.delete", role: .destructive) { deleting = tag.name }
                Button("tags.rename") {
                    newName = tag.name
                    renaming = tag.name
                }
            }
        }
        .contextMenu {
            if !open.readOnly {
                Button {
                    newName = tag.name
                    renaming = tag.name
                } label: {
                    Label("tags.rename", systemImage: "pencil")
                }
                Button {
                    merging = tag.name
                } label: {
                    Label("tags.merge", systemImage: "arrow.triangle.merge")
                }
                Menu {
                    ForEach(open.tagGroups.filter { !$0.tags.contains(tag.name) }) { target in
                        Button {
                            Task {
                                await open.saveTagGroup(.update(
                                    id: target.id, name: target.name, color: target.color, tags: target.tags + [tag.name]
                                ))
                            }
                        } label: {
                            Text(verbatim: target.name)
                        }
                    }
                } label: {
                    Label("tags.addToGroup", systemImage: "rectangle.stack.badge.plus")
                }
                .disabled(open.tagGroups.allSatisfy { $0.tags.contains(tag.name) })
                if let group {
                    Button {
                        Task {
                            await open.saveTagGroup(.update(
                                id: group.id, name: group.name, color: group.color, tags: group.tags.filter { $0 != tag.name }
                            ))
                        }
                    } label: {
                        Label("tags.removeFromGroup", systemImage: "rectangle.stack.badge.minus")
                    }
                }
                Button(role: .destructive) {
                    deleting = tag.name
                } label: {
                    Label("tags.delete", systemImage: "trash")
                }
            }
        }
    }

    private func reload() async {
        await open.refresh()
        if let list = await open.perform({ try $0.tags() }) {
            tags = list.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        }
    }
}

/// A tag name as a sheet item.
private struct TagName: Identifiable {
    let name: String
    var id: String { name }
}

private struct GroupHeader: View {
    let group: TagGroup
    let edit: () -> Void
    let delete: () -> Void

    var body: some View {
        HStack {
            if let color = group.color {
                Circle().fill(LabelColors.color(color)).frame(width: 9, height: 9)
            }
            Text(verbatim: group.name)
            Spacer()
            Menu {
                Button("tags.editGroup", action: edit)
                Button("tags.deleteGroup", role: .destructive, action: delete)
            } label: {
                Label("tags.groupActions", systemImage: "ellipsis.circle")
                    .labelStyle(.iconOnly)
            }
            .accessibilityIdentifier("group.menu.\(group.name)")
        }
    }
}

/// The tag `source` is merged into.
private struct MergeTagPicker: View {
    let open: OpenLibrary
    let source: String
    let tags: [String]
    let done: () async -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var search = ""

    var body: some View {
        NavigationStack {
            List {
                ForEach(tags.filter { $0 != source && (search.isEmpty || $0.localizedCaseInsensitiveContains(search)) }, id: \.self) { target in
                    Button {
                        Task {
                            if await open.mergeTag(source, into: target) {
                                await done()
                                dismiss()
                            }
                        }
                    } label: {
                        Text(verbatim: target)
                            .foregroundStyle(EglTokens.foreground)
                    }
                    .accessibilityIdentifier("merge.\(target)")
                }
            }
            .searchable(text: $search, placement: .navigationBarDrawer(displayMode: .always), prompt: Text("tags.find"))
            .navigationTitle("tags.mergeTitle")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
            }
        }
    }
}

enum TagGroupEditing: Identifiable {
    case create
    case edit(TagGroup)

    var id: String {
        if case let .edit(group) = self { return group.id }
        return "new"
    }
}

/// A tag group's name, colour and tags.
struct TagGroupEditor: View {
    let open: OpenLibrary
    let editing: TagGroupEditing
    let vocabulary: [String]
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var color: String? = nil
    @State private var chosen: [String] = []

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("tags.groupName", text: $name)
                        .accessibilityIdentifier("group.name")
                }
                Section("folder.color") {
                    ColorChoice(color: $color)
                }
                Section("tags.groupTags") {
                    ForEach(vocabulary, id: \.self) { tag in
                        Button {
                            if let index = chosen.firstIndex(of: tag) {
                                chosen.remove(at: index)
                            } else {
                                chosen.append(tag)
                            }
                        } label: {
                            HStack {
                                Text(verbatim: tag).foregroundStyle(EglTokens.foreground)
                                Spacer()
                                if chosen.contains(tag) {
                                    Image(systemName: "checkmark").foregroundStyle(EglTokens.accent)
                                }
                            }
                        }
                        .accessibilityAddTraits(chosen.contains(tag) ? .isSelected : [])
                        .accessibilityIdentifier("group.tag.\(tag)")
                    }
                }
            }
            .navigationTitle(isNew ? Text("tags.newGroup") : Text("tags.editGroup"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.save", action: save)
                        .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                        .accessibilityIdentifier("group.save")
                }
            }
            .onAppear {
                if case let .edit(group) = editing {
                    name = group.name
                    color = group.color
                    chosen = group.tags
                }
            }
        }
    }

    private var isNew: Bool {
        if case .create = editing { return true }
        return false
    }

    private func save() {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let change: TagGroupChange = switch editing {
        case .create: .create(name: name, color: color, tags: chosen)
        case let .edit(group): .update(id: group.id, name: name, color: color, tags: chosen)
        }
        Task {
            if await open.saveTagGroup(change) {
                dismiss()
            }
        }
    }
}
