import EglCore
import SwiftUI

/// What the folder editor works on: a new folder (under `parent`) or an existing one.
enum FolderEditing: Identifiable {
    case create(parent: String?)
    case edit(Folder)

    var id: String {
        switch self {
        case let .create(parent): "new.\(parent ?? "")"
        case let .edit(folder): folder.id
        }
    }
}

/// A folder's name, description and colour (D-097). Folder tags and its own sort stay as they
/// are: they belong to the desktop's properties panel.
struct FolderEditor: View {
    let open: OpenLibrary
    let editing: FolderEditing
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var notes = ""
    @State private var color: String? = nil
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("folder.name", text: $name)
                        .focused($focused)
                        .submitLabel(.done)
                        .onSubmit(save)
                        .accessibilityIdentifier("folder.name")
                    TextField("folder.description", text: $notes, axis: .vertical)
                        .lineLimit(1...4)
                }
                Section("folder.color") {
                    ColorChoice(color: $color)
                }
            }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.save", action: save)
                        .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                        .accessibilityIdentifier("folder.save")
                }
            }
            .onAppear {
                if case let .edit(folder) = editing {
                    name = folder.name
                    notes = folder.description
                    color = folder.iconColor
                }
                focused = true
            }
        }
        .presentationDetents([.medium, .large])
    }

    private var title: LocalizedStringKey {
        if case .edit = editing { return "folder.editTitle" }
        return "folder.newTitle"
    }

    private func save() {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { return }
        let change: FolderChange = switch editing {
        case let .create(parent):
            .create(name: name, parent: parent, description: notes, iconColor: color)
        case let .edit(folder):
            .update(
                id: folder.id, name: name, description: notes, tags: folder.tags, iconColor: color,
                orderBy: folder.orderBy, sortAscending: folder.sortAscending
            )
        }
        Task {
            if await open.saveFolder(change) {
                dismiss()
            }
        }
    }
}

/// No colour or one of the eight label colours.
struct ColorChoice: View {
    @Binding var color: String?

    var body: some View {
        HStack(spacing: 10) {
            swatch(nil)
            ForEach(LabelColor.allCases, id: \.self) { label in
                swatch(label.rawValue)
            }
        }
        .padding(.vertical, 4)
    }

    private func swatch(_ value: String?) -> some View {
        Button {
            color = value
        } label: {
            Circle()
                .fill(value.map(LabelColors.color) ?? EglTokens.grid)
                .frame(width: 28, height: 28)
                .overlay {
                    if value == nil {
                        Image(systemName: "slash.circle")
                            .foregroundStyle(EglTokens.mutedForeground)
                    }
                }
                .overlay {
                    if color == value {
                        Circle().strokeBorder(EglTokens.foreground, lineWidth: 2)
                            .padding(-3)
                    }
                }
        }
        .buttonStyle(.plain)
        .accessibilityLabel(value.map(LabelColors.name) ?? Text("color.none"))
        .accessibilityAddTraits(color == value ? .isSelected : [])
        .accessibilityIdentifier("color.\(value ?? "none")")
    }
}

/// The label colours as SwiftUI colours and names.
enum LabelColors {
    static func color(_ name: String) -> Color {
        switch LabelColor(rawValue: name) {
        case .red: .red
        case .orange: .orange
        case .yellow: .yellow
        case .green: .green
        case .aqua: .teal
        case .blue: .blue
        case .purple: .purple
        case .pink: .pink
        case nil: .gray
        }
    }

    static func name(_ name: String) -> Text {
        switch LabelColor(rawValue: name) {
        case .red: Text("color.red")
        case .orange: Text("color.orange")
        case .yellow: Text("color.yellow")
        case .green: Text("color.green")
        case .aqua: Text("color.aqua")
        case .blue: Text("color.blue")
        case .purple: Text("color.purple")
        case .pink: Text("color.pink")
        case nil: Text(verbatim: name)
        }
    }
}

/// Where a folder moves: the top level or another folder, never into itself.
struct FolderMovePicker: View {
    let open: OpenLibrary
    let folder: Folder
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let blocked = open.subtree(of: folder.id)
        let current = open.parent(of: folder.id)
        NavigationStack {
            List {
                Button {
                    move(to: nil)
                } label: {
                    HStack {
                        Label("folder.topLevel", systemImage: "square.stack")
                        Spacer()
                        if current == nil { Image(systemName: "checkmark").foregroundStyle(EglTokens.accent) }
                    }
                    .foregroundStyle(EglTokens.foreground)
                }
                .accessibilityIdentifier("move.top")
                ForEach(open.pickableFolders.filter { !blocked.contains($0.folder.id) }, id: \.folder.id) { entry in
                    Button {
                        move(to: entry.folder.id)
                    } label: {
                        HStack {
                            Label {
                                Text(verbatim: entry.folder.name)
                            } icon: {
                                Image(systemName: "folder")
                            }
                            .padding(.leading, CGFloat(entry.depth) * 18)
                            Spacer()
                            if current == entry.folder.id {
                                Image(systemName: "checkmark").foregroundStyle(EglTokens.accent)
                            }
                        }
                        .foregroundStyle(EglTokens.foreground)
                    }
                    .accessibilityIdentifier("move.\(entry.folder.name)")
                }
            }
            .navigationTitle("folder.moveTitle")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }

    private func move(to parent: String?) {
        guard parent != open.parent(of: folder.id) else {
            dismiss()
            return
        }
        let index = open.endIndex(under: parent)
        Task {
            if await open.saveFolder(.move(id: folder.id, parent: parent, index: index)) {
                dismiss()
            }
        }
    }
}
