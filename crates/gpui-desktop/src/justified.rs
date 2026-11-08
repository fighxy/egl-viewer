//! Justified grid packing — Rust port of `ui/src/features/browse/justified.ts` (D-067 / Phase 1).

pub const GAP: f32 = 4.0;
pub const PADDING: f32 = 8.0;
pub const NAME_HEIGHT: f32 = 20.0;

#[derive(Debug, Clone, Copy)]
pub struct CellDimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
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
    #[allow(dead_code)]
    pub start_index: usize,
    pub cells: Vec<JustifiedCell>,
    #[allow(dead_code)]
    pub thumb_height: f32,
    pub content_height: f32,
    pub stride: f32,
    pub top: f32,
}

#[derive(Debug, Clone)]
pub struct JustifiedLayout {
    pub rows: Vec<JustifiedRow>,
    #[allow(dead_code)]
    pub total_height: f32,
    #[allow(dead_code)]
    pub max_columns: usize,
    pub row_of: Vec<usize>,
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
    let mut row_of = vec![0usize; total];
    let mut index = 0usize;
    let mut top = 0.0f32;
    let mut max_columns = 1usize;

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
        let is_last = index >= total;
        let natural_width = sum_aspect * target + gaps;
        let mut thumb_height = if is_last && natural_width / available < 0.7 {
            target
        } else {
            ((available - gaps) / sum_aspect).max(1.0)
        };
        thumb_height = thumb_height.clamp(target * 0.5, target * 1.5);

        let start_index = aspects[0].0;
        let count = aspects.len();
        let mut left = 0.0f32;
        let mut used = 0.0f32;
        let mut cells = Vec::with_capacity(count);
        for (i, (item_index, aspect)) in aspects.into_iter().enumerate() {
            let width = if i + 1 == count && !(is_last && natural_width / available < 0.7) {
                (available - gaps - used).max(1.0)
            } else {
                (aspect * thumb_height).max(1.0)
            };
            if i + 1 < count {
                used += width;
            }
            cells.push(JustifiedCell {
                index: item_index,
                width,
                height: thumb_height,
                left,
            });
            if item_index < row_of.len() {
                row_of[item_index] = rows.len();
            }
            left += width + if i + 1 < count { GAP } else { 0.0 };
        }
        max_columns = max_columns.max(count);
        let content_height = thumb_height + caption;
        let stride = content_height + GAP;
        rows.push(JustifiedRow {
            start_index,
            cells,
            thumb_height,
            content_height,
            stride,
            top,
        });
        top += stride;
    }

    JustifiedLayout {
        total_height: if rows.is_empty() { 0.0 } else { top - GAP },
        rows,
        max_columns,
        row_of,
    }
}

pub fn cell_rect(layout: &JustifiedLayout, index: usize) -> Option<Rect> {
    let row_index = *layout.row_of.get(index)?;
    let row = layout.rows.get(row_index)?;
    let cell = row.cells.iter().find(|c| c.index == index)?;
    let left = PADDING + cell.left;
    let top = PADDING + row.top;
    Some(Rect {
        left,
        top,
        right: left + cell.width,
        bottom: top + row.content_height,
    })
}

pub fn indices_in_rect(layout: &JustifiedLayout, total: usize, rect: Rect) -> Vec<usize> {
    let mut found = Vec::new();
    for row in &layout.rows {
        let row_top = PADDING + row.top;
        let row_bottom = row_top + row.content_height;
        if row_bottom < rect.top || row_top > rect.bottom {
            continue;
        }
        for cell in &row.cells {
            if cell.index >= total {
                break;
            }
            let left = PADDING + cell.left;
            let right = left + cell.width;
            if right < rect.left || left > rect.right {
                continue;
            }
            found.push(cell.index);
        }
    }
    found
}

