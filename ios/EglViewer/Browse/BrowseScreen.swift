import EglCore
import SwiftUI

/// The items of one place: a lazy grid or a list, with sort, thumbnail size, search, filters
/// and a selection mode (D-094).
struct BrowseScreen: View {
    @State private var model: BrowseModel
    @State private var filtering = false
    @State private var emptying = false

    init(open: OpenLibrary, place: Place) {
        _model = State(initialValue: BrowseModel(open: open, place: place))
    }

    var body: some View {
        Group {
            switch model.settings.layout {
            case .grid: BrowseGrid(model: model)
            case .list: BrowseList(model: model)
            }
        }
        .overlay {
            if !model.loading, model.total == 0 {
                emptyState
            }
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(model.selecting)
        .searchable(text: model.binding(\.search), prompt: Text("browse.search"))
        .autocorrectionDisabled()
        .textInputAutocapitalization(.never)
        .toolbar { toolbar }
        .safeAreaInset(edge: .bottom) {
            if model.selecting {
                SelectionBar(model: model)
            }
        }
        .toast(model.open, alignment: .bottom, padding: model.selecting ? 96 : 16, shown: model.previewing == nil)
        .sheet(isPresented: $filtering) {
            FilterSheet(model: model)
        }
        .fullScreenCover(item: $model.previewing) { start in
            PreviewScreen(model: model, start: start.index)
        }
        .sheet(item: $model.details) { cell in
            DetailsSheet(open: model.open, cell: cell)
        }
        .sheet(item: $model.sharing) { files in
            ShareSheet(files: files)
                .presentationDetents([.medium, .large])
        }
        .confirmationDialog("trash.emptyConfirm", isPresented: $emptying, titleVisibility: .visible) {
            Button("trash.empty", role: .destructive) {
                Task { await model.emptyTrash() }
            }
            .accessibilityIdentifier("trash.emptyConfirmed")
        } message: {
            Text("trash.noUndo")
        }
        .task { await model.start() }
        .onDisappear {
            // The full-screen preview hides this screen too; its view stays open for it.
            if model.previewing == nil {
                model.stop()
            }
        }
    }

    private var title: Text {
        switch model.place {
        case .all: Text("place.all")
        case .uncategorized: Text("place.uncategorized")
        case .untagged: Text("place.untagged")
        case .trash: Text("place.trash")
        case let .folder(_, name), let .smart(_, name): Text(verbatim: name)
        }
    }

    @ViewBuilder
    private var emptyState: some View {
        if model.settings.filters.isActive || !model.settings.search.isEmpty {
            ContentUnavailableView {
                Label("browse.noMatches", systemImage: "magnifyingglass")
            } actions: {
                Button("filter.reset") {
                    model.resetFilters()
                    model.change { $0.search = "" }
                }
            }
        } else if model.place == .trash {
            ContentUnavailableView("browse.trashEmpty", systemImage: "trash")
        } else {
            ContentUnavailableView("browse.empty", systemImage: "photo.on.rectangle")
        }
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        if model.selecting {
            ToolbarItem(placement: .topBarLeading) {
                if model.allSelected || model.selectedCount == model.total, model.total > 0 {
                    Button("browse.deselectAll") { model.deselectAll() }
                        .accessibilityIdentifier("browse.deselectAll")
                } else {
                    Button("browse.selectAll") { model.selectAll() }
                        .accessibilityIdentifier("browse.selectAll")
                }
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button("common.done") { model.setSelecting(false) }
                    .fontWeight(.semibold)
                    .accessibilityIdentifier("browse.done")
            }
        } else {
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button {
                    filtering = true
                } label: {
                    Label("browse.filter", systemImage: filterSymbol)
                }
                .accessibilityIdentifier("browse.filter")
                OptionsMenu(model: model)
                Button("browse.select") { model.setSelecting(true) }
                    .disabled(model.total == 0)
                    .accessibilityIdentifier("browse.select")
            }
            if model.place == .trash, model.total > 0, !model.open.readOnly {
                ToolbarItem(placement: .bottomBar) {
                    Button("trash.empty", role: .destructive) { emptying = true }
                        .accessibilityIdentifier("trash.empty")
                }
            }
        }
    }

