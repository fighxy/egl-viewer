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
            VStack(alignment: .leading, spacing: 20) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("egl-viewer")
                        .font(EglTokens.heroFont)
                        .tracking(-0.5)
                        .foregroundStyle(EglTokens.foreground)
                    Text("Open a native library on this iPhone. Libraries live as .eglib packages in Files or iCloud Drive.")
                        .font(.system(size: 14))
                        .foregroundStyle(EglTokens.mutedForeground)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.top, 28)

                VStack(spacing: 10) {
                    Button {
                        showCreateNameAlert = true
                    } label: {
                        Label("Create library", systemImage: "plus")
                            .font(.system(size: 15, weight: .semibold))
                            .frame(maxWidth: .infinity)
                            .frame(height: 48)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(EglTokens.primary)

                    Button {
                        showOpenPicker = true
                    } label: {
                        Label("Open .eglib…", systemImage: "folder")
                            .font(.system(size: 15, weight: .semibold))
                            .frame(maxWidth: .infinity)
                            .frame(height: 48)
                    }
                    .buttonStyle(.bordered)
                }

                VStack(alignment: .leading, spacing: 8) {
                    Text("Recent")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(EglTokens.mutedForeground)

                    if access.recents.isEmpty {
                        Text("No recent libraries yet.")
                            .font(EglTokens.chromeFont)
                            .foregroundStyle(EglTokens.mutedForeground)
                            .padding(.vertical, 8)
                    }

                    ForEach(Array(access.recents.enumerated()), id: \.element.id) { index, recent in
                        Button {
                            guard !recent.missing else { return }
                            bridge.openRecent(recent)
                            path.append(AppRoute.browse)
                        } label: {
                            HStack(spacing: 10) {
                                ZStack {
                                    RoundedRectangle(cornerRadius: 8)
                                        .fill(EglTokens.sidebar)
                                        .overlay(
                                            RoundedRectangle(cornerRadius: 8)
                                                .stroke(EglTokens.border, lineWidth: 1)
                                        )
                                    Image(systemName: "square.grid.2x2")
                                        .font(.system(size: 12))
                                        .foregroundStyle(EglTokens.mutedForeground.opacity(0.7))
                                }
                                .frame(width: 36, height: 36)

                                VStack(alignment: .leading, spacing: 2) {
                                    Text(recent.name)
                                        .font(.system(size: 14, weight: .semibold))
                                        .foregroundStyle(
                                            recent.missing
                                                ? EglTokens.mutedForeground
                                                : EglTokens.foreground
                                        )
                                        .lineLimit(1)
                                    Text(recent.locationHint)
                                        .font(EglTokens.captionFont)
                                        .foregroundStyle(EglTokens.mutedForeground)
                                        .lineLimit(1)
                                }
                                Spacer(minLength: 0)
                            }
                            .padding(.horizontal, 10)
                            .padding(.vertical, 10)
                            .background(
                                RoundedRectangle(cornerRadius: 10)
                                    .fill(index == 0 && !recent.missing ? EglTokens.muted : Color.clear)
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

                                Text("Native .eglib only · same product as desktop")
                    .font(EglTokens.captionFont)
                    .foregroundStyle(EglTokens.mutedForeground)
                    .frame(maxWidth: .infinity)
                    .padding(.top, 16)
                    .padding(.bottom, 12)
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
