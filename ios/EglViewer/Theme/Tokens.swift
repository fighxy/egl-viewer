import SwiftUI

/// Desktop-aligned light tokens from `ui/src/index.css` / D-028 (ios-ui-concept.md).
enum EglTokens {
    static let background = Color.white
    static let foreground = Color(red: 0.09, green: 0.09, blue: 0.09)
    static let muted = Color(red: 0.96, green: 0.96, blue: 0.96)
    static let mutedForeground = Color(red: 0.45, green: 0.45, blue: 0.45)
    static let sidebar = Color(red: 0.98, green: 0.98, blue: 0.98)
    static let grid = Color(red: 0.94, green: 0.94, blue: 0.94)
    static let border = Color(red: 0.90, green: 0.90, blue: 0.90)
    static let previewStage = Color.black

    static let chromeFont: Font = .system(size: 13, weight: .regular)
    static let titleFont: Font = .system(size: 17, weight: .semibold)
    static let heroFont: Font = .system(size: 34, weight: .semibold)
}

struct PlaceChipStyle: ButtonStyle {
    var active: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(EglTokens.chromeFont)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(active ? EglTokens.foreground : Color.clear)
            .foregroundStyle(active ? Color.white : EglTokens.foreground)
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(EglTokens.border, lineWidth: active ? 0 : 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .opacity(configuration.isPressed ? 0.85 : 1)
    }
}
