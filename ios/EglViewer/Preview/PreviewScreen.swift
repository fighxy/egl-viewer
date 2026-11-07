import EglCore
import SwiftUI

/// The items of a view one at a time, full screen on the dark stage (D-095): swipe between
/// them, pinch or double-tap to zoom, tap to hide the bars. The bars hold the item's details,
/// its rating, sharing and the trash.
struct PreviewScreen: View {
    let model: BrowseModel
    let start: Int
    @Environment(\.dismiss) private var dismiss
    @Environment(\.displayScale) private var scale
    @State private var position: Int?
    @State private var chrome = true
    @State private var zoomed = false
    @State private var deleting = false
    @State private var details: ItemCell? = nil
    @State private var sharing: ShareFiles? = nil

    init(model: BrowseModel, start: Int) {
        self.model = model
        self.start = start
        _position = State(initialValue: start)
    }

    private var index: Int { min(position ?? start, max(model.total - 1, 0)) }
    private var cell: ItemCell? { model.cell(at: index) }

    var body: some View {
        GeometryReader { geometry in
            let pixels = Int(max(geometry.size.width, geometry.size.height) * scale * 1.5)
            ScrollViewReader { proxy in
                ScrollView(.horizontal) {
                    LazyHStack(spacing: 0) {
                        ForEach(0..<model.total, id: \.self) { index in
                            PreviewPage(model: model, index: index, pixels: pixels, zoomed: $zoomed) {
                                withAnimation(.easeInOut(duration: 0.2)) { chrome.toggle() }
                            }
                            .containerRelativeFrame([.horizontal, .vertical])
                            .id(index)
                            .onAppear { model.need(index) }
                        }
                    }
                    .scrollTargetLayout()
                }
                .scrollTargetBehavior(.paging)
                .scrollPosition(id: $position)
                .modifier(VisiblePage(position: $position))
                .scrollDisabled(zoomed)
                .scrollIndicators(.hidden)
                .accessibilityIdentifier("preview.pager")
                .onAppear {
                    position = start
                    proxy.scrollTo(start, anchor: .leading)
                }
            }
        }
        .ignoresSafeArea()
        .background(EglTokens.previewStage.ignoresSafeArea())
        .overlay(alignment: .top) {
            if chrome { topBar }
        }
        .overlay(alignment: .bottom) {
            if chrome { bottomBar }
        }
        .toast(model.open, alignment: .bottom, padding: chrome ? 88 : 24)
        .statusBarHidden(!chrome)
        .preferredColorScheme(.dark)
        .onChange(of: index) { _, _ in
            // A recycled or previous page must not leave paging locked after a zoom.
            if zoomed { zoomed = false }
        }
        .onChange(of: model.total) { _, total in
            if total == 0 { dismiss() }
        }
        .sheet(item: $details) { cell in
            DetailsSheet(open: model.open, cell: cell)
        }
        .sheet(item: $sharing) { files in
            ShareSheet(files: files)
                .presentationDetents([.medium, .large])
        }
        .confirmationDialog("trash.deleteConfirm \(1)", isPresented: $deleting, titleVisibility: .visible) {
            Button("action.deleteForGood", role: .destructive) {
                if let cell { Task { await model.deleteForGood(ItemsTarget(ids: [cell.id])) } }
            }
        } message: {
            Text("trash.noUndo")
        }
    }

