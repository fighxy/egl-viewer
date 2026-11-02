import SwiftUI

struct EditSheet: View {
    @EnvironmentObject private var bridge: LibraryBridge
    let itemId: String

    @State private var segment: Segment = .details
    @State private var draftName: String = ""
    @State private var newTag: String = ""

    enum Segment: String, CaseIterable {
        case details = "Details"
        case tags = "Tags sheet"
        case folders = "Folders"
    }

    private var detail: ItemDTO? {
        bridge.itemDetail(id: itemId)
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker("Section", selection: $segment) {
                    ForEach(Segment.allCases, id: \.self) { segment in
                        Text(segment.rawValue).tag(segment)
                    }
                }
                .pickerStyle(.segmented)
                .padding(12)

                Group {
                    switch segment {
                    case .details:
                        detailsForm
                    case .tags:
                        tagsForm
                    case .folders:
                        foldersForm
                    }
                }
                Spacer(minLength: 0)
            }
            .background(EglTokens.background)
            .navigationTitle("Edit")
            .navigationBarTitleDisplayMode(.inline)
            .onAppear {
                draftName = detail?.name ?? ""
            }
        }
    }

    private var detailsForm: some View {
        Form {
            Section {
                TextField("Name", text: $draftName)
                    .font(EglTokens.titleFont)
                HStack(spacing: 6) {
                    ForEach(1 ... 5, id: \.self) { star in
                        Image(systemName: star <= Int(detail?.star ?? 0) ? "star.fill" : "star")
                            .foregroundStyle(EglTokens.foreground)
                            .font(.system(size: 16))
                    }
                }
            }
            Section("Tags") {
                FlowTags(tags: detail?.tags ?? [])
                HStack {
                    TextField("Add tag", text: $newTag)
                        .font(EglTokens.chromeFont)
                    Button("Add") {
                        let tag = newTag.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !tag.isEmpty else { return }
                        bridge.applyAddTag(itemId: itemId, tag: tag)
                        newTag = ""
                    }
                    .font(EglTokens.chromeFont)
                }
            }
            Section("Folders") {
                ForEach(detail?.folders ?? [], id: \.self) { folderId in
                    Text(folderName(folderId))
                        .font(EglTokens.chromeFont)
                }
            }
        }
    }

    private var tagsForm: some View {
        List {
            Section("Match mode") {
                Text("Any · All · None (stub)")
                    .font(EglTokens.chromeFont)
                    .foregroundStyle(EglTokens.mutedForeground)
            }
            Section("Vocabulary") {
                ForEach(bridge.tags, id: \.name) { tag in
                    HStack {
                        Text(tag.name)
                            .font(EglTokens.chromeFont)
                        Spacer()
                        if let count = tag.imageCount {
                            Text("\(count)")
                                .font(EglTokens.chromeFont)
                                .foregroundStyle(EglTokens.mutedForeground)
                        }
                        Button("Add") {
                            bridge.applyAddTag(itemId: itemId, tag: tag.name)
                        }
                        .font(EglTokens.chromeFont)
                    }
                }
            }
        }
        .listStyle(.plain)
    }

    private var foldersForm: some View {
        List {
            ForEach(bridge.folders) { folder in
                folderRow(folder, depth: 0)
            }
        }
        .listStyle(.plain)
    }

    @ViewBuilder
    private func folderRow(_ folder: FolderDTO, depth: Int) -> some View {
        let checked = detail?.folders.contains(folder.id) == true
        HStack {
            Text(String(repeating: "  ", count: depth) + folder.name)
                .font(EglTokens.chromeFont)
            Spacer()
            Image(systemName: checked ? "checkmark.square.fill" : "square")
                .foregroundStyle(EglTokens.foreground)
        }
        ForEach(folder.children) { child in
            folderRow(child, depth: depth + 1)
        }
    }

    private func folderName(_ id: String) -> String {
        func search(_ nodes: [FolderDTO]) -> String? {
            for node in nodes {
                if node.id == id { return node.name }
                if let nested = search(node.children) { return nested }
            }
            return nil
        }
        return search(bridge.folders) ?? id
    }
}

private struct FlowTags: View {
    let tags: [String]

    var body: some View {
        FlexibleTagWrap(tags: tags)
    }
}

private struct FlexibleTagWrap: View {
    let tags: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(tags, id: \.self) { tag in
                Text(tag)
                    .font(EglTokens.chromeFont)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(EglTokens.muted)
                    .overlay(
                        RoundedRectangle(cornerRadius: 4)
                            .stroke(EglTokens.border, lineWidth: 1)
                    )
            }
        }
    }
}
