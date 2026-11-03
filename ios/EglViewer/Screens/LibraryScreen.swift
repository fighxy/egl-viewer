import EglCore
import SwiftUI

/// The open library: its places with their counts. Browsing the items arrives with the grid
/// (the next PR of the series, D-093).
struct LibraryScreen: View {
    @Environment(AppModel.self) private var model
    let open: OpenLibrary

    var body: some View {
        NavigationStack {
            List {
                if open.readOnly {
                    Section {
                        Label(readOnlyText, systemImage: "lock")
                            .foregroundStyle(EglTokens.mutedForeground)
                            .accessibilityIdentifier("library.readOnly")
                    }
                }
                Section("library.places") {
                    PlaceRow(title: "place.all", systemImage: "square.grid.2x2", count: open.counts?.all)
                    PlaceRow(title: "place.uncategorized", systemImage: "tray", count: open.counts?.uncategorized)
                    PlaceRow(title: "place.untagged", systemImage: "tag.slash", count: open.counts?.untagged)
                    PlaceRow(title: "place.trash", systemImage: "trash", count: open.counts?.trash)
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
    }

    private var readOnlyText: LocalizedStringKey {
        switch open.info.readOnlyReason {
        case .syncFolder: "library.readOnly.synced"
        case .otherInstance: "library.readOnly.otherInstance"
        default: "library.readOnly"
        }
    }
}

private struct PlaceRow: View {
    let title: LocalizedStringKey
    let systemImage: String
    let count: Int?

    var body: some View {
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
}
