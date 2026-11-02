import SwiftUI

@main
struct EglViewerApp: App {
    @StateObject private var bridge = LibraryBridge()

    var body: some Scene {
        WindowGroup {
            RootNavigation()
                .environmentObject(bridge)
        }
    }
}

struct RootNavigation: View {
    @EnvironmentObject private var bridge: LibraryBridge
    @State private var path = NavigationPath()

    var body: some View {
        NavigationStack(path: $path) {
            WelcomeScreen(path: $path)
                .navigationDestination(for: AppRoute.self) { route in
                    switch route {
                    case .browse:
                        BrowseScreen(path: $path)
                    case let .preview(itemId):
                        PreviewScreen(path: $path, itemId: itemId)
                    }
                }
        }
        .tint(EglTokens.foreground)
    }
}
