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
        return "\(index + 1) / \(bridge.total)"
    }

    private var item: ItemCellDTO? {
        bridge.items.first { $0.id == itemId }
    }

    var body: some View {
        ZStack {
            EglTokens.previewStage.ignoresSafeArea()

            VStack(spacing: 0) {
                HStack {
                    Button {
                        path.removeLast()
                    } label: {
                        Image(systemName: "xmark")
                            .foregroundStyle(.white)
                    }
                    Spacer()
                    Text(item?.name ?? "Item")
                        .font(EglTokens.chromeFont)
                        .foregroundStyle(.white.opacity(0.9))
                    Spacer()
                    Button {
                        showEdit = true
                    } label: {
                        Image(systemName: "info.circle")
                            .foregroundStyle(.white)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)

                Spacer()

                RoundedRectangle(cornerRadius: 2)
                    .fill(Color.white.opacity(0.12))
                    .aspectRatio(itemAspect, contentMode: .fit)
                    .overlay {
                        Text((item?.ext ?? "img").uppercased())
                            .font(.system(size: 28, weight: .semibold))
                            .foregroundStyle(.white.opacity(0.55))
                    }
                    .padding(.horizontal, 20)
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

                HStack(spacing: 28) {
                    Button { step(-1) } label: {
                        Image(systemName: "chevron.left")
                    }
                    Button {
                        withAnimation { scale = max(scale - 0.25, 1) }
                    } label: {
                        Image(systemName: "minus.magnifyingglass")
                    }
                    Button {
                        withAnimation { scale = min(scale + 0.25, 4) }
                    } label: {
                        Image(systemName: "plus.magnifyingglass")
                    }
                    Button {
                        withAnimation(.easeInOut(duration: 0.2)) {
                            rotation += 90
                        }
                    } label: {
                        Image(systemName: "rotate.right")
                    }
                    Button { step(1) } label: {
                        Image(systemName: "chevron.right")
                    }
                }
                .font(.system(size: 18, weight: .medium))
                .foregroundStyle(.white)
                .padding(.bottom, 8)

                Text(indexLabel)
                    .font(EglTokens.chromeFont)
                    .foregroundStyle(.white.opacity(0.7))
                    .padding(.bottom, 20)
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
    }

    private var itemAspect: CGFloat {
        let width = CGFloat(item?.width ?? 3)
        let height = CGFloat(item?.height ?? 2)
        return width / max(height, 1)
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