    private var filterSymbol: String {
        model.settings.filters.isActive
            ? "line.3.horizontal.decrease.circle.fill"
            : "line.3.horizontal.decrease.circle"
    }
}

/// Layout, thumbnail size and sort.
private struct OptionsMenu: View {
    let model: BrowseModel

    var body: some View {
        Menu {
            Picker(selection: model.binding(\.layout)) {
                Label("browse.layout.grid", systemImage: "square.grid.2x2").tag(BrowseLayout.grid)
                Label("browse.layout.list", systemImage: "list.bullet").tag(BrowseLayout.list)
            } label: {
                Text("browse.layout")
            }
            .pickerStyle(.inline)
            if model.settings.layout == .grid {
                Picker(selection: model.binding(\.size)) {
                    Text("browse.size.small").tag(ThumbnailSize.small)
                    Text("browse.size.medium").tag(ThumbnailSize.medium)
                    Text("browse.size.large").tag(ThumbnailSize.large)
                } label: {
                    Label("browse.size", systemImage: "photo")
                }
                .pickerStyle(.menu)
            }
            Picker(selection: model.binding(\.sort)) {
                ForEach(SortKey.offered, id: \.self) { key in
                    Text(SortLabel.of(key)).tag(key)
                }
            } label: {
                Label("browse.sort", systemImage: "arrow.up.arrow.down")
            }
            .pickerStyle(.menu)
            if model.settings.sort != .random {
                Toggle(isOn: model.binding(\.ascending)) {
                    Text("browse.ascending")
                }
            }
        } label: {
            Label("browse.options", systemImage: "ellipsis.circle")
        }
        .accessibilityIdentifier("browse.options")
    }
}

enum SortLabel {
    static func of(_ key: SortKey) -> LocalizedStringKey {
        switch key {
        case .added: "sort.added"
        case .btime: "sort.created"
        case .mtime: "sort.modified"
        case .name: "sort.name"
        case .ext: "sort.type"
        case .size: "sort.size"
        case .resolution: "sort.resolution"
        case .star: "sort.rating"
        case .random: "sort.random"
        case .duration, .tag, .manual: "sort.added"
        }
    }
}

// MARK: Grid and list

private struct BrowseGrid: View {
    let model: BrowseModel
    @Environment(\.displayScale) private var scale

    var body: some View {
        let size = model.settings.size
        ScrollView {
            CountLine(model: model)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: size.minimumWidth), spacing: 4)], spacing: 8) {
                ForEach(0..<model.total, id: \.self) { index in
                    GridTile(model: model, index: index, pixels: Int(size.minimumWidth * 1.6 * scale))
                        .onAppear { model.need(index) }
                }
            }
            .padding(.horizontal, 4)
        }
        .accessibilityIdentifier("browse.grid")
    }
}

private struct BrowseList: View {
    let model: BrowseModel
    @Environment(\.displayScale) private var scale

    var body: some View {
        List {
            CountLine(model: model)
                .listRowSeparator(.hidden)
            ForEach(0..<model.total, id: \.self) { index in
                ListRow(model: model, index: index, pixels: Int(48 * scale))
                    .onAppear { model.need(index) }
            }
        }
        .listStyle(.plain)
        .accessibilityIdentifier("browse.list")
    }
}

private struct CountLine: View {
    let model: BrowseModel

    var body: some View {
        if !model.loading {
            Text("browse.count \(model.total)")
                .font(.footnote)
                .foregroundStyle(EglTokens.mutedForeground)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 12)
                .padding(.vertical, 4)
                .accessibilityIdentifier("browse.count")
        }
    }
}

private struct GridTile: View {
    let model: BrowseModel
    let index: Int
    let pixels: Int

