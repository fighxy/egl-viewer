import SwiftUI
import UniformTypeIdentifiers

/// Create a library in On My iPhone, open one from Files, import an Eagle library (D-116), or
/// reopen a recent one.
struct WelcomeScreen: View {
    @Environment(AppModel.self) private var model
    @State private var creating = false
    @State private var picking: Picking?
    @State private var settings = false

    /// What the Files picker is open for: one picker serves both, as SwiftUI allows one.
    private enum Picking { case library, eagle }

    var body: some View {
        NavigationStack {
            List {
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
                } footer: {
                    Text("welcome.hint")
                }

                Section {
                    Button {
                        picking = .eagle
                    } label: {
                        Label("welcome.importEagle", systemImage: "square.and.arrow.down.on.square")
                    }
                    .accessibilityIdentifier("welcome.importEagle")
                } footer: {
                    Text("welcome.importEagleHint")
                }

                if !model.recents.isEmpty {
                    Section("welcome.recent") {
                        ForEach(model.recents) { recent in
                            Button {
                                Task { await model.open(recent: recent) }
                            } label: {
                                LibraryRow(name: recent.name, detail: recent.path)
                            }
                            .accessibilityIdentifier("welcome.recent.\(recent.name)")
                            .swipeActions {
                                Button("welcome.forget", role: .destructive) {
                                    model.forget(recent)
                                }
                            }
                        }
                    }
                }

                let local = model.localLibraries.filter { url in
                    !model.recents.contains { $0.path == url.path }
                }
                if !local.isEmpty {
                    Section("welcome.onThisPhone") {
                        ForEach(local, id: \.self) { url in
                            Button {
                                Task { await model.openLocal(url) }
                            } label: {
                                LibraryRow(
                                    name: url.deletingPathExtension().lastPathComponent,
                                    detail: String(localized: "welcome.onThisPhone")
                                )
                            }
                        }
                    }
                }
            }
            .navigationTitle("app.name")
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
}

private struct LibraryRow: View {
    let name: String
    let detail: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(name)
                .foregroundStyle(EglTokens.foreground)
            Text(detail)
                .font(.caption)
                .foregroundStyle(EglTokens.mutedForeground)
                .lineLimit(1)
                .truncationMode(.middle)
        }
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
