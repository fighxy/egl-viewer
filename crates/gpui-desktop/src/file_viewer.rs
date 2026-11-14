//! Viewing loose photos and videos opened from Explorer, without a library (D-180).
//!
//! `gpui-desktop.exe photo.jpg` (Explorer's "Open", or the app as the default for a type)
//! opens the full-size preview on that file. Left / Right step through the other photos and
//! videos of its folder in Explorer's name order. Nothing is written: no library opens, no
//! library lock is taken, and closing the preview closes the window.
//!
//! In this mode the preview's cells are made up here: a cell's id is the file's path, so the
//! preview's cache keys stay unique and the path is at hand wherever the preview reads the
//! file (in Rust, by path; the bytes never leave the Rust side).

use std::cmp::Ordering;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use library_core::{ItemCell, ItemId};

/// Extensions with a preview in a library that the viewer still leaves alone: `.ts` is far
/// more often TypeScript than an MPEG transport stream, and a folder of code is no gallery.
const NOT_VIEWED: &[&str] = &["ts"];

/// Every lowercase extension the viewer opens: what gets a preview in a library (photos,
/// PSD, HEIC, SVG and video) but [`NOT_VIEWED`]. The installer registers the same list
/// (`ForEachViewerExtension` in `scripts/installer/egl-viewer-gpui.nsi`).
pub fn extensions() -> impl Iterator<Item = &'static str> {
    media::preview_extensions().filter(|ext| !NOT_VIEWED.contains(ext))
}

/// Whether the viewer opens a file with this extension, in any case.
pub fn is_viewable(ext: &str) -> bool {
    let ext = ext.to_ascii_lowercase();
    extensions().any(|known| known == ext)
}

fn ext_of(path: &Path) -> Option<&str> {
    path.extension()?.to_str()
}

fn viewable_path(path: &Path) -> bool {
    ext_of(path).is_some_and(is_viewable)
}

/// What the command line asks the app to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupTarget {
    /// A library folder (`Name.eglib`, or an Eagle `.library` to import from).
    Library(PathBuf),
    /// A photo or video to view on its own.
    File(PathBuf),
}

/// The first argument that is not a flag (`--no-reopen`), if it names a folder or a file the
/// viewer opens. Anything else is ignored, as before.
pub fn startup_target(args: impl IntoIterator<Item = OsString>) -> Option<StartupTarget> {
    let arg = args
        .into_iter()
        .find(|arg| !arg.to_string_lossy().starts_with("--"))?;
    let path = PathBuf::from(arg);
    if path.is_dir() {
        Some(StartupTarget::Library(path))
    } else if path.is_file() && viewable_path(&path) {
        Some(StartupTarget::File(path))
    } else {
        None
    }
}

/// Explorer's order for names: case-insensitive, with runs of digits compared as numbers
/// (`img2` before `img10`). Ties fall back to the exact text, so the order is total.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut left = a.chars().peekable();
    let mut right = b.chars().peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |chars: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut digits = String::new();
                    while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        chars.next();
                    }
                    digits
                };
                let (x, y) = (take(&mut left), take(&mut right));
                let (xt, yt) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let order = xt.len().cmp(&yt.len()).then_with(|| xt.cmp(yt));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                left.next();
                right.next();
            }
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The files the viewer steps through, in Explorer's name order.
#[derive(Debug, Clone, Default)]
pub struct LooseFiles {
    paths: Vec<PathBuf>,
}

impl LooseFiles {
    /// Just the opened file, while its folder is listed.
    pub fn single(file: PathBuf) -> Self {
        Self { paths: vec![file] }
    }

    /// The viewable files among `paths`, sorted, and the position of `opened` in them. The
    /// opened file is always there, even when the listing missed it.
    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>, opened: &Path) -> (Self, usize) {
        let mut paths: Vec<PathBuf> = paths.into_iter().filter(|p| viewable_path(p)).collect();
        if !paths.iter().any(|p| p == opened) {
            paths.push(opened.to_path_buf());
        }
        paths.sort_by(|a, b| natural_cmp(&file_name(a), &file_name(b)));
        paths.dedup();
        let index = paths.iter().position(|p| p == opened).unwrap_or(0);
        (Self { paths }, index)
    }

    /// The photos and videos in the folder of `opened`. Reads the folder: off the UI thread.
    /// An unreadable folder leaves the opened file alone.
    pub fn around(opened: &Path) -> (Self, usize) {
        let listed = opened
            .parent()
            .and_then(|dir| std::fs::read_dir(dir).ok())
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Self::from_paths(listed, opened)
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn path(&self, index: usize) -> Option<&Path> {
        self.paths.get(index).map(PathBuf::as_path)
    }

    pub fn position(&self, path: &Path) -> Option<usize> {
        self.paths.iter().position(|p| p == path)
    }

    /// The preview's cell for the file at `index`; its id is the path.
    pub fn cell(&self, index: usize) -> Option<ItemCell> {
        let path = self.path(index)?;
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Some(ItemCell {
            id: ItemId(path.to_string_lossy().into_owned()),
            name,
            ext: ext_of(path).unwrap_or_default().to_owned(),
            size: std::fs::metadata(path).ok().map(|m| m.len()),
            ..ItemCell::default()
        })
    }
}