fn nearest_in_row(row: &JustifiedRow, from_x: f32) -> usize {
    let Some(first) = row.cells.first() else {
        return 0;
    };
    let mut best = first.index;
    let mut best_dist = f32::INFINITY;
    for cell in &row.cells {
        let centre = cell.left + cell.width / 2.0;
        let dist = (centre - from_x).abs();
        if dist < best_dist {
            best_dist = dist;
            best = cell.index;
        }
    }
    best
}

/// Keyboard move on a justified grid (`Arrow*` / Page / Home / End).
pub fn move_focus_justified(
    key: &str,
    focus: Option<usize>,
    layout: &JustifiedLayout,
    page_rows: usize,
) -> Option<usize> {
    let total = layout.row_of.len();
    if total == 0 {
        return None;
    }
    let from = focus.unwrap_or(0);
    let row_index_of = |index: usize| layout.row_of.get(index).copied().unwrap_or(0);
    let x_of = |index: usize| -> f32 {
        let row = &layout.rows[row_index_of(index)];
        row.cells
            .iter()
            .find(|c| c.index == index)
            .map(|c| c.left + c.width / 2.0)
            .unwrap_or(0.0)
    };

    match key {
        "left" => Some(from.saturating_sub(1)),
        "right" => Some((from + 1).min(total - 1)),
        "home" => Some(0),
        "end" => Some(total - 1),
        "up" | "pageup" | "down" | "pagedown" => {
            let current_row = row_index_of(from);
            let delta = match key {
                "up" => -1isize,
                "down" => 1,
                "pageup" => -(page_rows.max(1) as isize),
                "pagedown" => page_rows.max(1) as isize,
                _ => 0,
            };
            let target_row = (current_row as isize + delta).clamp(0, layout.rows.len() as isize - 1)
                as usize;
            if target_row == current_row && (key == "up" || key == "pageup") {
                return Some(0);
            }
            if target_row == current_row && (key == "down" || key == "pagedown") {
                return Some(total - 1);
            }
            Some(nearest_in_row(&layout.rows[target_row], x_of(from)))
        }
        _ => None,
    }
}

/// List / wrap-grid keyboard move when justified packing is not used.
pub fn move_focus_list(
    key: &str,
    focus: Option<usize>,
    total: usize,
    columns: usize,
    page_rows: usize,
) -> Option<usize> {
    if total == 0 {
        return None;
    }
    let from = focus.unwrap_or(0);
    let cols = columns.max(1);
    match key {
        "left" => Some(from.saturating_sub(1)),
        "right" => Some((from + 1).min(total - 1)),
        "up" => Some(from.saturating_sub(cols)),
        "down" => Some((from + cols).min(total - 1)),
        "home" => Some(0),
        "end" => Some(total - 1),
        "pageup" => Some(from.saturating_sub(cols * page_rows.max(1))),
        "pagedown" => Some((from + cols * page_rows.max(1)).min(total - 1)),
        _ => None,
    }
}

pub fn band_rect(a: (f32, f32), b: (f32, f32)) -> Rect {
    Rect {
        left: a.0.min(b.0),
        top: a.1.min(b.1),
        right: a.0.max(b.0),
        bottom: a.1.max(b.1),
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
        assert_eq!(layout.row_of.len(), 5);
        assert!(layout.max_columns >= 1);
    }

    #[test]
    fn indices_in_rect_finds_overlap() {
        let layout = pack_justified(4, 240.0, 100.0, false, |_| CellDimensions {
            width: 100,
            height: 100,
        });
        let first = cell_rect(&layout, 0).expect("first cell");
        let hits = indices_in_rect(&layout, 4, first);
        assert!(hits.contains(&0));
    }

    #[test]
    fn arrow_moves_focus() {
        let layout = pack_justified(6, 400.0, 80.0, false, |_| CellDimensions {
            width: 100,
            height: 100,
        });
        let next = move_focus_justified("right", Some(0), &layout, 2);
        assert_eq!(next, Some(1));
        let home = move_focus_justified("home", Some(3), &layout, 2);
        assert_eq!(home, Some(0));
    }
}
