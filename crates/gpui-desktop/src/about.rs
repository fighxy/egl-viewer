//! Help → About and Third-party licenses (D-133, D-166): the toolkit version and the notices
//! text, laid out as headings and paragraphs that wrap to the dialog.

/// The notices shipped with the app; packaging copies the same file next to the exe, so it
/// names the gpui-kit version literally and a test keeps it equal to the pin.
const LICENSES: &str = include_str!("../assets/THIRD_PARTY_LICENSES.txt");

/// The workspace manifest, which pins gpui-kit to one exact version.
const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");

/// The gpui-kit version the app is built with, read from its exact pin (`gpui-kit = "=x.y.z"`)
/// in the workspace manifest, so About never names a version the app does not use.
pub fn gpui_kit_version() -> &'static str {
    pinned_version(WORKSPACE_MANIFEST, "gpui-kit").unwrap_or("unknown")
}

fn pinned_version<'a>(manifest: &'a str, package: &str) -> Option<&'a str> {
    manifest.lines().find_map(|line| {
        let value = line.trim().strip_prefix(package)?.trim_start();
        let value = value.strip_prefix('=')?.trim();
        let version = value.strip_prefix('"')?.split('"').next()?;
        Some(version.trim_start_matches('='))
    })
}

/// One block of the notices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicenseBlock {
    /// A line underlined with `---`.
    Heading(String),
    /// Consecutive lines up to a blank one, joined so the dialog can wrap them.
    Paragraph(String),
}

/// The notices as blocks. The title (underlined with `===`) is left out: the dialog has its
/// own.
pub fn license_blocks() -> Vec<LicenseBlock> {
    parse_blocks(LICENSES)
}

fn parse_blocks(text: &str) -> Vec<LicenseBlock> {
    let is_rule = |line: &str, mark: char| {
        let line = line.trim();
        line.len() >= 3 && line.chars().all(|c| c == mark)
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |paragraph: &mut Vec<&str>, blocks: &mut Vec<LicenseBlock>| {
        if !paragraph.is_empty() {
            blocks.push(LicenseBlock::Paragraph(paragraph.join(" ")));
            paragraph.clear();
        }
    };
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        let next = lines.get(i + 1).copied().unwrap_or_default();
        if line.is_empty() {
            flush(&mut paragraph, &mut blocks);
            i += 1;
        } else if is_rule(next, '=') {
            flush(&mut paragraph, &mut blocks);
            i += 2;
        } else if is_rule(next, '-') {
            flush(&mut paragraph, &mut blocks);
            blocks.push(LicenseBlock::Heading(line.to_owned()));
            i += 2;
        } else {
            paragraph.push(line);
            i += 1;
        }
    }
    flush(&mut paragraph, &mut blocks);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_the_exact_pin_of_the_manifest() {
        let manifest =
            "[workspace.dependencies]\r\ngpui-kit = \"=0.9.1\"\r\ngpui-kit-assets = \"1\"\r\n";
        assert_eq!(pinned_version(manifest, "gpui-kit"), Some("0.9.1"));
        assert_eq!(pinned_version("serde = \"1\"", "gpui-kit"), None);
        // The real manifest pins it, and the notices name the same version.
        let version = gpui_kit_version();
        assert_ne!(version, "unknown");
        assert!(version.starts_with("0."), "{version}");
        let all: String = license_blocks()
            .into_iter()
            .map(|block| match block {
                LicenseBlock::Heading(text) | LicenseBlock::Paragraph(text) => text,
            })
            .collect();
        assert!(
            all.contains(&format!("={version}")),
            "THIRD_PARTY_LICENSES.txt must name gpui-kit ={version}"
        );
    }

    #[test]
    fn notices_become_headings_and_wrapped_paragraphs() {
        let text = "Title\r\n=====\r\n\r\nFirst line\r\nsecond line.\r\n\r\nlibheif\r\n-------\r\nLGPL.\r\n";
        assert_eq!(
            parse_blocks(text),
            [
                LicenseBlock::Paragraph("First line second line.".into()),
                LicenseBlock::Heading("libheif".into()),
                LicenseBlock::Paragraph("LGPL.".into()),
            ]
        );
    }

    #[test]
    fn the_shipped_notices_have_no_retired_host() {
        assert!(!LICENSES.contains("Tauri"));
        assert!(
            license_blocks()
                .iter()
                .any(|b| matches!(b, LicenseBlock::Heading(h) if h.starts_with("libheif")))
        );
    }
}
