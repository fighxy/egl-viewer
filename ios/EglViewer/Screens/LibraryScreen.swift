import EglCore
import SwiftUI

/// The open library: its places with their counts, the folder tree and the smart folders.
/// A place opens its items (BrowseScreen); the last place opens again with the library
/// (D-094).
struct LibraryScreen: View {
    @Environment(AppModel.self) private var model
    let open: OpenLibrary
    @State private var path: [Place] = []
    @State private var restored = false

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
                }
                if !open.folders.isEmpty {
                    Section("library.folders") {
                        OutlineGroup(open.folders, children: \.subfolders) { folder in
                            FolderRow(folder: folder)
                        }
                    }
                }
                if !open.smartFolders.isEmpty {
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
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("library.close") {
                        Task { await model.close() }
                    }
                    .accessibilityIdentifier("library.close")
                }
            }
            .refreshable { await open.refresh() }
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

    /// Opens the place the library was left in, once, if it still exists.
    private func restore() {
        guard !restored else { return }
        restored = true
        if let place = open.savedView.place, open.exists(place) {
            path = [place]
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

/// A folder of the tree. A locked folder shows its lock and does not open; unlocking arrives
/// with the locks PR.
private struct FolderRow: View {
    let folder: Folder

    var body: some View {
        if folder.isLocked, !folder.isUnlocked {
            HStack {
                Label {
                    Text(verbatim: folder.lockLabel ?? folder.name)
                } icon: {
                    Image(systemName: "lock.fill")
                }
                Spacer()
                Text("folder.locked")
                    .font(.caption)
                    .foregroundStyle(EglTokens.mutedForeground)
            }
            .foregroundStyle(EglTokens.mutedForeground)
            .accessibilityIdentifier("folder.\(folder.name)")
        } else {
            NavigationLink(value: Place.folder(id: folder.id, name: folder.name)) {
                HStack {
                    Label {
                        Text(verbatim: folder.name)
                    } icon: {
                        Image(systemName: folder.isLocked ? "lock.open" : "folder")
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