    private var topBar: some View {
        HStack(spacing: 12) {
            Button {
                dismiss()
            } label: {
                Label("common.close", systemImage: "xmark")
                    .labelStyle(.iconOnly)
                    .font(.title3)
                    .frame(width: 44, height: 44)
            }
            .accessibilityIdentifier("preview.close")
            VStack(alignment: .leading, spacing: 1) {
                Text(verbatim: cell?.name ?? "")
                    .font(.headline)
                    .lineLimit(1)
                    .truncationMode(.middle)
                if model.total > 0 {
                    Text("preview.position \(index + 1) \(model.total)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .monospacedDigit()
                        .accessibilityIdentifier("preview.position")
                }
            }
            Spacer()
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(.ultraThinMaterial)
        .transition(.move(edge: .top).combined(with: .opacity))
    }

    private var bottomBar: some View {
        let target = cell.map { ItemsTarget(ids: [$0.id]) }
        let readOnly = model.open.readOnly
        return HStack {
            Button {
                details = cell
            } label: {
                Label("action.info", systemImage: "info.circle")
            }
            .accessibilityIdentifier("preview.info")
            Spacer()
            if model.place == .trash {
                Button {
                    if let target { Task { await model.restore(target) } }
                } label: {
                    Label("action.restore", systemImage: "arrow.uturn.backward")
                }
                .disabled(readOnly)
                .accessibilityIdentifier("preview.restore")
                Spacer()
                Button(role: .destructive) {
                    deleting = true
                } label: {
                    Label("action.deleteForGood", systemImage: "trash.slash")
                }
                .disabled(readOnly)
                .accessibilityIdentifier("preview.delete")
            } else {
                RatingMenu { stars in
                    if let target { Task { await model.rate(target, stars: stars) } }
                }
                .disabled(readOnly)
                .accessibilityIdentifier("preview.rate")
                Spacer()
                Button {
                    if let target { Task { sharing = await model.shareFiles(target) } }
                } label: {
                    Label("action.share", systemImage: "square.and.arrow.up")
                }
                .accessibilityIdentifier("preview.share")
                Spacer()
                Button(role: .destructive) {
                    if let target { Task { await model.trash(target) } }
                } label: {
                    Label("action.trash", systemImage: "trash")
                }
                .disabled(readOnly)
                .accessibilityIdentifier("preview.trash")
            }
        }
        .labelStyle(.iconOnly)
        .font(.title3)
        .disabled(cell == nil)
        .padding(.horizontal, 28)
        .padding(.top, 12)
        .padding(.bottom, 28)
        .background(.ultraThinMaterial)
        .transition(.move(edge: .bottom).combined(with: .opacity))
    }
}

/// One position of the preview: the original at screen size, its thumbnail until then, or the
/// thumbnail with the file type for what the phone cannot draw.
private struct PreviewPage: View {
    let model: BrowseModel
    let index: Int
    let pixels: Int
    @Binding var zoomed: Bool
    let tapped: () -> Void
    @State private var image: UIImage? = nil
    @State private var failed = false

    var body: some View {
        let cell = model.cell(at: index)
        ZStack {
            if let image {
                ZoomingImage(image: image, zoomed: $zoomed, tapped: tapped)
                    // New identity resets zoom @State when the decoded original changes.
                    .id("\(cell?.id ?? "")#\(cell?.thumbVersion ?? 0)#\(pixels)")
            } else {
                Group {
                    if let cell {
                        VStack(spacing: 12) {
                            ItemThumbnail(cell: cell, cache: model.open.thumbnails, pixels: 512)
                                .frame(maxWidth: 320, maxHeight: 320)
                            if failed {
                                Text(verbatim: cell.ext.uppercased())
                                    .font(.footnote.weight(.semibold))
                                    .foregroundStyle(.secondary)
                            } else {
                                ProgressView()
                            }
                        }
                    } else {
                        ProgressView()
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .contentShape(Rectangle())
                .onTapGesture(perform: tapped)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .task(id: cell.map { "\($0.id)#\($0.thumbVersion)#\(pixels)" } ?? "nil#\(pixels)") {
            guard let cell, pixels > 0 else { return }
            failed = false
            image = await model.open.originals.image(for: cell.id, version: cell.thumbVersion, pixels: pixels)
            failed = image == nil
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(cell.map { "preview.page.\($0.name)" } ?? "preview.page")
    }
}

/// The page that fills the screen on iOS 18 and later, from the scroll offset itself (one page is
/// one container width), next to `scrollPosition`, which does not always follow a quick swipe.
private struct VisiblePage: ViewModifier {
    @Binding var position: Int?

    func body(content: Content) -> some View {
        if #available(iOS 18.0, *) {
            content.onScrollGeometryChange(for: Int.self) { geometry in
                let width = max(geometry.containerSize.width, 1)
                return max(Int((geometry.contentOffset.x / width).rounded()), 0)
            } action: { _, page in
                if page != position {
                    position = page
                }
            }
        } else {
            content
        }
    }
}
