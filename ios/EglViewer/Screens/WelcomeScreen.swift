import SwiftUI
import UniformTypeIdentifiers

struct WelcomeScreen: View {
    @EnvironmentObject private var bridge: LibraryBridge
    @ObservedObject private var access = LibraryAccess.shared
    @Binding var path: NavigationPath

    @State private var showOpenPicker = false
    @State private var showCreateParentPicker = false
    @State private var pendingCreateName = "Demo"
    @State private var showCreateNameAlert = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("egl-viewer")
                        .font(EglTokens.heroFont)
                        .foregroundStyle(EglTokens.foreground)
                    Text("Open a native .eglib library from Files or iCloud.")
                        .font(EglTokens.chromeFont)
                        .foregroundStyle(EglTokens.mutedForeground)
                }
                .padding(.top, 24)

                VStack(spacing: 10) {
                    Button {
                        showCreateNameAlert = true
                    } label: {
                        Text("Create library")
                            .font(EglTokens.titleFont)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(EglTokens.foreground)

                    Button {
                        showOpenPicker = true
                    } label: {
                        Text("Open .eglib…")
                            .font(EglTokens.titleFont)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                    }
                    .buttonStyle(.bordered)
                }

                VStack(alignment: .leading, spacing: 8) {
                    Text("Recent")
                        .font(EglTokens.chromeFont)
                        .foregroundStyle(EglTokens.mutedForeground)

                    if access.recents.isEmpty {
                        Text("No recent libraries yet.")
                            .font(EglTokens.chromeFont)
                            .foregroundStyle(EglTokens.mutedForeground)
                            .padding(.vertical, 8)
                    }

                    ForEach(access.recents) { recent in
                        Button {
                            guard !recent.missing else { return }
                            bridge.openRecent(recent)
                            path.append(AppRoute.browse)
                        } label: {
                            HStack {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(recent.name)
                                        .font(EglTokens.titleFont)
                                        .foregroundStyle(
                                            recent.missing
                                                ? EglTokens.mutedForeground
                                                : EglTokens.foreground
                                        )
                                    Text(recent.locationHint)
                                        .font(EglTokens.chromeFont)
                                        .foregroundStyle(EglTokens.mutedForeground)
                                }
                                Spacer()
                                if recent.missing {
                                    Text("Missing")
                                        .font(EglTokens.chromeFont)
                                        .foregroundStyle(EglTokens.mutedForeground)
                                }
                            }
                            .padding(12)
                            .background(EglTokens.sidebar)
                            .overlay(
                                Rectangle()
                                    .stroke(EglTokens.border, lineWidth: 1)
                            )
                        }
                        .disabled(recent.missing)
                        .contextMenu {
                            Button("Forget", role: .destructive) {
                                access.forget(path: recent.path)
                            }
                        }
                    }
                }

                if let error = bridge.lastError {
                    Text(error)
                        .font(EglTokens.chromeFont)
                        .foregroundStyle(EglTokens.mutedForeground)
                }

                Text("Native .eglib only — no Eagle on this client.")
                    .font(EglTokens.chromeFont)
                    .foregroundStyle(EglTokens.mutedForeground)
                    .padding(.top, 8)
            }
            .padding(.horizontal, 20)
            .padding(.bottom, 32)
        }
        .background(EglTokens.background.ignoresSafeArea())
        .navigationBarHidden(true)
        .fileImporter(
            isPresented: $showOpenPicker,
            allowedContentTypes: LibraryAccess.openContentTypes,
            allowsMultipleSelection: false
        ) { result in
            handleOpen(result)
        }
        .fileImporter(
            isPresented: $showCreateParentPicker,
            allowedContentTypes: [.folder, .directory],
            allowsMultipleSelection: false
        ) { result in
            handleCreateParent(result)
        }
        .alert("Library name", isPresented: $showCreateNameAlert) {
            TextField("Name", text: $pendingCreateName)
            Button("Choose folder…") {
                showCreateParentPicker = true
            }
            Button("Mock only", role: .cancel) {
                bridge.createLibrary(named: pendingCreateName, parentURL: nil)
                path.append(AppRoute.browse)
            }
        } message: {
            Text(
                bridge.ffiLinked
                    ? "Pick a parent folder for Name.eglib (security-scoped)."
                    : "FFI not linked — Create uses mock data unless you link libegl_library.a."
            )
        }
        .onAppear {
            access.reloadFromDefaults()
        }
    }

    private func handleOpen(_ result: Result<[URL], Error>) {
        switch result {
        case let .success(urls):
            guard let url = urls.first else { return }
            bridge.openPickedLibrary(url: url)
            path.append(AppRoute.browse)
        case let .failure(error):
            bridge.lastError = error.localizedDescription
        }
    }

    private func handleCreateParent(_ result: Result<[URL], Error>) {
        switch result {
        case let .success(urls):
            guard let url = urls.first else { return }
            let accessed = url.startAccessingSecurityScopedResource()
            bridge.createLibrary(named: pendingCreateName, parentURL: url)
            if accessed {
                url.stopAccessingSecurityScopedResource()
            }
            path.append(AppRoute.browse)
        case let .failure(error):
            bridge.lastError = error.localizedDescription
        }
    }
}
