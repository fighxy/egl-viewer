//! Every v1 format through `render`, on files made by `fixtures`.

use std::fs;

use image::ImageFormat;

use crate::fixtures::{self, every_format};
use crate::{Kind, MediaError, THUMBNAIL_SIZE, kind, render, render_bytes};

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

#[test]
fn every_v1_format_gives_dimensions_a_thumbnail_and_a_palette() {
    let dir = tempfile::tempdir().unwrap();
    for (ext, bytes) in every_format(1) {
        let path = write(dir.path(), &format!("sample.{ext}"), &bytes);
        let rendered = render(&path, ext).unwrap_or_else(|error| panic!("{ext}: {error}"));
        assert_eq!((rendered.width, rendered.height), (96, 64), "{ext}");
        assert!(is_webp(&rendered.thumbnail.webp), "{ext}");
        assert!(!rendered.palette.is_empty(), "{ext}");
    }
}

#[test]
fn bytes_in_memory_render_like_the_file() {
    let dir = tempfile::tempdir().unwrap();
    for (ext, bytes) in every_format(2) {
        let path = write(dir.path(), &format!("sample.{ext}"), &bytes);
        let from_file = render(&path, ext).unwrap();
        let from_bytes = render_bytes(&bytes, ext).unwrap_or_else(|error| panic!("{ext}: {error}"));
        assert_eq!(
            (from_bytes.width, from_bytes.height),
            (from_file.width, from_file.height)
        );
        assert_eq!(from_bytes.thumbnail.webp, from_file.thumbnail.webp, "{ext}");
        assert_eq!(from_bytes.palette, from_file.palette, "{ext}");
        assert_eq!(from_bytes.exif, from_file.exif, "{ext}");
    }
    let exif = fixtures::jpeg_with_exif(40, 20, 6);
    assert!(render_bytes(&exif, "jpg").unwrap().exif.is_some());
    assert!(matches!(
        render_bytes(b"x", "zip"),
        Err(MediaError::Unsupported)
    ));
}

#[test]
fn large_images_are_scaled_to_512_and_small_ones_are_not_enlarged() {
    let dir = tempfile::tempdir().unwrap();
    let big = write(
        dir.path(),
        "big.png",
        &fixtures::raster(ImageFormat::Png, 2000, 1000, 2),
    );
    let rendered = render(&big, "png").unwrap();
    assert_eq!((rendered.width, rendered.height), (2000, 1000));
    assert_eq!(
        (rendered.thumbnail.width, rendered.thumbnail.height),
        (THUMBNAIL_SIZE, 256)
    );
    let decoded = image::load_from_memory(&rendered.thumbnail.webp).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (512, 256));

    let small = write(
        dir.path(),
        "small.png",
        &fixtures::raster(ImageFormat::Png, 40, 30, 3),
    );
    let rendered = render(&small, "png").unwrap();
    assert_eq!(
        (rendered.thumbnail.width, rendered.thumbnail.height),
        (40, 30)
    );
}

#[test]
fn svg_renders_at_thumbnail_size_and_keeps_its_own_size() {
    let dir = tempfile::tempdir().unwrap();
    let icon = write(dir.path(), "icon.svg", &fixtures::svg(24, 12, 4));
    let rendered = render(&icon, "svg").unwrap();
    assert_eq!((rendered.width, rendered.height), (24, 12));
    assert_eq!(
        (rendered.thumbnail.width, rendered.thumbnail.height),
        (512, 256)
    );
    assert_eq!(rendered.exif, None);
}

#[test]
fn exif_orientation_is_applied_and_camera_data_read() {
    let dir = tempfile::tempdir().unwrap();
    let photo = write(
        dir.path(),
        "photo.jpg",
        &fixtures::jpeg_with_exif(120, 80, 6),
    );
    let rendered = render(&photo, "jpg").unwrap();
    assert_eq!(
        (rendered.width, rendered.height),
        (80, 120),
        "rotated to portrait"
    );
    assert_eq!(
        rendered.exif,
        Some(serde_json::json!({
            "camera": "Canon EOS R5",
            "isoSpeed": "400",
            "aperture": "2.8",
            "focalLength": "35",
            "shutter": "1/250",
            "timestamp": "2026:10:28 09:30:00",
        }))
    );
}

#[test]
fn other_formats_have_no_preview_and_broken_files_fail_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    for ext in ["avif", "mp4", "pdf", "txt", ""] {
        assert_eq!(kind(ext), Kind::Other, "{ext}");
    }
    let clip = write(dir.path(), "clip.mp4", b"not decoded");
    assert!(matches!(render(&clip, "mp4"), Err(MediaError::Unsupported)));

    let broken = write(dir.path(), "broken.jpg", b"\xFF\xD8\xFF garbage");
    assert!(matches!(render(&broken, "jpg"), Err(MediaError::Decode(_))));
    let empty = write(dir.path(), "empty.png", b"");
    assert!(render(&empty, "png").is_err());
    let bad_svg = write(dir.path(), "bad.svg", b"<svg");
    assert!(matches!(
        render(&bad_svg, "svg"),
        Err(MediaError::Decode(_))
    ));
    let bad_psd = write(dir.path(), "bad.psd", b"8BPS\x00\x01");
    assert!(matches!(
        render(&bad_psd, "psd"),
        Err(MediaError::Decode(_))
    ));
}

#[test]
fn pasted_pixels_become_a_png() {
    let png = crate::png_from_rgba(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128]).unwrap();
    let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
    assert_eq!(decoded.as_raw(), &[255, 0, 0, 255, 0, 0, 255, 128]);
    assert!(crate::png_from_rgba(2, 2, vec![0; 4]).is_err());
}

#[test]
fn a_thumbnail_becomes_a_small_png_to_drag() {
    let rendered =
        crate::render_bytes(&fixtures::raster(ImageFormat::Png, 640, 320, 3), "png").unwrap();
    let png = crate::drag_image(&rendered.thumbnail.webp, 96).unwrap();
    let decoded = image::load_from_memory_with_format(&png, ImageFormat::Png).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (96, 48));
    assert!(crate::drag_image(b"not an image", 96).is_err());
}

#[test]
fn extensions_are_matched_without_case() {
    assert_eq!(kind("JPG"), Kind::Raster);
    assert_eq!(kind("Tiff"), Kind::Raster);
    assert_eq!(kind("SVG"), Kind::Svg);
    assert_eq!(kind("PSD"), Kind::Psd);
}