/// The path of a cell made by [`LooseFiles::cell`].
pub fn cell_path(cell: &ItemCell) -> PathBuf {
    PathBuf::from(&cell.id.0)
}

/// The window title while viewing `path`.
pub fn window_title(path: &Path) -> String {
    format!("{} — egl-viewer", file_name(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(files: &LooseFiles) -> Vec<String> {
        (0..files.len())
            .map(|i| file_name(files.path(i).unwrap()))
            .collect()
    }

    #[test]
    fn opens_photos_and_videos_in_any_case() {
        for ext in [
            "jpg", "JPEG", "png", "webp", "heic", "psd", "tiff", "svg", "mp4", "MOV",
        ] {
            assert!(is_viewable(ext), "{ext}");
        }
        for ext in ["txt", "eglib", "exe", "", "mp3"] {
            assert!(!is_viewable(ext), "{ext}");
        }
    }

    #[test]
    fn natural_order_matches_explorer() {
        let mut list = vec![
            "img10.jpg",
            "IMG2.jpg",
            "img1.jpg",
            "a.png",
            "B.png",
            "img02.jpg",
        ];
        list.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            list,
            [
                "a.png",
                "B.png",
                "img1.jpg",
                "IMG2.jpg",
                "img02.jpg",
                "img10.jpg"
            ]
        );
        // Long runs of digits do not overflow.
        assert_eq!(
            natural_cmp(
                "x99999999999999999999999.jpg",
                "x100000000000000000000000.jpg"
            ),
            Ordering::Less
        );
        assert_eq!(natural_cmp("same", "same"), Ordering::Equal);
    }

    #[test]
    fn folder_listing_keeps_viewable_files_and_finds_the_opened_one() {
        let dir = PathBuf::from("C:/photos");
        let paths = ["b10.jpg", "notes.txt", "b2.mp4", "a.PNG", "clip.mov"].map(|n| dir.join(n));
        let (files, index) = LooseFiles::from_paths(paths, &dir.join("b2.mp4"));
        assert_eq!(names(&files), ["a.PNG", "b2.mp4", "b10.jpg", "clip.mov"]);
        assert_eq!(index, 1);
    }

    #[test]
    fn the_opened_file_is_kept_when_the_listing_missed_it() {
        let opened = PathBuf::from("C:/photos/new.jpg");
        let (files, index) = LooseFiles::from_paths(Vec::new(), &opened);
        assert_eq!(files.len(), 1);
        assert_eq!(files.path(index), Some(opened.as_path()));
    }

    #[test]
    fn cells_carry_the_path_as_id() {
        let opened = PathBuf::from("C:/photos/Holiday 01.JPG");
        let files = LooseFiles::single(opened.clone());
        let cell = files.cell(0).unwrap();
        assert_eq!(cell.name, "Holiday 01");
        assert_eq!(cell.ext, "JPG");
        assert!(!cell.has_thumbnail);
        assert_eq!(cell_path(&cell), opened);
        assert!(files.cell(1).is_none());
    }

    #[test]
    fn lists_a_real_folder() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["2.jpg", "10.jpg", "readme.md"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub.jpg")).unwrap();
        let (files, index) = LooseFiles::around(&dir.path().join("10.jpg"));
        assert_eq!(names(&files), ["2.jpg", "10.jpg"]);
        assert_eq!(index, 1);
    }

    #[test]
    fn startup_target_tells_files_from_libraries() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("p.webp");
        let text = dir.path().join("t.txt");
        std::fs::write(&photo, b"x").unwrap();
        std::fs::write(&text, b"x").unwrap();
        let args = |list: &[&Path]| {
            list.iter()
                .map(|p| p.as_os_str().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            startup_target(args(&[&photo])),
            Some(StartupTarget::File(photo.clone()))
        );
        assert_eq!(
            startup_target(args(&[dir.path()])),
            Some(StartupTarget::Library(dir.path().to_path_buf()))
        );
        assert_eq!(startup_target(args(&[&text])), None);
        assert_eq!(startup_target(Vec::<OsString>::new()), None);
        let mut flagged = vec![OsString::from("--no-reopen")];
        flagged.extend(args(&[&photo]));
        assert_eq!(startup_target(flagged), Some(StartupTarget::File(photo)));
    }

    #[test]
    fn installer_registers_the_viewer_extensions() {
        let nsi = include_str!("../../../scripts/installer/egl-viewer-gpui.nsi");
        let listed: Vec<&str> = nsi
            .lines()
            .skip_while(|line| !line.starts_with("!macro ForEachViewerExtension"))
            .skip(1)
            .take_while(|line| !line.starts_with("!macroend"))
            .filter_map(|line| {
                line.trim()
                    .strip_prefix("!insertmacro ${MACRO} \"")?
                    .strip_suffix('"')
            })
            .collect();
        let known: Vec<&str> = extensions().collect();
        assert_eq!(listed, known);
        assert!(!is_viewable("ts"));
    }
}