    var body: some View {
        let cell = model.cell(at: index)
        let selected = cell.map { model.isSelected($0.id) } ?? false
        VStack(spacing: 3) {
            RoundedRectangle(cornerRadius: 6)
                .fill(EglTokens.grid)
                .aspectRatio(1, contentMode: .fit)
                .overlay {
                    if let cell {
                        ItemThumbnail(cell: cell, cache: model.open.thumbnails, pixels: pixels)
                            .padding(4)
                    }
                }
                .overlay {
                    if selected {
                        RoundedRectangle(cornerRadius: 6)
                            .strokeBorder(EglTokens.accent, lineWidth: 3)
                    }
                }
                .overlay(alignment: .topTrailing) {
                    if model.selecting, cell != nil {
                        SelectionMark(selected: selected)
                            .padding(6)
                    }
                }
            Text(cell?.name ?? " ")
                .font(.caption2)
                .foregroundStyle(EglTokens.mutedForeground)
                .lineLimit(1)
                .truncationMode(.middle)
        }
        .contentShape(Rectangle())
        .onTapGesture {
            if let cell { model.tap(cell, at: index) }
        }
        .contextMenu {
            if let cell { ItemMenu(model: model, cell: cell) }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: cell?.name ?? ""))
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
        .accessibilityIdentifier(cell.map { "item.\($0.name)" } ?? "item.loading")
    }
}

private struct ListRow: View {
    let model: BrowseModel
    let index: Int
    let pixels: Int

    var body: some View {
        let cell = model.cell(at: index)
        let selected = cell.map { model.isSelected($0.id) } ?? false
        HStack(spacing: 12) {
            if model.selecting {
                SelectionMark(selected: selected)
            }
            RoundedRectangle(cornerRadius: 4)
                .fill(EglTokens.grid)
                .frame(width: 48, height: 48)
                .overlay {
                    if let cell {
                        ItemThumbnail(cell: cell, cache: model.open.thumbnails, pixels: pixels)
                    }
                }
            VStack(alignment: .leading, spacing: 2) {
                Text(verbatim: cell?.name ?? " ")
                    .lineLimit(1)
                    .truncationMode(.middle)
                if let cell {
                    Text(verbatim: CellDetails.line(cell))
                        .font(.caption)
                        .foregroundStyle(EglTokens.mutedForeground)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            if let cell, cell.star > 0 {
                Text(verbatim: String(repeating: "★", count: cell.star))
                    .font(.caption)
                    .foregroundStyle(EglTokens.mutedForeground)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            if let cell { model.tap(cell, at: index) }
        }
        .contextMenu {
            if let cell { ItemMenu(model: model, cell: cell) }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
        .accessibilityIdentifier(cell.map { "item.\($0.name)" } ?? "item.loading")
    }
}

/// Type, dimensions and size of a cell, for the list.
enum CellDetails {
    static func line(_ cell: ItemCell) -> String {
        var parts = [cell.ext.uppercased()]
        if let width = cell.width, let height = cell.height {
            parts.append("\(width) × \(height)")
        }
        if let size = cell.size {
            parts.append(ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file))
        }
        return parts.joined(separator: " · ")
    }
}

private struct SelectionMark: View {
    let selected: Bool

    var body: some View {
        Image(systemName: selected ? "checkmark.circle.fill" : "circle")
            .font(.title3)
            .symbolRenderingMode(.palette)
            .foregroundStyle(selected ? EglTokens.background : EglTokens.foreground, selected ? EglTokens.accent : EglTokens.background.opacity(0.6))
            .accessibilityHidden(true)
    }
}

/// A thumbnail, or the file type while it has none.
struct ItemThumbnail: View {
    let cell: ItemCell
    let cache: ThumbnailCache
    let pixels: Int
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image {
                Image(uiImage: image)
                    .resizable()
                    .scaledToFit()
            } else if let colour = placeholder {
                colour.opacity(0.35)
            } else {
                Text(verbatim: cell.ext.uppercased())
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(EglTokens.mutedForeground)
            }
        }
        .task(id: "\(cell.id)#\(cell.thumbVersion)#\(pixels)") {
            image = await cache.image(for: cell, pixels: pixels)
        }
    }

    /// The main colour of the image, shown until the thumbnail arrives.
    private var placeholder: Color? {
        guard cell.hasThumbnail, let rgb = cell.palette0, rgb.count >= 3 else { return nil }
        return Color(red: Double(rgb[0]) / 255, green: Double(rgb[1]) / 255, blue: Double(rgb[2]) / 255)
    }
}
