//! EXIF in Eagle's `rawMetas` shape (correction 38), from JPEG, TIFF, PNG and WebP.

use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Seek};
use std::path::Path;

use exif::{In, Tag, Value};
use serde_json::{Map, Value as Json};

pub(crate) fn exif(path: &Path) -> Option<Json> {
    exif_from(&mut BufReader::new(File::open(path).ok()?))
}

pub(crate) fn exif_bytes(bytes: &[u8]) -> Option<Json> {
    exif_from(&mut Cursor::new(bytes))
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
