import EglCore
import SwiftUI

/// The open library: its places with their counts, the folder tree and the smart folders.
/// A place opens its items (BrowseScreen); the last place opens again with the library
/// (D-094).
struct LibraryScreen: View {
    @Environment(AppModel.self) private var model
    let open: OpenLibrary
    /// Places and the tag list; a path of values, so links from the tag list push too.
    @State private var path = NavigationPath()
    @State private var restored = false
    @State private var adding: AddSource? = nil
    @State private var folderEditing: FolderEditing? = nil
    @State private var movingFolder: Folder? = nil
    @State private var deletingFolder: Folder? = nil
    @State private var smartEditing: SmartEditing? = nil
    @State private var deletingSmart: SmartFolder? = nil
    @State private var lockAction: LockAction? = nil

    var body: some View {
        NavigationStack(path: $path) {
            List {
                if open.readOnly {
                    Section {
                        Label(readOnlyText, systemImage: "lock")
                            .foregroundStyle(EglTokens.mutedForeground)
                            .accessibilityIdentifier("library.readOnly")
                    }
                }
                Section("library.places") {
                    PlaceLink(place: .all, title: "place.all", systemImage: "square.grid.2x2", count: open.counts?.all)
                    PlaceLink(place: .uncategorized, title: "place.uncategorized", systemImage: "tray", count: open.counts?.uncategorized)
                    PlaceLink(place: .untagged, title: "place.untagged", systemImage: "tag.slash", count: open.counts?.untagged)
                    PlaceLink(place: .trash, title: "place.trash", systemImage: "trash", count: open.counts?.trash)
                    NavigationLink(value: LibraryRoute.tags) {
                        Label("library.tags", systemImage: "tag")
                    }
                    .accessibilityIdentifier("place.tags")
                }
                if !open.folders.isEmpty || !open.readOnly {
                    Section("library.folders") {
                        OutlineGroup(open.folders, children: \.subfolders) { folder in
                            FolderRow(folder: folder) {
                                lockAction = .unlock(folder)
                            }
                            .contextMenu {
                                if folder.isLocked, !folder.isUnlocked {
                                    lockedMenu(folder)
                                } else if !open.readOnly {
                                    folderMenu(folder)
                                    lockMenu(folder)
                                }
                            }
                        }
                        if !open.readOnly {
                            Button {
                                folderEditing = .create(parent: nil)
                            } label: {
                                Label("folder.new", systemImage: "folder.badge.plus")
                            }
                            .accessibilityIdentifier("folder.new")
                        }
                    }
                }
                if !open.smartFolders.isEmpty || !open.readOnly {
                    Section("library.smartFolders") {
                        OutlineGroup(open.smartFolders, children: \.subfolders) { folder in
                            NavigationLink(value: Place.smart(id: folder.id, name: folder.name)) {
                                Label {
                                    Text(verbatim: folder.name)
                                } icon: {
                                    Image(systemName: "gearshape")
                                }
                            }
                            .accessibilityIdentifier("smart.\(folder.name)")
                            .contextMenu {
                                if !open.readOnly {
                                    smartMenu(folder)
                                }
                            }
                        }
                        if !open.readOnly {
                            Button {
                                smartEditing = .create(parent: nil, conditions: nil)
                            } label: {
                                Label("smart.new", systemImage: "gearshape.2")
                            }
                            .accessibilityIdentifier("smart.new")
                        }
                    }
                }
                if !open.imports.isEmpty {
                    Section("library.imports") {
                        ForEach(Array(open.imports.values)) { progress in
                            ProgressView(
                                value: Double(progress.processed),
                                total: Double(max(progress.total, 1))
                            ) {
                                Text("library.adding \(progress.processed) \(progress.total)")
                            }
                        }
                    }
                }
            }
            .navigationTitle(open.info.name)
            .accessibilityIdentifier("library.screen")
            .navigationDestination(for: Place.self) { place in
                BrowseScreen(open: open, place: place)
            }
            .navigationDestination(for: LibraryRoute.self) { _ in
                TagsScreen(open: open)
            }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("library.close") {
                        Task { await model.close() }
                    }
                    .accessibilityIdentifier("library.close")
                }
                if open.hasUnlockedFolders {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button {
                            Task { await open.relockAll() }
                        } label: {
                            Label("lock.lockAll", systemImage: "lock")
                        }
                        .accessibilityIdentifier("lock.lockAll")
                    }
                }
                if !open.readOnly {
                    ToolbarItem(placement: .topBarTrailing) {
                        AddMenu(open: open, folder: nil, source: $adding)
                    }
                }
            }
            .refreshable { await open.refresh() }
            .addPickers(open, folder: nil, source: $adding)
        }
        .duplicateQuestion(open)
        .sheet(item: $folderEditing) { editing in
            FolderEditor(open: open, editing: editing)
        }
        .sheet(item: $movingFolder) { folder in
            FolderMovePicker(open: open, folder: folder)
        }
        .sheet(item: $lockAction) { action in
            LockSheet(open: open, action: action)
        }
        .sheet(item: $smartEditing) { editing in
            SmartFolderEditor(open: open, editing: editing)
        }
        .confirmationDialog(
            "folder.deleteTitle",
            isPresented: Binding(get: { deletingFolder != nil }, set: { if !$0 { deletingFolder = nil } }),
            titleVisibility: .visible,
            presenting: deletingFolder
        ) { folder in
            Button("folder.deleteKeepItems", role: .destructive) {
                Task { await open.deleteFolder(folder.id, trashItems: false) }
            }
            .accessibilityIdentifier("folder.deleteKeepItems")
            Button("folder.deleteTrashItems", role: .destructive) {
                Task { await open.deleteFolder(folder.id, trashItems: true) }
            }
            .accessibilityIdentifier("folder.deleteTrashItems")
        } message: { folder in
            Text(verbatim: folder.name)
        }
        .confirmationDialog(
            "smart.deleteTitle",
            isPresented: Binding(get: { deletingSmart != nil }, set: { if !$0 { deletingSmart = nil } }),
            titleVisibility: .visible,
            presenting: deletingSmart
        ) { folder in
            Button("smart.delete", role: .destructive) {
                Task { await open.deleteSmartFolder(folder.id) }
            }
            .accessibilityIdentifier("smart.deleteConfirm")
        } message: { folder in
            folder.children.isEmpty ? Text("smart.deleteHint") : Text("smart.deleteHintChildren")
        }
        .onAppear(perform: restore)
        .onChange(of: path) { _, path in
            if path.isEmpty, restored {
                open.forgetPlace()
            }
        }
        .alert(
            "error.title",
            isPresented: Binding(get: { open.error != nil }, set: { if !$0 { open.error = nil } }),
            presenting: open.error
        ) { _ in
            Button("common.ok", role: .cancel) {}
        } message: { message in
            Text(message)
        }
    }

    @ViewBuilder
    private func folderMenu(_ folder: Folder) -> some View {
        Button {
            folderEditing = .create(parent: folder.id)
        } label: {
            Label("folder.newSubfolder", systemImage: "folder.badge.plus")
        }
        Button {
            folderEditing = .edit(folder)
        } label: {
            Label("folder.edit", systemImage: "pencil")
        }
        Button {
            movingFolder = folder
        } label: {
            Label("folder.move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
        }
        Button(role: .destructive) {
            deletingFolder = folder
        } label: {
            Label("folder.delete", systemImage: "trash")
        }
    }

    /// A closed lock: open it, with the password or the recovery key.
    @ViewBuilder
    private func lockedMenu(_ folder: Folder) -> some View {
        Button {
            lockAction = .unlock(folder)
        } label: {
            Label("lock.unlock", systemImage: "lock.open")
        }
        Button {
            lockAction = .recover(folder)
        } label: {
            Label("lock.recover", systemImage: "key")
        }
        if !open.readOnly, !folder.hasPassword {
            // A lock imported without a password (an Eagle folder): give it one or take it off.
            Button {
                lockAction = .lock(folder)
            } label: {
                Label("lock.setPassword", systemImage: "lock.rotation")
            }
            Button(role: .destructive) {
                lockAction = .removeLock(folder)
            } label: {
                Label("lock.removeLock", systemImage: "lock.slash")
            }
        }
    }

    /// An open folder: lock it; an open lock: close it again or manage it.
    @ViewBuilder
    private func lockMenu(_ folder: Folder) -> some View {
        if !folder.isLocked {
            Button {
                lockAction = .lock(folder)
            } label: {
                Label("lock.lock", systemImage: "lock")
            }
        } else {
            Button {
                Task { _ = await open.lockWork { try $0.relock(folder.id) } }
            } label: {
                Label("lock.lockNow", systemImage: "lock")
            }
            Button {
                lockAction = .changePassword(folder)
            } label: {
                Label("lock.changePassword", systemImage: "key")
            }
            Button {
                lockAction = .renewKey(folder)
            } label: {
                Label("lock.renewKey", systemImage: "key.viewfinder")
            }
            Button {
                lockAction = .label(folder)
            } label: {
                Label("lock.labelTitle", systemImage: "tag")
            }
            Button(role: .destructive) {
                lockAction = .removeLock(folder)
            } label: {
                Label("lock.removeLock", systemImage: "lock.slash")
            }
        }
    }

    @ViewBuilder
    private func smartMenu(_ folder: SmartFolder) -> some View {
        Button {
            smartEditing = .create(parent: folder.id, conditions: nil)
        } label: {
            Label("smart.newChild", systemImage: "gearshape.2")
        }
        Button {
            smartEditing = .edit(folder)
        } label: {
            Label("smart.edit", systemImage: "pencil")
        }
        Button(role: .destructive) {
            deletingSmart = folder
        } label: {
            Label("smart.delete", systemImage: "trash")
        }
    }

    /// Opens the place the library was left in, once, if it still exists.
    private func restore() {
        guard !restored else { return }
        restored = true
        if let place = open.savedView.place, open.exists(place) {
            path.append(place)
        }
    }

    private var readOnlyText: LocalizedStringKey {
        switch open.info.readOnlyReason {
        case .syncFolder: "library.readOnly.synced"
        case .otherInstance: "library.readOnly.otherInstance"
        default: "library.readOnly"
        }
    }
}

