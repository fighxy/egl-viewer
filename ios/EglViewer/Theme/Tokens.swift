import SwiftUI

/// Desktop-aligned light tokens from `ui/src/index.css` / D-028 (ios-ui-concept.md).
enum EglTokens {
    static let background = Color.white
    static let foreground = Color(red: 0.09, green: 0.09, blue: 0.09)
    static let primary = Color(red: 0.15, green: 0.15, blue: 0.15)
    static let primaryForeground = Color(red: 0.98, green: 0.98, blue: 0.98)
    static let muted = Color(red: 0.96, green: 0.96, blue: 0.96)
    static let mutedForeground = Color(red: 0.45, green: 0.45, blue: 0.45)
    static let sidebar = Color(red: 0.98, green: 0.98, blue: 0.98)
    static let sidebarAccent = Color(red: 0.96, green: 0.96, blue: 0.96)
    static let grid = Color(red: 0.94, green: 0.94, blue: 0.94)
    static let border = Color(red: 0.92, green: 0.92, blue: 0.92)
    static let ring = Color(red: 0.64, green: 0.64, blue: 0.64)
    static let previewStage = Color(red: 0.04, green: 0.04, blue: 0.04)

    static let chromeFont: Font = .system(size: 13, weight: .regular)
    static let titleFont: Font = .system(size: 17, weight: .semibold)
    static let heroFont: Font = .system(size: 28, weight: .semibold)
    static let captionFont: Font = .system(size: 11, weight: .regular)
}

struct PlaceChipStyle: ButtonStyle {
    var active: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12, weight: active ? .semibold : .medium))
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(active ? EglTokens.sidebarAccent : Color.clear)
            .foregroundStyle(EglTokens.foreground)
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .opacity(configuration.isPressed ? 0.85 : 1)
    }
}

struct FilterPillStyle: ButtonStyle {
    var active: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12, weight: .regular))
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(active ? EglTokens.primary : EglTokens.background)
            .foregroundStyle(active ? EglTokens.primaryForeground : EglTokens.foreground)
            .overlay(
                RoundedRectangle(cornerRadius: 7)
                    .stroke(active ? EglTokens.primary : EglTokens.border, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 7))
            .opacity(configuration.isPressed ? 0.85 : 1)
    }
}
