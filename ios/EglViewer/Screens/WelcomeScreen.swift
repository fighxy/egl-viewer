import SwiftUI

struct WelcomeScreen: View {
    @EnvironmentObject private var bridge: LibraryBridge
    @Binding var path: NavigationPath

    private let recents: [RecentLibrary] = [
        RecentLibrary(
            name: "Studio Refs",
            path: "/iCloud Drive/Studio Refs.eglib",
            locationHint: "iCloud Drive · Yesterday 18:12",
            missing: false
        ),
        RecentLibrary(
            name: "Travel 2025",
            path: "/On My iPhone/Travel 2025.eglib",
            locationHint: "On My iPhone · Oct 28",
            missing: false
        ),
        RecentLibrary(
            name: "Archive.eglib",
            path: "/Files/Archive.eglib",
            locationHint: "Files · missing",
            missing: true
        ),
    ]

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
                        bridge.createLibrary(named: "Demo")
                        path.append(AppRoute.browse)
                    } label: {
                        Label("Create library", systemImage: "plus")
                            .font(.system(size: 15, weight: .semibold))
                            .frame(maxWidth: .infinity)
                            .frame(height: 48)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(EglTokens.primary)

                    Button {
                        bridge.open(path: "/On My iPhone/Refs.eglib", syncFolder: false)
                        path.append(AppRoute.browse)
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

                    ForEach(Array(recents.enumerated()), id: \.element.id) { index, recent in
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
                    }
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
    }
}
