import SwiftUI

struct BrowseScreen: View {
    @EnvironmentObject private var bridge: LibraryBridge
    @Binding var path: NavigationPath
    @State private var place: Place = .all
    @State private var showFilters = false
    @State private var selectedTab: LibraryTab = .grid

    enum Place: String, CaseIterable {
        case all = "All"
        case uncategorized = "Uncategorized"
        case untagged = "Untagged"
        case trash = "Trash"
    }

    enum LibraryTab: String {
        case grid = "Grid"
        case places = "Places"
        case tags = "Tags"
        case more = "More"
    }

    private let columns = [
        GridItem(.adaptive(minimum: 96, maximum: 140), spacing: 4),
    ]

    var body: some View {
        VStack(spacing: 0) {
            placesStrip
            if showFilters {
                filterBar
            }
            content
            tabBar
        }
        .background(EglTokens.background.ignoresSafeArea())
        .navigationTitle(bridge.libraryName)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button("Libraries") {
                    bridge.close()
                    path = NavigationPath()
                }
                .font(EglTokens.chromeFont)
            }
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button {
                    showFilters.toggle()
                } label: {
                    Image(systemName: "line.3.horizontal.decrease")
                }
                Button {} label: {
                    Image(systemName: "plus")
                }
            }
        }
        .safeAreaInset(edge: .bottom) {
            if bridge.usingMockData {
                Text("Mock data — link libegl_library.a for a real .eglib")
                    .font(EglTokens.chromeFont)
                    .foregroundStyle(EglTokens.mutedForeground)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 6)
                    .background(EglTokens.muted)
            }
        }
    }

    private var placesStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(Place.allCases, id: \.self) { chip in
                    Button(chipLabel(chip)) {
                        place = chip
                    }
                    .buttonStyle(PlaceChipStyle(active: place == chip))
                }
                Button("Clients ▾") {}
                    .buttonStyle(PlaceChipStyle(active: false))
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }
        .background(EglTokens.sidebar)
    }

    private var filterBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                filterPill("Tags", active: true)
                filterPill("Folders", active: false)
                filterPill("Ext", active: false)
                filterPill("Rating", active: false)
                filterPill("Shape", active: false)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }
        .background(EglTokens.background)
        .overlay(alignment: .bottom) {
            Rectangle().fill(EglTokens.border).frame(height: 1)
        }
    }

    private var content: some View {
        Group {
            switch selectedTab {
            case .grid:
                grid
            case .places:
                listPanel(title: "Places", rows: bridge.folders.map(\.name))
            case .tags:
                listPanel(title: "Tags", rows: bridge.tags.map(\.name))
            case .more:
                listPanel(
                    title: "More",
                    rows: [
                        bridge.info?.readOnly == true ? "Read-only" : "Writable",
                        "Library info",
                        "Settings",
                    ]
                )
            }
        }
    }

    private var grid: some View {
        ScrollView {
            LazyVGrid(columns: columns, spacing: 4) {
                ForEach(bridge.items) { item in
                    Button {
                        path.append(AppRoute.preview(itemId: item.id))
                    } label: {
                        ZStack(alignment: .bottomLeading) {
                            Rectangle()
                                .fill(EglTokens.muted)
                            Text(item.ext.uppercased())
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundStyle(EglTokens.mutedForeground)
                                .padding(6)
                        }
                        .aspectRatio(itemAspect(item), contentMode: .fit)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(4)
        }
        .background(EglTokens.grid)
    }

    private func listPanel(title: String, rows: [String]) -> some View {
        List {
            Section(title) {
                ForEach(rows, id: \.self) { row in
                    Text(row)
                        .font(EglTokens.chromeFont)
                }
            }
        }
        .listStyle(.plain)
    }

    private var tabBar: some View {
        HStack {
            tabButton(.grid, systemImage: "square.grid.2x2")
            tabButton(.places, systemImage: "folder")
            tabButton(.tags, systemImage: "tag")
            tabButton(.more, systemImage: "ellipsis")
        }
        .padding(.vertical, 8)
        .background(EglTokens.sidebar)
        .overlay(alignment: .top) {
            Rectangle().fill(EglTokens.border).frame(height: 1)
        }
    }

    private func tabButton(_ tab: LibraryTab, systemImage: String) -> some View {
        Button {
            selectedTab = tab
        } label: {
            VStack(spacing: 2) {
                Image(systemName: systemImage)
                Text(tab.rawValue)
                    .font(.system(size: 11))
            }
            .foregroundStyle(selectedTab == tab ? EglTokens.foreground : EglTokens.mutedForeground)
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(.plain)
    }

    private func filterPill(_ title: String, active: Bool) -> some View {
        Text(title)
            .font(EglTokens.chromeFont)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(active ? EglTokens.foreground : Color.clear)
            .foregroundStyle(active ? Color.white : EglTokens.foreground)
            .overlay(
                Capsule().stroke(EglTokens.border, lineWidth: active ? 0 : 1)
            )
            .clipShape(Capsule())
    }

    private func chipLabel(_ place: Place) -> String {
        switch place {
        case .all:
            return "All · \(bridge.total)"
        default:
            return place.rawValue
        }
    }

    private func itemAspect(_ item: ItemCellDTO) -> CGFloat {
        let width = CGFloat(item.width ?? 1)
        let height = CGFloat(item.height ?? 1)
        return max(width / max(height, 1), 0.55)
    }
}
