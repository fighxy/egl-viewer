//! Small files of every v1 format, made on the fly so the repository holds no binary fixtures
//! (D-034). Each `seed` gives different pixels, so files do not collide as duplicates.

use std::io::Cursor;

use base64::Engine;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

/// An opaque gradient shifted by the seed: different seeds below 65 536 give different pixels.
pub fn pattern(width: u32, height: u32, seed: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        let r = ((x * 255) / width.max(1)).wrapping_add(seed) as u8;
        let g = ((y * 255) / height.max(1)).wrapping_add(seed >> 8) as u8;
        let b = seed.wrapping_mul(97).wrapping_add((x ^ y) & 7) as u8;
        Rgba([r, g, b, 255])
    })
}

fn encode(image: DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, format)
        .expect("encode a fixture");
    bytes.into_inner()
}

pub fn jpeg(width: u32, height: u32, seed: u32) -> Vec<u8> {
    let rgb = DynamicImage::ImageRgba8(pattern(width, height, seed)).into_rgb8();
    encode(DynamicImage::ImageRgb8(rgb), ImageFormat::Jpeg)
}

pub fn raster(format: ImageFormat, width: u32, height: u32, seed: u32) -> Vec<u8> {
    encode(
        DynamicImage::ImageRgba8(pattern(width, height, seed)),
        format,
    )
}

pub fn svg(width: u32, height: u32, seed: u32) -> Vec<u8> {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <rect width="{width}" height="{height}" fill="#{:06x}"/>
  <circle cx="{}" cy="{}" r="{}" fill="#ffffff"/>
</svg>"##,
        (seed.wrapping_mul(2_654_435_761)) & 0xFF_FFFF,
        width / 2,
        height / 2,
        width.min(height) / 3,
    )
    .into_bytes()
}

/// An RGB PSD with no layers and a raw composite image: what "Maximize compatibility" stores.
pub fn psd(width: u32, height: u32, seed: u32) -> Vec<u8> {
    let image = pattern(width, height, seed);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"8BPS");
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&[0; 6]);
    bytes.extend_from_slice(&3_u16.to_be_bytes()); // channels
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&8_u16.to_be_bytes()); // bits per channel
    bytes.extend_from_slice(&3_u16.to_be_bytes()); // RGB
    bytes.extend_from_slice(&0_u32.to_be_bytes()); // colour mode data
    bytes.extend_from_slice(&0_u32.to_be_bytes()); // image resources
    bytes.extend_from_slice(&0_u32.to_be_bytes()); // layers and masks
    bytes.extend_from_slice(&0_u16.to_be_bytes()); // raw image data
    for channel in 0..3 {
        bytes.extend(image.pixels().map(|pixel| pixel.0[channel]));
    }
    bytes
}

/// A JPEG with an EXIF block: camera, ISO 400, f/2.8, 1/250 s, 35 mm, a capture time, and the
/// given orientation (6 means "rotate 90° clockwise to display").
pub fn jpeg_with_exif(width: u32, height: u32, orientation: u16) -> Vec<u8> {
    let plain = jpeg(width, height, 7);
    let exif = exif_block(orientation);
    let mut bytes = Vec::with_capacity(plain.len() + exif.len() + 4);
    bytes.extend_from_slice(&plain[..2]); // SOI
    bytes.extend_from_slice(&[0xFF, 0xE1]);
    bytes.extend_from_slice(&u16::try_from(exif.len() + 2).unwrap().to_be_bytes());
    bytes.extend_from_slice(&exif);
    bytes.extend_from_slice(&plain[2..]);
    bytes
}

