import CoreGraphics
import Foundation

/// Justified rows for the library grid (D-067 / D-122): equal height per row, cell widths from
/// aspect ratios so each row fills the container. Missing sizes are treated as square.

enum JustifiedPacking {
    static let padding: CGFloat = 4
    static let gap: CGFloat = 4
    /// Caption under a thumb when names are shown.
    static let nameHeight: CGFloat = 16

    struct Dimensions: Equatable {
        var width: Int
        var height: Int

        static let unknown = Dimensions(width: 0, height: 0)
    }

    struct Cell: Equatable {
        var index: Int
        var width: CGFloat
        var height: CGFloat
        var left: CGFloat
    }

    struct Row: Equatable {
        var startIndex: Int
        var cells: [Cell]
        var thumbHeight: CGFloat
        var contentHeight: CGFloat
        var stride: CGFloat
        var top: CGFloat
    }

    struct Layout: Equatable {
        var rows: [Row]
        /// Content height excluding outer padding; last row has no trailing gap.
        var totalHeight: CGFloat
        var maxColumns: Int
        /// Index → row index.
        var rowOf: [Int]
    }

    /// Aspect ratio from stored pixels; missing or zero → 1 (square).
    static func aspectOf(width: Int, height: Int) -> CGFloat {
        if width > 0, height > 0 {
            return CGFloat(width) / CGFloat(height)
        }
        return 1
    }

    /**
     Packs `total` items into justified rows. `sizeAt` returns stored width/height (zeros =
     unknown). `targetHeight` is the thumbnail size control (row height before stretch).
     */
    static func pack(
        total: Int,
        containerWidth: CGFloat,
        targetHeight: CGFloat,
        showNames: Bool,
        sizeAt: (Int) -> Dimensions
    ) -> Layout {
        let available = max(1, containerWidth - 2 * padding)
        let target = max(1, targetHeight)
        let caption: CGFloat = showNames ? nameHeight : 0
        var rows: [Row] = []
        var rowOf = Array(repeating: 0, count: total)
        var index = 0
        var top: CGFloat = 0
        var maxColumns = 1

        while index < total {
            var aspects: [(index: Int, aspect: CGFloat)] = []
            var sumAspect: CGFloat = 0
            while index < total {
                let size = sizeAt(index)
                let aspect = aspectOf(width: size.width, height: size.height)
                let gaps = CGFloat(aspects.count) * gap
                let widthIfAdded = (sumAspect + aspect) * target + gaps
                if !aspects.isEmpty, widthIfAdded > available { break }
                aspects.append((index, aspect))
                sumAspect += aspect
                index += 1
            }
            guard !aspects.isEmpty else { break }

            let gaps = CGFloat(max(0, aspects.count - 1)) * gap
            let isLast = index >= total
            let naturalWidth = sumAspect * target + gaps
            // Last row stays near the target height unless it nearly fills the width.
            var thumbHeight =
                isLast && naturalWidth / available < 0.7
                ? target
                : max(1, (available - gaps) / sumAspect)
            // Keep the size control meaningful: clamp to 0.5×–1.5× target.
            thumbHeight = min(target * 1.5, max(target * 0.5, thumbHeight))

            let contentHeight = thumbHeight + caption
            var cells: [Cell] = []
            var left: CGFloat = 0
            var used: CGFloat = 0
            for i in aspects.indices {
                let entry = aspects[i]
                let width: CGFloat
                if i == aspects.count - 1, !(isLast && naturalWidth / available < 0.7) {
                    width = max(1, available - gaps - used)
                } else {
                    width = max(1, entry.aspect * thumbHeight)
                }
                if i < aspects.count - 1 { used += width }
                cells.append(Cell(index: entry.index, width: width, height: thumbHeight, left: left))
                rowOf[entry.index] = rows.count
                left += width + gap
            }

            guard let first = cells.first else { continue }
            maxColumns = max(maxColumns, cells.count)
            rows.append(
                Row(
                    startIndex: first.index,
                    cells: cells,
                    thumbHeight: thumbHeight,
                    contentHeight: contentHeight,
                    stride: contentHeight + gap,
                    top: top
                )
            )
            top += contentHeight + gap
        }

        return Layout(
            rows: rows,
            totalHeight: rows.isEmpty ? 0 : top - gap,
            maxColumns: maxColumns,
            rowOf: rowOf
        )
    }
}
