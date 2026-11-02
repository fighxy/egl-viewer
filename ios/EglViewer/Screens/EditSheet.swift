import SwiftUI

struct EditSheet: View {
    @EnvironmentObject private var bridge: LibraryBridge
    let itemId: String

    @State private var segment: Segment = .details
    @State private var draftName: String = ""
    @State private var newTag: String = ""
    @State private var addingTag = false

    enum Segment: String, CaseIterable {
        case details = "Details"
        case tags = "Tags sheet"
        case folders = "Folders"
    }

    private var detail: ItemDTO? {
        bridge.itemDetail(id: itemId)
    }

    private var cell: ItemCellDTO? {
        bridge.items.first { $0.id == itemId }
    }

    private var displayName: String {
        if let detail {
            let ext = detail.ext
            return ext.isEmpty ? detail.name : "\(detail.name).\(ext)"
        }
        return cell?.name ?? "Item"
    }

    private var metaHint: String {
        var parts: [String] = []
        if let w = cell?.width, let h = cell?.height {
            parts.append("\(w)×\(h)")
        }
        if let ext = (detail?.ext ?? cell?.ext)?.uppercased(), !ext.isEmpty {
            parts.append(ext)
        }
        return parts.isEmpty ? "Details" : parts.joined(separator: " · ")
    }

    var body: some View {
        VStack(spacing: 12) {
            Capsule()
                .fill(Color(white: 0.83))
                .frame(width: 36, height: 5)
                .padding(.top, 2)

            VStack(alignment: .leading, spacing: 2) {
                Text(displayName)
                    .font(.system(size: 16, weight: .semibold))
                Text(metaHint)
                    .font(.system(size: 12))
                    .foregroundStyle(EglTokens.mutedForeground)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

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

            HStack(spacing: 2) {
                ForEach(Segment.allCases, id: \.self) { option in
                    Button {
                        segment = option
                    } label: {
                        Text(option.rawValue)
                            .font(.system(size: 12, weight: segment == option ? .semibold : .regular))
                            .foregroundStyle(
                                segment == option ? EglTokens.foreground : EglTokens.mutedForeground
                            )
                            .frame(maxWidth: .infinity)
                            .frame(height: 28)
                            .background(segment == option ? EglTokens.background : Color.clear)
                            .clipShape(RoundedRectangle(cornerRadius: 6))
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(2)
            .background(EglTokens.muted)
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 28)
        .background(EglTokens.background)
        .onAppear {
            draftName = detail?.name ?? ""
        }
    }

    private var detailsForm: some View {
        VStack(alignment: .leading, spacing: 12) {
            fieldLabel("Name")
            TextField("Name", text: $draftName)
                .font(.system(size: 14))
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(EglTokens.border, lineWidth: 1)
                )

            fieldLabel("Rating")
            HStack(spacing: 4) {
                ForEach(1 ... 5, id: \.self) { star in
                    Image(systemName: star <= Int(detail?.star ?? 0) ? "star.fill" : "star")
                        .foregroundStyle(
                            star <= Int(detail?.star ?? 0)
                                ? EglTokens.foreground
                                : Color(white: 0.83)
                        )
                        .font(.system(size: 16))
                }
            }

            fieldLabel("Tags")
            FlowLayout(spacing: 6) {
                ForEach(detail?.tags ?? [], id: \.self) { tag in
                    Text("\(tag)  ×")
                        .font(.system(size: 12))
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(EglTokens.muted)
                        .clipShape(RoundedRectangle(cornerRadius: 7))
                }
                Button {
                    addingTag = true
                } label: {
                    Text("+ Add tag")
                        .font(.system(size: 12))
                        .foregroundStyle(EglTokens.mutedForeground)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .overlay(
                            RoundedRectangle(cornerRadius: 7)
                                .stroke(style: StrokeStyle(lineWidth: 1, dash: [4]))
                                .foregroundStyle(EglTokens.border)
                        )
                }
                .buttonStyle(.plain)
            }

            if addingTag {
                HStack {
                    TextField("Add tag", text: $newTag)
                        .font(EglTokens.chromeFont)
                    Button("Add") {
                        let tag = newTag.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !tag.isEmpty else { return }
                        bridge.applyAddTag(itemId: itemId, tag: tag)
                        newTag = ""
                        addingTag = false
                    }
                    .font(EglTokens.chromeFont)
                }
            }

            fieldLabel("Folders")
            ForEach(Self.flattenFolders(bridge.folders), id: \.id) { row in
                folderRow(row, selected: Set(detail?.folders ?? []))
            }
        }
    }

    private var tagsForm: some View {
        VStack(alignment: .leading, spacing: 8) {
            fieldLabel("Match mode")
            Text("Any · All · None (stub)")
                .font(EglTokens.chromeFont)
                .foregroundStyle(EglTokens.mutedForeground)
            fieldLabel("Vocabulary")
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
                }
                .contentShape(Rectangle())
                .onTapGesture {
                    bridge.applyAddTag(itemId: itemId, tag: tag.name)
                }
            }
        }
    }

    private var foldersForm: some View {
        let selected = Set(detail?.folders ?? [])
        return VStack(alignment: .leading, spacing: 2) {
            ForEach(Self.flattenFolders(bridge.folders), id: \.id) { row in
                folderRow(row, selected: selected)
            }
        }
    }

    private func folderRow(_ row: FlatFolder, selected: Set<String>) -> some View {
        let on = selected.contains(row.id)
        return HStack(spacing: 8) {
            Image(systemName: "folder")
                .font(.system(size: 12))
            Text(row.name)
                .font(EglTokens.chromeFont)
                .fontWeight(on ? .semibold : .regular)
            Spacer()
            RoundedRectangle(cornerRadius: 4)
                .fill(on ? EglTokens.primary : Color.clear)
                .overlay(
                    RoundedRectangle(cornerRadius: 4)
                        .stroke(on ? EglTokens.primary : EglTokens.ring, lineWidth: 1.5)
                )
                .frame(width: 16, height: 16)
        }
        .padding(.leading, CGFloat(row.depth) * 8)
        .padding(.horizontal, 8)
        .frame(height: 34)
        .background(on ? EglTokens.muted : Color.clear)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private func fieldLabel(_ text: String) -> some View {
        Text(text)
            .font(EglTokens.captionFont)
            .foregroundStyle(EglTokens.mutedForeground)
    }

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
}

/// Minimal wrapping layout for tag chips (avoids Form / FlowLayout API gaps on older SDKs).
private struct FlowLayout<Content: View>: View {
    var spacing: CGFloat = 6
    @ViewBuilder var content: () -> Content

    var body: some View {
        // Simple vertical stack of chips is fine for the draft shell density.
        VStack(alignment: .leading, spacing: spacing) {
            content()
        }
    }
}
