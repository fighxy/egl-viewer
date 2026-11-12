import SwiftUI
import UniformTypeIdentifiers

/// The libraries first: the recent ones, then the rest of On My iPhone (D-130). Below them,
/// create a library in On My iPhone, open one from Files, or import an Eagle library (D-116).
struct WelcomeScreen: View {
    @Environment(AppModel.self) private var model
    @State private var creating = false
    @State private var picking: Picking?
    @State private var settings = false

    /// What the Files picker is open for: one picker serves them all, as SwiftUI allows one.
    private enum Picking: Equatable {
        case library, eagle
        /// A folder to move this library from the app's folder into (D-132).
        case move(URL)
    }

    var body: some View {
        let entries = WelcomeEntries(
            recents: model.recents, local: model.localLibraries, locations: model.locations
        )
        NavigationStack {
            List {
                if entries.isEmpty {
                    Section {
                        NoLibraries()
                    }
                    .listRowBackground(Color.clear)
                }

                if !entries.recent.isEmpty {
                    Section("welcome.recent") {
                        ForEach(entries.recent) { entry in
                            row(entry)
                        }
                    }
                }

                if !entries.local.isEmpty {
                    Section("welcome.onThisPhone") {
                        ForEach(entries.local) { entry in
                            row(entry)
                        }
                    }
                }

                Section {
                    Button {
                        creating = true
                    } label: {
                        Label("welcome.create", systemImage: "plus.square.on.square")
                    }
                    .accessibilityIdentifier("welcome.create")
                    Button {
                        picking = .library
                    } label: {
                        Label("welcome.open", systemImage: "folder")
                    }
                    .accessibilityIdentifier("welcome.open")
                    Button {
                        picking = .eagle
                    } label: {
                        Label("welcome.importEagle", systemImage: "square.and.arrow.down.on.square")
                    }
                    .accessibilityIdentifier("welcome.importEagle")
                } header: {
                    if !entries.isEmpty { Text("welcome.start") }
                } footer: {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("welcome.hint")
                        Text("welcome.importEagleHint")
                    }
                }
            }
            .navigationTitle("app.name")
            .refreshable { model.reloadLists() }
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        settings = true
                    } label: {
                        Label("settings.title", systemImage: "gearshape")
                    }
                    .accessibilityIdentifier("welcome.settings")
                }
            }
            .sheet(isPresented: $settings) {
                SettingsScreen(open: nil)
            }
            .overlay {
                if model.busy { ProgressView().controlSize(.large) }
            }
            .disabled(model.busy)
            .sheet(isPresented: $creating) {
                CreateLibrarySheet()
            }
            .sheet(item: Binding(get: { model.eagleImport }, set: { if $0 == nil { model.cancelEagle() } })) { plan in
                EagleImportSheet(plan: plan)
            }
            .fileImporter(
                isPresented: Binding(get: { picking != nil }, set: { if !$0 { picking = nil } }),
                allowedContentTypes: picking == .library ? LibraryLocations.openTypes : [.folder],
                allowsMultipleSelection: false
            ) { [picking] result in
                switch result {
                case let .success(urls):
                    guard let url = urls.first else { return }
                    switch picking {
                    case .eagle: Task { await model.pickEagle(url) }
                    case let .move(library): Task { await model.move(library, to: url) }
                    case .library, nil: Task { await model.open(picked: url) }
                    }
                case let .failure(error):
                    model.error = AppModel.describe(error)
                }
            }
            .confirmationDialog(
                "where.title",
                isPresented: Binding(get: { model.askingWhere != nil }, set: { if !$0 { model.askingWhere = nil } }),
                titleVisibility: .visible,
                presenting: model.askingWhere
            ) { picked in
                Button("where.onDevice") { Task { await model.open(picked: picked.url, onDevice: true) } }
                    .accessibilityIdentifier("where.onDevice")
                Button("where.synced") { Task { await model.open(picked: picked.url, onDevice: false) } }
                    .accessibilityIdentifier("where.synced")
                Button("common.cancel", role: .cancel) { model.askingWhere = nil }
            } message: { _ in
                Text("where.message")
            }
            .onAppear { model.reloadLists() }
        }
    }

    /// Libraries in the app's folder go with the app: these can move to a folder picked in Files.
    @ViewBuilder
    private func moveButton(_ library: URL) -> some View {
        Button("welcome.moveToFolder", systemImage: "folder.badge.plus") {
            picking = .move(library)
        }
    }

    @ViewBuilder
    private func row(_ entry: WelcomeEntry) -> some View {
        switch entry.source {
        case let .recent(recent):
            Button {
                Task { await model.open(recent: recent) }
            } label: {
                LibraryRow(entry: entry)
            }
            .accessibilityIdentifier("welcome.recent.\(entry.name)")
            .swipeActions {
                Button("welcome.forget", role: .destructive) {
                    model.forget(recent)
                }
            }
            .contextMenu {
                if entry.folder == nil {
                    moveButton(URL(fileURLWithPath: recent.path))
                }
                Button("welcome.forget", systemImage: "minus.circle", role: .destructive) {
                    model.forget(recent)
                }
            }
        case let .local(url):
            Button {
                Task { await model.openLocal(url) }
            } label: {
                LibraryRow(entry: entry)
            }
            .accessibilityIdentifier("welcome.local.\(entry.name)")
            .contextMenu {
                moveButton(url)
            }
        }
    }
}

