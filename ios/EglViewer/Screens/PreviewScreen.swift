import SwiftUI

struct PreviewScreen: View {
    @EnvironmentObject private var bridge: LibraryBridge
    @Binding var path: NavigationPath
    let itemId: String

    @State private var showEdit = false
    @State private var rotation: Double = 0
    @State private var scale: CGFloat = 1

    private var indexLabel: String {
        guard let index = bridge.items.firstIndex(where: { $0.id == itemId }) else {
            return item?.name ?? itemId
        }
        let total = max(bridge.total, bridge.items.count)
        return "\(index + 1) / \(total)"
    }

    private var item: ItemCellDTO? {
        bridge.items.first { $0.id == itemId }
    }

    var body: some View {
        ZStack {
            EglTokens.previewStage.ignoresSafeArea()

            VStack(spacing: 0) {
                HStack {
                    ghostButton(systemImage: "xmark") {
                        path.removeLast()
                    }
                    Text("\(item?.name ?? "Item") · \(indexLabel)")
                        .font(EglTokens.chromeFont)
                        .foregroundStyle(.white.opacity(0.7))
                        .lineLimit(1)
                        .frame(maxWidth: .infinity)
                    ghostButton(systemImage: "info.circle") {
                        showEdit = true
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 4)

                Spacer()

                Rectangle()
                    .fill(Color.white.opacity(0.12))
                    .aspectRatio(itemAspect, contentMode: .fit)
                    .overlay {
                        Text((item?.ext ?? "img").uppercased())
                            .font(.system(size: 28, weight: .semibold))
                            .foregroundStyle(.white.opacity(0.55))
                    }
                    .scaleEffect(scale)
                    .rotationEffect(.degrees(rotation))
                    .gesture(
                        MagnificationGesture().onChanged { value in
                            scale = min(max(value, 1), 4)
                        }
                    )
                    .onTapGesture(count: 2) {
                        withAnimation(.easeInOut(duration: 0.2)) {
                            scale = scale > 1 ? 1 : 2
                        }
                    }

                Spacer()

                HStack(spacing: 10) {
                    ghostTool("chevron.left") { step(-1) }
                    ghostTool("minus.magnifyingglass") {
                        withAnimation { scale = max(scale - 0.25, 1) }
                    }
                    ghostTool("plus.magnifyingglass") {
                        withAnimation { scale = min(scale + 0.25, 4) }
                    }
                    ghostTool("rotate.right") {
                        withAnimation(.easeInOut(duration: 0.2)) {
                            rotation += 90
                        }
                    }
                    ghostTool("chevron.right") { step(1) }
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 28)
            }
        }
        .navigationBarHidden(true)
        .sheet(isPresented: $showEdit) {
            EditSheet(itemId: itemId)
                .environmentObject(bridge)
                .presentationDetents([.medium, .large])
        }
        .gesture(
            DragGesture(minimumDistance: 40).onEnded { value in
                if value.translation.height < -80 {
                    showEdit = true
                }
            }
        )
        .onChange(of: itemId) { _, _ in
            scale = 1
            rotation = 0
        }
    }

    private var itemAspect: CGFloat {
        let width = CGFloat(item?.width ?? 3)
        let height = CGFloat(item?.height ?? 2)
        return width / max(height, 1)
    }

    private func ghostButton(systemImage: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .foregroundStyle(.white)
                .frame(width: 36, height: 36)
                .background(Color.white.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 10))
        }
        .buttonStyle(.plain)
    }

    private func ghostTool(_ systemImage: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 18, weight: .medium))
                .foregroundStyle(.white)
                .frame(width: 44, height: 44)
                .background(Color.white.opacity(0.1))
                .clipShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
    }

    private func step(_ delta: Int) {
        guard let index = bridge.items.firstIndex(where: { $0.id == itemId }) else { return }
        let next = index + delta
        guard bridge.items.indices.contains(next) else { return }
        path.removeLast()
        path.append(AppRoute.preview(itemId: bridge.items[next].id))
        scale = 1
        rotation = 0
    }
}