/// `Exif\0\0` and a big-endian TIFF structure with IFD0 and an EXIF IFD.
pub(crate) fn exif_block(orientation: u16) -> Vec<u8> {
    const ASCII: u16 = 2;
    const SHORT: u16 = 3;
    const LONG: u16 = 4;
    const RATIONAL: u16 = 5;
    let make = b"Canon\0";
    let model = b"Canon EOS R5\0";
    let taken = b"2026:10:28 09:30:00\0";

    // Layout: header (8) | IFD0 (2 + 4*12 + 4) | EXIF IFD (2 + 5*12 + 4) | data
    let ifd0 = 8_u32;
    let exif_ifd = ifd0 + 2 + 4 * 12 + 4;
    let mut data_at = exif_ifd + 2 + 5 * 12 + 4;
    let mut data = Vec::new();
    let mut put = |bytes: &[u8], data_at: &mut u32| {
        let offset = *data_at;
        data.extend_from_slice(bytes);
        *data_at += bytes.len() as u32;
        offset
    };
    let make_at = put(make, &mut data_at);
    let model_at = put(model, &mut data_at);
    let taken_at = put(taken, &mut data_at);
    let rational = |num: u32, den: u32| [num.to_be_bytes(), den.to_be_bytes()].concat();
    let fnumber_at = put(&rational(28, 10), &mut data_at);
    let exposure_at = put(&rational(1, 250), &mut data_at);
    let focal_at = put(&rational(35, 1), &mut data_at);

    let entry = |tag: u16, kind: u16, count: u32, value: [u8; 4]| {
        [
            &tag.to_be_bytes()[..],
            &kind.to_be_bytes(),
            &count.to_be_bytes(),
            &value,
        ]
        .concat()
    };
    let short = |value: u16| {
        let [a, b] = value.to_be_bytes();
        [a, b, 0, 0]
    };
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"MM\0\x2a");
    tiff.extend_from_slice(&ifd0.to_be_bytes());
    tiff.extend_from_slice(&4_u16.to_be_bytes());
    tiff.extend(entry(
        0x010F,
        ASCII,
        make.len() as u32,
        make_at.to_be_bytes(),
    ));
    tiff.extend(entry(
        0x0110,
        ASCII,
        model.len() as u32,
        model_at.to_be_bytes(),
    ));
    tiff.extend(entry(0x0112, SHORT, 1, short(orientation)));
    tiff.extend(entry(0x8769, LONG, 1, exif_ifd.to_be_bytes()));
    tiff.extend_from_slice(&0_u32.to_be_bytes());
    tiff.extend_from_slice(&5_u16.to_be_bytes());
    tiff.extend(entry(0x829A, RATIONAL, 1, exposure_at.to_be_bytes()));
    tiff.extend(entry(0x829D, RATIONAL, 1, fnumber_at.to_be_bytes()));
    tiff.extend(entry(0x8827, SHORT, 1, short(400)));
    tiff.extend(entry(
        0x9003,
        ASCII,
        taken.len() as u32,
        taken_at.to_be_bytes(),
    ));
    tiff.extend(entry(0x920A, RATIONAL, 1, focal_at.to_be_bytes()));
    tiff.extend_from_slice(&0_u32.to_be_bytes());
    tiff.extend(data);
    [&b"Exif\0\0"[..], &tiff].concat()
}

/// A 96x64 HEIC with an EXIF block (camera Canon EOS R5), base64-encoded because pure Rust
/// has no HEIC encoder to make one in code (D-034, exception D-063); the pixels do not vary
/// with the seed, so this file stays out of [`every_format`] and the duplicate tests.
pub fn heic() -> Vec<u8> {
    // The base64 file is wrapped at 76 columns; the decoder rejects line breaks.
    let encoded: String = include_str!("../fixtures/sample.heic.b64")
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("decode the HEIC fixture")
}

/// One file of every v1 format, as `(extension, bytes)`.
pub fn every_format(seed: u32) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("jpg", jpeg(96, 64, seed)),
        ("png", raster(ImageFormat::Png, 96, 64, seed)),
        ("gif", raster(ImageFormat::Gif, 96, 64, seed)),
        ("webp", raster(ImageFormat::WebP, 96, 64, seed)),
        ("tif", raster(ImageFormat::Tiff, 96, 64, seed)),
        ("bmp", raster(ImageFormat::Bmp, 96, 64, seed)),
        ("svg", svg(96, 64, seed)),
        ("psd", psd(96, 64, seed)),
    ]
}