/// Shown instead of the library lists until there is a library.
private struct NoLibraries: View {
    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "books.vertical")
                .font(.system(size: 44, weight: .light))
                .foregroundStyle(EglTokens.mutedForeground)
                .accessibilityHidden(true)
            Text("welcome.empty.title")
                .font(.headline)
                .foregroundStyle(EglTokens.foreground)
            Text("welcome.empty.hint")
                .font(.subheadline)
                .foregroundStyle(EglTokens.mutedForeground)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("welcome.empty")
    }
}

/// A library: its icon, its name, and where it lives with when it was last opened.
private struct LibraryRow: View {
    let entry: WelcomeEntry

    private var detail: String {
        let place = entry.folder ?? String(localized: "welcome.onThisPhone")
        guard let openedAt = entry.openedAt else { return place }
        return place + " · " + openedAt.formatted(.relative(presentation: .named))
    }

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: entry.folder == nil ? "books.vertical.fill" : "folder.fill")
                .font(.title3)
                .foregroundStyle(EglTokens.foreground)
                .frame(width: 40, height: 40)
                .background(EglTokens.grid, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(verbatim: entry.name)
                    .font(.body.weight(.medium))
                    .foregroundStyle(EglTokens.foreground)
                    .lineLimit(1)
                Text(verbatim: detail)
                    .font(.caption)
                    .foregroundStyle(EglTokens.mutedForeground)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer(minLength: 0)
            Image(systemName: "chevron.right")
                .font(.footnote.weight(.semibold))
                .foregroundStyle(.tertiary)
                .accessibilityHidden(true)
        }
        .contentShape(Rectangle())
    }
}

/// The name of a new library and where it goes: the app's folder, which iOS removes with the
/// app, or a folder picked in Files that outlives it (D-132).
struct CreateLibrarySheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var folder: URL?
    @State private var choosing = false
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("create.name", text: $name)
                        .focused($focused)
                        .textInputAutocapitalization(.words)
                        .submitLabel(.done)
                        .onSubmit(create)
                        .accessibilityIdentifier("create.name")
                } footer: {
                    Text("create.hint")
                }
                Section {
                    Button {
                        folder = nil
                    } label: {
                        PlaceRow(title: "create.inApp", systemImage: "iphone", detail: nil, chosen: folder == nil)
                    }
                    .accessibilityIdentifier("create.inApp")
                    Button {
                        choosing = true
                    } label: {
                        PlaceRow(
                            title: "create.inFolder", systemImage: "folder",
                            detail: folder?.lastPathComponent, chosen: folder != nil
                        )
                    }
                    .accessibilityIdentifier("create.inFolder")
                } header: {
                    Text("create.where")
                } footer: {
                    if folder == nil {
                        Text("create.inAppHint")
                    } else {
                        Text("create.inFolderHint")
                    }
                }
            }
            .fileImporter(isPresented: $choosing, allowedContentTypes: [.folder]) { result in
                switch result {
                case let .success(url):
                    if LibraryLocations.isICloud(url) {
                        model.error = String(localized: "error.cloudFolder")
                    } else {
                        folder = url
                    }
                case let .failure(error):
                    model.error = AppModel.describe(error)
                }
            }
            .navigationTitle("create.title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("create.confirm", action: create)
                        .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                        .accessibilityIdentifier("create.confirm")
                }
            }
            .onAppear { focused = true }
        }
        .presentationDetents([.medium, .large])
    }

    private func create() {
        let (name, folder) = (name, folder)
        dismiss()
        Task { await model.create(named: name, in: folder) }
    }
}

/// One place a new library can go, with a check mark on the chosen one.
private struct PlaceRow: View {
    let title: LocalizedStringKey
    let systemImage: String
    let detail: String?
    let chosen: Bool

    var body: some View {
        HStack {
            Label(title, systemImage: systemImage)
                .foregroundStyle(EglTokens.foreground)
            Spacer()
            if let detail {
                Text(verbatim: detail)
                    .foregroundStyle(EglTokens.mutedForeground)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            if chosen {
                Image(systemName: "checkmark")
                    .foregroundStyle(EglTokens.foreground)
                    .accessibilityHidden(true)
            }
        }
        .contentShape(Rectangle())
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

/// What an Eagle library holds, the name of the new library and whether Eagle's trash comes
/// too; Import creates the library on this iPhone and copies the Eagle library into it (D-116).
struct EagleImportSheet: View {
    let plan: EaglePlan
    @Environment(AppModel.self) private var model
    @State private var name = ""
    @State private var importTrash = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    LabeledContent("eagle.library") {
                        Text(verbatim: plan.summary.name)
                    }
                    LabeledContent("eagle.items") {
                        Text(verbatim: "\(plan.summary.itemCount)")
                    }
                    LabeledContent("eagle.size") {
                        Text(verbatim: ByteCountFormatter.string(
                            fromByteCount: Int64(plan.summary.bytes), countStyle: .file
                        ))
                    }
                }
                Section {
                    TextField("eagle.name", text: $name)
                        .textInputAutocapitalization(.words)
                        .accessibilityIdentifier("eagle.name")
                    Toggle("eagle.trash", isOn: $importTrash)
                        .accessibilityIdentifier("eagle.trash")
                } footer: {
                    Text("eagle.hint")
                }
            }
            .navigationTitle("eagle.title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("common.cancel") { model.cancelEagle() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("eagle.confirm") {
                        let (plan, name, importTrash) = (plan, name, importTrash)
                        Task { await model.importEagle(plan, named: name, importTrash: importTrash) }
                    }
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                    .accessibilityIdentifier("eagle.confirm")
                }
            }
            .onAppear { name = plan.summary.name }
        }
        .presentationDetents([.medium, .large])
    }
}
