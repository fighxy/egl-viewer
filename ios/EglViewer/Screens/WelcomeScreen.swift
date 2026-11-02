import SwiftUI

struct WelcomeScreen: View {
    @EnvironmentObject private var bridge: LibraryBridge
    @Binding var path: NavigationPath

    private let recents: [RecentLibrary] = [
        RecentLibrary(
            name: "Refs",
            path: "/On My iPhone/Refs.eglib",
            locationHint: "On My iPhone",
            missing: false
        ),
        RecentLibrary(
            name: "Moodboard",
            path: "/iCloud Drive/Moodboard.eglib",
            locationHint: "iCloud",
            missing: false
        ),
        RecentLibrary(
            name: "Archive",
            path: "/Files/Archive.eglib",
            locationHint: "Files",
            missing: true
        ),
    ]

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
                        bridge.createLibrary(named: "Demo")
                        path.append(AppRoute.browse)
                    } label: {
                        Text("Create library")
                            .font(EglTokens.titleFont)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(EglTokens.foreground)

                    Button {
                        // Document picker is stubbed in this draft.
                        bridge.open(path: "/On My iPhone/Refs.eglib", syncFolder: false)
                        path.append(AppRoute.browse)
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

                    ForEach(recents) { recent in
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
                    }
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
    }
}
