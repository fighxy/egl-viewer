import SwiftUI

struct BrowseScreen: View {
    @EnvironmentObject private var bridge: LibraryBridge
    @Binding var path: NavigationPath
    @State private var place: Place = .all
    @State private var showFilters = true
    @State private var detailsPressed = true
    @State private var selectedTab: LibraryTab = .grid
    @State private var search = ""

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
        GridItem(.adaptive(minimum: 96, maximum: 140), spacing: 3),
    ]

    var body: some View {
        VStack(spacing: 0) {
            placesStrip
            searchToolbar
            if showFilters {
                filterBar
            }
            content
            if bridge.usingMockData {
                Text(
                    bridge.ffiLinked
                        ? "Mock fallback — could not open via FFI"
                        : "Mock data — link libegl_library.a for a real .eglib"
                )
                .font(EglTokens.chromeFont)
                .foregroundStyle(EglTokens.mutedForeground)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 6)
                .background(EglTokens.muted)
            }
            tabBar
        }
        .background(EglTokens.background.ignoresSafeArea())
        .navigationTitle(bridge.libraryName.isEmpty ? "Library" : bridge.libraryName)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button {
                    bridge.close()
                    path = NavigationPath()
                } label: {
                    Image(systemName: "chevron.left")
                }
            }
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button {} label: {
                    Image(systemName: "plus")
                }
                Button {
                    detailsPressed.toggle()
                } label: {
                    Image(systemName: "sidebar.right")
                        .padding(4)
                        .background(detailsPressed ? EglTokens.muted : Color.clear)
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                }
            }
        }
    }

    private var placesStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                Button {
                    place = .all
                } label: {
                    HStack(spacing: 6) {
                        Text(Place.all.rawValue)
                        Text("\(bridge.total)")
                            .font(EglTokens.captionFont)
                            .foregroundStyle(EglTokens.mutedForeground)
                    }
                }
                .buttonStyle(PlaceChipStyle(active: place == .all))

                ForEach([Place.uncategorized, .untagged], id: \.self) { chip in
                    Button {
                        place = chip
                    } label: {
                        HStack(spacing: 6) {
                            Text(chip.rawValue)
                            Text(chip == .uncategorized ? "42" : "19")
                                .font(EglTokens.captionFont)
                                .foregroundStyle(EglTokens.mutedForeground)
                        }
                    }
                    .buttonStyle(PlaceChipStyle(active: place == chip))
                }

                Button("Clients ▾") {}
                    .buttonStyle(PlaceChipStyle(active: false))

                Button {
                    place = .trash
                } label: {
                    HStack(spacing: 6) {
                        Text(Place.trash.rawValue)
                        Text("3")
                            .font(EglTokens.captionFont)
                            .foregroundStyle(EglTokens.mutedForeground)
                    }
                }
                .buttonStyle(PlaceChipStyle(active: place == .trash))
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }
        .background(EglTokens.sidebar)
        .overlay(alignment: .bottom) {
            Rectangle().fill(EglTokens.border).frame(height: 1)
        }
    }

    private var searchToolbar: some View {
        HStack(spacing: 8) {
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 12))
                    .foregroundStyle(EglTokens.mutedForeground)
                TextField("Search", text: $search)
                    .font(EglTokens.chromeFont)
            }
            .padding(.horizontal, 10)
            .frame(height: 30)
            .background(EglTokens.muted)
            .clipShape(RoundedRectangle(cornerRadius: 8))

            Button {
                showFilters.toggle()
            } label: {
                Image(systemName: "line.3.horizontal.decrease")
                    .frame(width: 32, height: 32)
                    .background(showFilters ? EglTokens.muted : Color.clear)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            Button {} label: {
                Image(systemName: "arrow.up.arrow.down")
                    .frame(width: 32, height: 32)
            }
        }
        .padding(.horizontal, 12)
        .frame(height: 40)
        .overlay(alignment: .bottom) {
            Rectangle().fill(EglTokens.border).frame(height: 1)
        }
    }

    private var filterBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                pill("Tags: poster · logo", active: true)
                pill("Folders", active: false)
                pill("Ext", active: false)
                pill("★ Rating", active: false)
                pill("Shape", active: false)
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
                        bridge.ffiLinked ? "FFI linked" : "FFI mock",
                        "Library info",
                        "Settings",
                    ]
                )
            }
        }
    }

    private var grid: some View {
        ScrollView {
            LazyVGrid(columns: columns, spacing: 3) {
                ForEach(bridge.items) { item in
                    Button {
                        path.append(AppRoute.preview(itemId: item.id))
                    } label: {
                        ItemThumbCell(item: item)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(6)
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
            tabButton(.places, systemImage: "list.bullet")
            tabButton(.tags, systemImage: "square.and.arrow.up")
            tabButton(.more, systemImage: "ellipsis.circle")
        }
        .padding(.top, 6)
        .padding(.bottom, 18)
        .background(EglTokens.background)
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
                    .font(.system(size: 10, weight: selectedTab == tab ? .semibold : .regular))
            }
            .foregroundStyle(selectedTab == tab ? EglTokens.foreground : EglTokens.mutedForeground)
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(.plain)
    }

    private func pill(_ title: String, active: Bool) -> some View {
        Text(title)
            .font(.system(size: 12))
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(active ? EglTokens.primary : EglTokens.background)
            .foregroundStyle(active ? EglTokens.primaryForeground : EglTokens.foreground)
            .overlay(
                RoundedRectangle(cornerRadius: 7)
                    .stroke(active ? EglTokens.primary : EglTokens.border, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 7))
    }
}

struct ItemThumbCell: View {
    @EnvironmentObject private var bridge: LibraryBridge
    let item: ItemCellDTO

    var body: some View {
        ZStack(alignment: .bottomLeading) {
            if let image = bridge.thumbnailImage(id: item.id) {
                Image(uiImage: image)
                    .resizable()
                    .scaledToFill()
            } else {
                Rectangle()
                    .fill(EglTokens.muted)
                Text(item.ext.uppercased())
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(EglTokens.mutedForeground)
                    .padding(6)
            }
        }
        .aspectRatio(itemAspect(item), contentMode: .fit)
        .clipped()
    }

    private func itemAspect(_ item: ItemCellDTO) -> CGFloat {
        let width = CGFloat(item.width ?? 1)
        let height = CGFloat(item.height ?? 1)
        return max(width / max(height, 1), 0.55)
    }
}
