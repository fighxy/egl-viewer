import SwiftUI

/// Create a library in On My iPhone, open one from Files, or reopen a recent one.
struct WelcomeScreen: View {
    @Environment(AppModel.self) private var model
    @State private var creating = false
    @State private var picking = false

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
                        picking = true
                    } label: {
                        Label("welcome.open", systemImage: "folder")
                    }
                    .accessibilityIdentifier("welcome.open")
                } footer: {
                    Text("welcome.hint")
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
            .overlay {
                if model.busy { ProgressView().controlSize(.large) }
            }
            .disabled(model.busy)
            .sheet(isPresented: $creating) {
                CreateLibrarySheet()
            }
            .fileImporter(
                isPresented: $picking,
                allowedContentTypes: LibraryLocations.openTypes,
                allowsMultipleSelection: false
            ) { result in
                switch result {
                case let .success(urls):
                    if let url = urls.first {
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
