//! EXIF in Eagle's `rawMetas` shape (correction 38), from JPEG, TIFF, PNG, WebP and HEIC.

use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Seek};
use std::path::Path;

use exif::{In, Tag, Value};
use libheif_rs::HeifContext;
use serde_json::{Map, Value as Json};

pub(crate) fn exif(path: &Path) -> Option<Json> {
    exif_from(&mut BufReader::new(File::open(path).ok()?))
}

pub(crate) fn exif_bytes(bytes: &[u8]) -> Option<Json> {
    exif_from(&mut Cursor::new(bytes))
}

/// EXIF of a HEIC/HEIF file: the `Exif` metadata item of the primary image (D-063).
pub(crate) fn exif_heic_bytes(bytes: &[u8]) -> Option<Json> {
    let context = HeifContext::read_from_bytes(bytes).ok()?;
    let handle = context.primary_image_handle().ok()?;
    let mut ids = [0; 4];
    if handle.metadata_block_ids(&mut ids, b"Exif") == 0 {
        return None;
    }
    let block = handle.metadata(ids[0]).ok()?;
    exif_from(&mut Cursor::new(tiff_in_heif_exif(&block)?))
}

/// The HEIF `Exif` item starts with a big-endian offset, counted from the byte after it,
/// to the TIFF header (ISO/IEC 23008-12 §6.2). The offset may point past an `Exif\0\0`
/// prefix, so it is followed rather than the first bytes checked.
fn tiff_in_heif_exif(block: &[u8]) -> Option<&[u8]> {
    let (prefix, rest) = block.split_at_checked(4)?;
    let offset = u32::from_be_bytes(prefix.try_into().ok()?) as usize;
    let tiff = rest.get(offset..)?;
    let magic = tiff.first_chunk::<2>()?;
    (magic == b"MM" || magic == b"II").then_some(tiff)
}

fn exif_from<R: BufRead + Seek>(reader: &mut R) -> Option<Json> {
    let exif = exif::Reader::new().read_from_container(reader).ok()?;
    let field = |tag: Tag| exif.get_field(tag, In::PRIMARY);
    let text = |tag: Tag| {
        field(tag).and_then(|field| match &field.value {
            Value::Ascii(parts) => parts
                .first()
                .map(|bytes| String::from_utf8_lossy(bytes).trim().to_owned())
                .filter(|text| !text.is_empty()),
            _ => None,
        })
    };
    let rational = |tag: Tag| {
        field(tag).and_then(|field| match &field.value {
            Value::Rational(values) => values.first().map(|value| value.to_f64()),
            _ => None,
        })
    };

    let mut meta = Map::new();
    let make = text(Tag::Make);
    let model = text(Tag::Model);
    let camera = match (make, model) {
        (Some(make), Some(model)) if model.starts_with(&make) => Some(model),
        (Some(make), Some(model)) => Some(format!("{make} {model}")),
        (make, model) => model.or(make),
    };
    if let Some(camera) = camera {
        meta.insert("camera".into(), camera.into());
    }
    if let Some(iso) = field(Tag::PhotographicSensitivity).and_then(|field| field.value.get_uint(0))
    {
        meta.insert("isoSpeed".into(), iso.to_string().into());
    }
    if let Some(aperture) = rational(Tag::FNumber) {
        meta.insert("aperture".into(), trimmed(aperture).into());
    }
    if let Some(focal) = rational(Tag::FocalLength) {
        meta.insert("focalLength".into(), trimmed(focal).into());
    }
    if let Some(shutter) = field(Tag::ExposureTime).and_then(|field| match &field.value {
        Value::Rational(values) => values.first().map(|value| {
            if value.num > 0 && value.num < value.denom && value.denom % value.num == 0 {
                format!("1/{}", value.denom / value.num)
            } else {
                trimmed(value.to_f64())
            }
        }),
        _ => None,
    }) {
        meta.insert("shutter".into(), shutter.into());
    }
    if let Some(taken) = text(Tag::DateTimeOriginal) {
        meta.insert("timestamp".into(), taken.into());
    }
    (!meta.is_empty()).then_some(Json::Object(meta))
}

/// `2.8`, `35`, `0.5`: at most two decimals, no trailing zeros.
fn trimmed(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `Exif` item of a HEIF file: a four-byte offset to the TIFF header, then the TIFF.
    /// Built in code from the same block the JPEG fixture uses.
    #[test]
    fn heif_exif_block_offset_points_at_the_tiff_header() {
        let tiff = &crate::fixtures::exif_block(1)[6..];
        let mut block = Vec::with_capacity(4 + tiff.len());
        block.extend_from_slice(&0_u32.to_be_bytes());
        block.extend_from_slice(tiff);
        let parsed = exif_from(&mut Cursor::new(tiff_in_heif_exif(&block).unwrap())).unwrap();
        assert_eq!(parsed["camera"], "Canon EOS R5");
        assert_eq!(parsed["isoSpeed"], "400");
    }

    #[test]
    fn heif_exif_block_without_tiff_magic_is_rejected() {
        assert!(tiff_in_heif_exif(b"\0\0\0\0garbage").is_none());
        assert!(tiff_in_heif_exif(b"MM").is_none());
    }
}
