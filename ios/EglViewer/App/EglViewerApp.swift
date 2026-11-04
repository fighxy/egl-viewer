import SwiftUI

@main
struct EglViewerApp: App {
    @State private var model = AppModel()
    @State private var lock = AppLock.standard()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .environment(lock)
        }
    }
}

/// Welcome, or the open library; the app's login over it (D-098).
struct RootView: View {
    @Environment(AppModel.self) private var model
    @Environment(AppLock.self) private var lock
    @Environment(\.scenePhase) private var phase
    @State private var cover = CoverWindow()

    var body: some View {
        Group {
            switch model.screen {
            case .welcome:
                WelcomeScreen()
            case let .library(open):
                LibraryScreen(open: open)
            }
        }
        .tint(EglTokens.accent)
        .alert(
            "error.title",
            isPresented: Binding(get: { model.error != nil }, set: { if !$0 { model.error = nil } }),
            presenting: model.error
        ) { _ in
            Button("common.ok", role: .cancel) {}
        } message: { message in
            Text(message)
        }
        .onAppear(perform: showCover)
        .onChange(of: lock.locked) { showCover() }
        .onChange(of: phase) { _, phase in
            switch phase {
            case .background:
                // Leaving the screen closes the open locked folders and the app.
                lock.lock()
                if let open = model.openLibrary {
                    Task { await open.relockAll() }
                }
            case .inactive:
                // The app switcher takes its picture now.
                if lock.enabled, !lock.locked { cover.show(PrivacyCover()) }
            case .active:
                showCover()
            @unknown default:
                break
            }
        }
    }

    private func showCover() {
        if lock.locked {
            cover.show(LoginScreen(lock: lock))
        } else {
            cover.hide()
        }
    }
}
