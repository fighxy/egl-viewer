//! The C header and the exported functions agree (D-092): every `#[unsafe(no_mangle)]`
//! function is declared in `include/egl_library.h`, and the header declares nothing else.
//! The shells' copies of the header are kept identical by `scripts/check-mobile-shell.sh`.

use std::collections::BTreeSet;
use std::path::Path;

fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Function names declared in the header: `egl_x(` at the start of a declaration.
fn declared() -> BTreeSet<String> {
    let header = std::fs::read_to_string(crate_dir().join("include/egl_library.h")).unwrap();
    let mut names = BTreeSet::new();
    for line in header.lines() {
        let line = line.trim_start();
        if line.starts_with("typedef") || line.starts_with('*') || line.starts_with("/*") {
            continue;
        }
        if let Some(at) = line.find("egl_") {
            let rest = &line[at..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            if rest[end..].starts_with('(') {
                names.insert(rest[..end].to_owned());
            }
        }
    }
    names
}

/// Function names exported by the sources: the `fn egl_x` after `#[unsafe(no_mangle)]`.
fn exported() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(crate_dir().join("src"))
        .unwrap()
        .flatten()
    {
        let text = std::fs::read_to_string(entry.path()).unwrap();
        let mut marked = false;
        for line in text.lines() {
            let line = line.trim();
            if line == "#[unsafe(no_mangle)]" {
                marked = true;
                continue;
            }
            if marked && let Some(at) = line.find("fn egl_") {
                let rest = &line[at + 3..];
                let end = rest.find('(').unwrap_or(rest.len());
                names.insert(rest[..end].to_owned());
                marked = false;
            }
        }
    }
    names
}

#[test]
fn the_header_declares_exactly_the_exported_functions() {
    let declared = declared();
    let exported = exported();
    assert!(exported.len() > 60, "{exported:?}");
    let missing: Vec<_> = exported.difference(&declared).collect();
    let extra: Vec<_> = declared.difference(&exported).collect();
    assert!(
        missing.is_empty(),
        "exported but not in the header: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "in the header but not exported: {extra:?}"
    );
}

#[test]
fn the_status_codes_match_the_header() {
    let header = std::fs::read_to_string(crate_dir().join("include/egl_library.h")).unwrap();
    for (name, value) in [
        ("EGL_OK", crate::EglStatus::Ok),
        ("EGL_ERR_INVALID", crate::EglStatus::Invalid),
        ("EGL_ERR_NOT_FOUND", crate::EglStatus::NotFound),
        ("EGL_ERR_READ_ONLY", crate::EglStatus::ReadOnly),
        ("EGL_ERR_NOT_A_LIBRARY", crate::EglStatus::NotALibrary),
        ("EGL_ERR_STORAGE", crate::EglStatus::Storage),
        ("EGL_ERR_UNSUPPORTED", crate::EglStatus::Unsupported),
        ("EGL_ERR_OTHER", crate::EglStatus::Other),
        ("EGL_ERR_WRONG_PASSWORD", crate::EglStatus::WrongPassword),
        ("EGL_ERR_LOCKED", crate::EglStatus::Locked),
    ] {
        let line = format!("{name} = {}", value as i32);
        assert!(header.contains(&line), "{line}");
    }
}