private struct PlaceLink: View {
    let place: Place
    let title: LocalizedStringKey
    let systemImage: String
    let count: Int?

    var body: some View {
        NavigationLink(value: place) {
            HStack {
                Label(title, systemImage: systemImage)
                Spacer()
                if let count {
                    Text(count, format: .number)
                        .foregroundStyle(EglTokens.mutedForeground)
                        .monospacedDigit()
                }
            }
        }
        .accessibilityIdentifier("place.\(place.key)")
    }
}

/// A folder of the tree. A closed lock shows its label and opens the unlock sheet.
private struct FolderRow: View {
    let folder: Folder
    let unlock: () -> Void

    var body: some View {
        if folder.isLocked, !folder.isUnlocked {
            Button(action: unlock) {
                HStack {
                    Label {
                        Text(verbatim: folder.lockLabel ?? folder.name)
                    } icon: {
                        Image(systemName: "lock.fill")
                            .foregroundStyle(folder.lockColor.map(LabelColors.color) ?? EglTokens.mutedForeground)
                    }
                    Spacer()
                    Text("folder.locked")
                        .font(.caption)
                }
                .foregroundStyle(EglTokens.mutedForeground)
            }
            .accessibilityIdentifier("folder.\(folder.name)")
        } else {
            NavigationLink(value: Place.folder(id: folder.id, name: folder.name)) {
                HStack {
                    Label {
                        Text(verbatim: folder.name)
                    } icon: {
                        Image(systemName: folder.isLocked ? "lock.open" : "folder")
                            .foregroundStyle(folder.iconColor.map(LabelColors.color) ?? EglTokens.accent)
                    }
                    Spacer()
                    if let count = folder.descendantImageCount ?? folder.imageCount {
                        Text(count, format: .number)
                            .foregroundStyle(EglTokens.mutedForeground)
                            .monospacedDigit()
                    }
                }
            }
            .accessibilityIdentifier("folder.\(folder.name)")
        }
    }
}

extension Folder {
    /// Children for OutlineGroup: nil for a leaf, so it shows no disclosure arrow.
    var subfolders: [Folder]? { children.isEmpty ? nil : children }
}

extension SmartFolder {
    var subfolders: [SmartFolder]? { children.isEmpty ? nil : children }
}

/// Screens of the library that are not places.
enum LibraryRoute: Hashable {
    case tags
}

