//! The colour filter (D-176): items by the colours of their palette.
//!
//! Every item keeps a palette of up to 8 colours with the share of the image each covers (the
//! same shape as Eagle's). The filter adds up the shares of the palette colours that qualify and
//! keeps the item when they cover enough of it. "Paper" is the preset for photographed and
//! scanned documents, whiteboards and receipts: most of the image light and nearly colourless.

use serde::{Deserialize, Serialize};

use crate::folder::color_distance;
use crate::model::Palette;

/// Weighted RGB distance (scaled to plain RGB units) under which two colours are similar.
const SIMILAR_DISTANCE: f64 = 45.0;

/// Highest chroma (max − min channel, 0–255) a paper colour may have: paper photographed under
/// warm or cold light is slightly tinted.
const PAPER_CHROMA: u8 = 52;

/// Lowest lightness ((max + min) / 2, 0–255) of a paper colour: shadows on a sheet are grey.
const PAPER_LIGHTNESS: u16 = 120;

/// Default share of the image for a picked colour.
pub const SIMILAR_SHARE: u8 = 15;

/// Default share of the image for paper.
pub const PAPER_SHARE: u8 = 55;

/// What the colour filter keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ColorFilter {
    /// Colours near `rgb` cover at least `min_share` percent of the image.
    Similar { rgb: [u8; 3], min_share: u8 },
    /// Light, nearly colourless colours cover at least `min_share` percent: documents.
    Paper { min_share: u8 },
}

impl ColorFilter {
    /// A picked colour with the default share.
    pub fn similar(rgb: [u8; 3]) -> Self {
        Self::Similar {
            rgb,
            min_share: SIMILAR_SHARE,
        }
    }

    /// The documents preset.
    pub fn paper() -> Self {
        Self::Paper {
            min_share: PAPER_SHARE,
        }
    }

    /// Whether an item with this palette passes. An item without a palette never does.
    pub fn matches(&self, palette: &[Palette]) -> bool {
        let (min_share, qualifies): (u8, &dyn Fn([u8; 3]) -> bool) = match self {
            Self::Similar { rgb, min_share } => (*min_share, &|color| similar(*rgb, color)),
            Self::Paper { min_share } => (*min_share, &is_paper),
        };
        let share: f64 = palette
            .iter()
            .filter(|entry| qualifies(entry.color))
            .map(|entry| entry.ratio)
            .sum();
        !palette.is_empty() && share >= f64::from(min_share)
    }
}

/// Whether two colours look alike.
pub fn similar(a: [u8; 3], b: [u8; 3]) -> bool {
    let pack = |c: [u8; 3]| u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]);
    // The redmean distance is about three times plain RGB distance.
    (color_distance(pack(a), pack(b)) as f64).sqrt() / 3.0 <= SIMILAR_DISTANCE
}

/// Light and nearly colourless: white, off-white and light grey paper.
pub fn is_paper(color: [u8; 3]) -> bool {
    let max = color.iter().copied().max().unwrap_or(0);
    let min = color.iter().copied().min().unwrap_or(0);
    max - min <= PAPER_CHROMA && (u16::from(max) + u16::from(min)) / 2 >= PAPER_LIGHTNESS
}

/// `#rrggbb` of a colour, for Eagle's `color similar` rule.
pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// A colour from `#rrggbb` or `#rgb` (any case), as Eagle's rules write them.
pub fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let digits = text.trim().strip_prefix('#')?;
    let channel = |s: &str| u8::from_str_radix(s, 16).ok();
    match digits.len() {
        6 => Some([
            channel(&digits[0..2])?,
            channel(&digits[2..4])?,
            channel(&digits[4..6])?,
        ]),
        3 => {
            let short = |i: usize| channel(&digits[i..=i]).map(|v| v * 17);
            Some([short(0)?, short(1)?, short(2)?])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette(entries: &[([u8; 3], f64)]) -> Vec<Palette> {
        entries
            .iter()
            .map(|&(color, ratio)| Palette { color, ratio })
            .collect()
    }

    #[test]
    fn a_photographed_page_is_paper_and_a_cat_is_not() {
        // A sheet under warm light with grey shadows and a little dark text.
        let page = palette(&[
            ([231, 224, 208], 46.0),
            ([186, 182, 174], 22.0),
            ([142, 140, 136], 14.0),
            ([40, 40, 44], 9.0),
            ([120, 96, 70], 9.0),
        ]);
        assert!(ColorFilter::paper().matches(&page));
        let cat = palette(&[
            ([92, 88, 84], 40.0),
            ([168, 130, 96], 30.0),
            ([230, 228, 224], 12.0),
            ([30, 30, 30], 18.0),
        ]);
        assert!(!ColorFilter::paper().matches(&cat));
        assert!(!ColorFilter::paper().matches(&[]));
    }

    #[test]
    fn paper_is_light_and_nearly_grey() {
        assert!(is_paper([255, 255, 255]));
        assert!(is_paper([235, 225, 200]));
        assert!(is_paper([150, 150, 150]));
        assert!(!is_paper([90, 90, 90]), "too dark");
        assert!(!is_paper([250, 200, 150]), "too coloured");
        assert!(!is_paper([200, 230, 255]), "a pale blue sky");
    }

    #[test]
    fn a_picked_colour_keeps_items_where_it_covers_enough() {
        let red = [220, 40, 40];
        let poster = palette(&[([200, 30, 35], 20.0), ([20, 20, 20], 80.0)]);
        assert!(ColorFilter::similar(red).matches(&poster));
        let speck = palette(&[([200, 30, 35], 5.0), ([20, 20, 20], 95.0)]);
        assert!(!ColorFilter::similar(red).matches(&speck));
        assert!(similar(red, [180, 30, 30]));
        assert!(!similar(red, [240, 130, 40]), "orange is not red");
        assert!(!similar(red, [236, 72, 153]), "pink is not red");
        assert!(!similar([255, 255, 255], [150, 150, 150]));
    }

    #[test]
    fn colours_read_and_write_as_eagle_writes_them() {
        assert_eq!(parse_hex("#ff0000"), Some([255, 0, 0]));
        assert_eq!(parse_hex("#0F0"), Some([0, 255, 0]));
        assert_eq!(parse_hex("red"), None);
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(hex([1, 171, 255]), "#01abff");
        let filter = ColorFilter::paper();
        let json = serde_json::to_value(filter).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "paper", "minShare": 55 }));
        assert_eq!(serde_json::from_value::<ColorFilter>(json).unwrap(), filter);
    }
}
