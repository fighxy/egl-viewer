//! Justified grid packing — Rust port of `ui/src/features/browse/justified.ts` (D-067 / Phase 1).
//! Wired into the live wrap grid in a follow-up; kept tested here so the algorithm is ready.

#![allow(dead_code)]

pub const GAP: f32 = 4.0;
pub const PADDING: f32 = 8.0;
pub const NAME_HEIGHT: f32 = 20.0;

#[derive(Debug, Clone, Copy)]
pub struct CellDimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct JustifiedCell {
    pub index: usize,
    pub width: f32,
    pub height: f32,
    pub left: f32,
}

#[derive(Debug, Clone)]
pub struct JustifiedRow {
    pub start_index: usize,
    pub cells: Vec<JustifiedCell>,
    pub thumb_height: f32,
    pub content_height: f32,
    pub stride: f32,
    pub top: f32,
}

#[derive(Debug, Clone)]
pub struct JustifiedLayout {
    pub rows: Vec<JustifiedRow>,
    pub total_height: f32,
}

pub fn aspect_of(width: u32, height: u32) -> f32 {
    if width > 0 && height > 0 {
        width as f32 / height as f32
    } else {
        1.0
    }
}

pub fn pack_justified(
    total: usize,
    container_width: f32,
    target_height: f32,
    show_names: bool,
    size_at: impl Fn(usize) -> CellDimensions,
) -> JustifiedLayout {
    let available = (container_width - 2.0 * PADDING).max(1.0);
    let target = target_height.max(1.0);
    let caption = if show_names { NAME_HEIGHT } else { 0.0 };
    let mut rows = Vec::new();
    let mut index = 0usize;
    let mut top = 0.0f32;

    while index < total {
        let mut aspects: Vec<(usize, f32)> = Vec::new();
        let mut sum_aspect = 0.0f32;
        while index < total {
            let size = size_at(index);
            let aspect = aspect_of(size.width, size.height);
            let gaps = aspects.len() as f32 * GAP;
            let width_if_added = (sum_aspect + aspect) * target + gaps;
            if !aspects.is_empty() && width_if_added > available {
                break;
            }
            aspects.push((index, aspect));
            sum_aspect += aspect;
            index += 1;
        }
        if aspects.is_empty() {
            break;
        }
        let gaps = (aspects.len().saturating_sub(1)) as f32 * GAP;
        let row_height = if sum_aspect > 0.0 {
            ((available - gaps) / sum_aspect).max(1.0)
        } else {
            target
        };
        let start_index = aspects[0].0;
        let mut left = PADDING;
        let count = aspects.len();
        let mut cells = Vec::with_capacity(count);
        for (i, (item_index, aspect)) in aspects.into_iter().enumerate() {
            let width = aspect * row_height;
            cells.push(JustifiedCell {
                index: item_index,
                width,
                height: row_height,
                left,
            });
            left += width + if i + 1 < count { GAP } else { 0.0 };
        }
        let content_height = row_height + caption;
        let stride = content_height + GAP;
        rows.push(JustifiedRow {
            start_index,
            cells,
            thumb_height: row_height,
            content_height,
            stride,
            top,
        });
        top += stride;
    }

    JustifiedLayout {
        total_height: if rows.is_empty() {
            0.0
        } else {
            top - GAP + PADDING
        },
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_square_items_into_rows() {
        let layout = pack_justified(5, 400.0, 100.0, false, |_| CellDimensions {
            width: 100,
            height: 100,
        });
        assert!(!layout.rows.is_empty());
        assert_eq!(
            layout.rows.iter().map(|r| r.cells.len()).sum::<usize>(),
            5
        );
    }
}
