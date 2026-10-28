//! Rules for library names, which become Windows folder names (docs/library-format.md §9).

use library_core::SourceError;

const MAX_CHARS: usize = 100;
const FORBIDDEN: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Returns the trimmed name, or the reason it cannot be a folder name. Never fixes a name.
pub fn validate_library_name(name: &str) -> Result<String, SourceError> {
    let name = name.trim();
    let invalid = |reason: &str| Err(SourceError::Invalid(reason.to_owned()));
    if name.is_empty() {
        return invalid("Enter a name.");
    }
    if name.chars().count() > MAX_CHARS {
        return invalid("Use at most 100 characters.");
    }
    if let Some(c) = name
        .chars()
        .find(|c| FORBIDDEN.contains(c) || c.is_control())
    {
        let shown = if c.is_control() {
            "control characters".to_owned()
        } else {
            format!("\"{c}\"")
        };
        return Err(SourceError::Invalid(format!(
            "A library name cannot contain {shown}."
        )));
    }
    if name.ends_with('.') {
        return invalid("A library name cannot end with a dot.");
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Err(SourceError::Invalid(format!(
            "\"{stem}\" is reserved by Windows."
        )));
    }
    Ok(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_names_and_trims_them() {
        assert_eq!(
            validate_library_name("  My Library ").unwrap(),
            "My Library"
        );
        assert_eq!(
            validate_library_name("Референсы 2026").unwrap(),
            "Референсы 2026"
        );
        assert_eq!(validate_library_name("Console").unwrap(), "Console");
        assert_eq!(validate_library_name("v1.2 refs").unwrap(), "v1.2 refs");
    }

    #[test]
    fn refuses_names_windows_cannot_use() {
        for name in [
            "", "   ", "a<b", "a>b", "a:b", "a\"b", "a/b", "a\\b", "a|b", "a?b", "a*b", "a\tb",
            "name.", "CON", "con", "Nul.txt", "COM1", "lpt9", "AUX ",
        ] {
            assert!(
                matches!(validate_library_name(name), Err(SourceError::Invalid(_))),
                "{name:?} was accepted"
            );
        }
        assert!(validate_library_name(&"x".repeat(101)).is_err());
        assert!(validate_library_name(&"x".repeat(100)).is_ok());
    }
}
