import SwiftUI
import UniformTypeIdentifiers

/// The libraries first: the recent ones, then the rest of On My iPhone (D-130). Below them,
/// create a library in On My iPhone, open one from Files, or import an Eagle library (D-116).
struct WelcomeScreen: View {
    @Environment(AppModel.self) private var model
    @State private var creating = false
    @State private var picking: Picking?
    @State private var settings = false

    /// What the Files picker is open for: one picker serves both, as SwiftUI allows one.
    private enum Picking { case library, eagle }

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
                allowedContentTypes: picking == .eagle ? [.folder] : LibraryLocations.openTypes,
                allowsMultipleSelection: false
            ) { [picking] result in
                switch result {
                case let .success(urls):
                    guard let url = urls.first else { return }
                    if picking == .eagle {
                        Task { await model.pickEagle(url) }
                    } else {
                        Task { await model.open(picked: url) }
                    }
                case let .failure(error):
                    model.error = AppModel.describe(error)
                }
            }
            .onAppear { model.reloadLists() }
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

/// The name of a new library; it goes to On My iPhone.
struct CreateLibrarySheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
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
        .presentationDetents([.medium])
    }

    private func create() {
        let name = name
        dismiss()
        Task { await model.create(named: name) }
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
