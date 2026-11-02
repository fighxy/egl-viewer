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
        // Plain VStack (not NavigationStack): avoids opaque-type recursion with
        // nested folder rows under newer Swift / Xcode CI toolchains.
        VStack(spacing: 0) {
            Text("Edit")
                .font(EglTokens.titleFont)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 16)
                .padding(.top, 12)

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
        .onAppear {
            draftName = detail?.name ?? ""
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
        let selected = Set(detail?.folders ?? [])
        return List {
            ForEach(Self.flattenFolders(bridge.folders), id: \.id) { row in
                HStack {
                    Text(String(repeating: "  ", count: row.depth) + row.name)
                        .font(EglTokens.chromeFont)
                    Spacer()
                    Image(systemName: selected.contains(row.id) ? "checkmark.square.fill" : "square")
                        .foregroundStyle(EglTokens.foreground)
                }
            }
        }
        .listStyle(.plain)
    }

    /// Flattened rows avoid recursive `some View` (opaque type self-reference on CI).
    private struct FlatFolder {
        let id: String
        let name: String
        let depth: Int
    }

    private static func flattenFolders(_ nodes: [FolderDTO], depth: Int = 0) -> [FlatFolder] {
        nodes.flatMap { node in
            [FlatFolder(id: node.id, name: node.name, depth: depth)]
                + flattenFolders(node.children, depth: depth + 1)
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
