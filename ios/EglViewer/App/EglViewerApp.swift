import SwiftUI

@main
struct EglViewerApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
        }
    }
}

/// Welcome, or the open library.
struct RootView: View {
    @Environment(AppModel.self) private var model

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
    }
}
