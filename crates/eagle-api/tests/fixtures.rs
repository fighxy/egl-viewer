//! Guards for the captured Eagle responses in `fixtures/eagle-api/`.
//!
//! Later phases build their HTTP mock tests on these files, so they must stay well-formed and
//! free of secrets.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/eagle-api")
}

fn fixtures() -> Vec<(String, Value)> {
    let mut files: Vec<_> = fs::read_dir(fixtures_dir())
        .expect("fixtures/eagle-api exists")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = fs::read_to_string(&path).unwrap();
            let json = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
            (name, json)
        })
        .collect()
}

#[test]
fn every_fixture_is_valid_json() {
    let all = fixtures();
    assert!(
        all.len() >= 49,
        "expected the captured fixtures, found {}",
        all.len()
    );
}

#[test]
fn captures_have_a_request_and_a_response() {
    for (name, json) in fixtures().iter().filter(|(name, _)| !name.starts_with('_')) {
        assert!(json.get("request").is_some(), "{name} has no request");
        let response = json
            .get("response")
            .unwrap_or_else(|| panic!("{name} has no response"));
        // Most captures keep the HTTP status next to the body; a few are summaries.
        if let Some(body) = response.get("body") {
            assert!(
                body.get("status").is_some() || body.get("data").is_some(),
                "{name}: unexpected body"
            );
        }
    }
}

#[test]
fn fixtures_contain_no_secrets() {
    for (name, json) in fixtures() {
        let text = json.to_string();
        assert!(!text.contains("\"token\""), "{name} contains a token field");
        check_passwords(&name, &json);
    }
}

/// Folder passwords may only appear empty or redacted.
fn check_passwords(name: &str, value: &Value) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if key == "password" || key == "passwordTips" {
                    let ok = inner
                        .as_str()
                        .is_some_and(|s| s.is_empty() || s == "<redacted>");
                    assert!(ok, "{name}: {key} is not empty or redacted");
                }
                check_passwords(name, inner);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| check_passwords(name, item)),
        _ => {}
    }
}
