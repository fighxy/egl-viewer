import SwiftUI
import UIKit

/// Colours that follow light and dark mode, aligned with the desktop tokens (D-028): neutral
/// greys, one accent. Sizes and fonts come from the system so Dynamic Type works.
enum EglTokens {
    static let background = Color(uiColor: .systemBackground)
    static let groupedBackground = Color(uiColor: .systemGroupedBackground)
    static let foreground = Color(uiColor: .label)
    static let mutedForeground = Color(uiColor: .secondaryLabel)
    static let border = Color(uiColor: .separator)
    static let grid = Color(uiColor: .secondarySystemBackground)
    /// The preview stage stays dark in both modes, as on the desktop.
    static let previewStage = Color(red: 0.04, green: 0.04, blue: 0.04)
    static let accent = Color(uiColor: UIColor { traits in
        traits.userInterfaceStyle == .dark
            ? UIColor(white: 0.92, alpha: 1)
            : UIColor(white: 0.15, alpha: 1)
    })
}
