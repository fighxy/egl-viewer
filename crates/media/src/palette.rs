//! Up to 8 colours with their share of the image, largest first, in Eagle's palette shape:
//! k-means on a 64 × 64 sample (docs/architecture.md §15.4).

use image::RgbaImage;
use library_core::Palette;

const SAMPLE: u32 = 64;
const COLOURS: usize = 8;
const ROUNDS: usize = 12;
/// Clusters closer than this (RGB distance) are merged.
const MERGE: f32 = 18.0;
/// Colours below this share (percent) are dropped, unless nothing would be left.
const MIN_SHARE: f64 = 0.5;

/// Deterministic: the same pixels always give the same palette. Transparent pixels are ignored.
pub fn palette(pixels: &RgbaImage) -> Vec<Palette> {
    let samples = sample(pixels);
    if samples.is_empty() {
        return Vec::new();
    }
    let mut centres = initial_centres(&samples);
    let mut assignment = vec![0_usize; samples.len()];
    for _ in 0..ROUNDS {
        for (slot, pixel) in assignment.iter_mut().zip(&samples) {
            *slot = nearest(&centres, pixel);
        }
        let mut sums = vec![[0.0_f32; 3]; centres.len()];
        let mut counts = vec![0_usize; centres.len()];
        for (&cluster, pixel) in assignment.iter().zip(&samples) {
            for channel in 0..3 {
                sums[cluster][channel] += pixel[channel];
            }
            counts[cluster] += 1;
        }
        for (centre, (sum, &count)) in centres.iter_mut().zip(sums.iter().zip(&counts)) {
            if count > 0 {
                *centre = sum.map(|channel| channel / count as f32);
            }
        }
    }

    let mut clusters: Vec<([f32; 3], usize)> = Vec::new();
    let mut counts = vec![0_usize; centres.len()];
    for &cluster in &assignment {
        counts[cluster] += 1;
    }
    for (centre, count) in centres.into_iter().zip(counts) {
        if count == 0 {
            continue;
        }
        match clusters
            .iter_mut()
            .find(|(other, _)| distance(other, &centre) < MERGE)
        {
            Some((other, other_count)) => {
                let total = (*other_count + count) as f32;
                for channel in 0..3 {
                    other[channel] = (other[channel] * *other_count as f32
                        + centre[channel] * count as f32)
                        / total;
                }
                *other_count += count;
            }
            None => clusters.push((centre, count)),
        }
    }
    clusters.sort_by_key(|cluster| std::cmp::Reverse(cluster.1));
    let total = samples.len() as f64;
    let mut colours: Vec<Palette> = clusters
        .into_iter()
        .map(|(centre, count)| Palette {
            color: centre.map(|channel| channel.round().clamp(0.0, 255.0) as u8),
            ratio: (count as f64 / total * 10_000.0).round() / 100.0,
        })
        .collect();
    let keep = colours
        .iter()
        .filter(|colour| colour.ratio >= MIN_SHARE)
        .count()
        .max(1);
    colours.truncate(keep);
    colours
}

/// Every pixel of a grid of at most 64 × 64 points, opaque enough to count.
fn sample(pixels: &RgbaImage) -> Vec<[f32; 3]> {
    let (width, height) = pixels.dimensions();
    let columns = width.min(SAMPLE);
    let rows = height.min(SAMPLE);
    let mut samples = Vec::with_capacity((columns * rows) as usize);
    for row in 0..rows {
        for column in 0..columns {
            let x = (u64::from(column) * u64::from(width) / u64::from(columns)) as u32;
            let y = (u64::from(row) * u64::from(height) / u64::from(rows)) as u32;
            let [r, g, b, a] = pixels.get_pixel(x, y).0;
            if a >= 128 {
                samples.push([f32::from(r), f32::from(g), f32::from(b)]);
            }
        }
    }
    samples
}

/// Evenly spaced picks along the brightness order: deterministic, and spread over the image's
/// range of tones.
fn initial_centres(samples: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| luma(a).total_cmp(&luma(b)));
    let count = COLOURS.min(sorted.len());
    (0..count)
        .map(|index| sorted[(index * 2 + 1) * sorted.len() / (count * 2)])
        .collect()
}

fn luma(pixel: &[f32; 3]) -> f32 {
    0.299 * pixel[0] + 0.587 * pixel[1] + 0.114 * pixel[2]
}

fn nearest(centres: &[[f32; 3]], pixel: &[f32; 3]) -> usize {
    centres
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| distance(a, pixel).total_cmp(&distance(b, pixel)))
        .map_or(0, |(index, _)| index)
}

fn distance(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;

    #[test]
    fn two_colours_in_three_to_one() {
        let image = RgbaImage::from_fn(40, 40, |x, _| {
            if x < 30 {
                Rgba([200, 30, 30, 255])
            } else {
                Rgba([20, 40, 220, 255])
            }
        });
        let colours = palette(&image);
        assert_eq!(colours.len(), 2);
        assert_eq!(colours[0].color, [200, 30, 30]);
        assert!((colours[0].ratio - 75.0).abs() < 0.01);
        assert_eq!(colours[1].color, [20, 40, 220]);
        assert!((colours[1].ratio - 25.0).abs() < 0.01);
    }

    #[test]
    fn transparent_pixels_do_not_count() {
        let image = RgbaImage::from_fn(10, 10, |x, _| {
            if x < 5 {
                Rgba([0, 255, 0, 0])
            } else {
                Rgba([10, 10, 10, 255])
            }
        });
        let colours = palette(&image);
        assert_eq!(colours.len(), 1);
        assert_eq!(colours[0].color, [10, 10, 10]);
        assert!(palette(&RgbaImage::from_pixel(4, 4, Rgba([1, 2, 3, 0]))).is_empty());
    }

    #[test]
    fn a_gradient_gives_at_most_eight_colours_largest_first() {
        let image = RgbaImage::from_fn(256, 64, |x, y| Rgba([x as u8, (y * 4) as u8, 128, 255]));
        let colours = palette(&image);
        assert!((1..=8).contains(&colours.len()));
        assert!(
            colours
                .windows(2)
                .all(|pair| pair[0].ratio >= pair[1].ratio)
        );
        let sum: f64 = colours.iter().map(|colour| colour.ratio).sum();
        assert!(sum > 95.0 && sum <= 100.01, "{sum}");
        assert_eq!(palette(&image), colours, "deterministic");
    }
}
