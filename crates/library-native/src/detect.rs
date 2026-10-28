//! Recognising libraries on disk without opening them.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::DB_FILE;
use crate::schema::{APPLICATION_ID, Header};

/// `dir` holds a `library.db` whose SQLite header carries egl-viewer's application id.
pub fn is_native_library(dir: &Path) -> bool {
    raw_header(&dir.join(DB_FILE)).is_some_and(|header| header.application_id == APPLICATION_ID)
}

/// The application id and format version from the first 100 bytes of the main file, without
/// SQLite: nothing is created next to the file. `None` when it is not an SQLite file. A version
/// changed in a WAL that was not checkpointed yet shows only through SQLite.
pub(crate) fn raw_header(db: &Path) -> Option<Header> {
    let mut bytes = [0_u8; 100];
    File::open(db)
        .and_then(|mut file| file.read_exact(&mut bytes))
        .ok()?;
    if &bytes[..16] != b"SQLite format 3\0" {
        return None;
    }
    let be = |at: usize| [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]];
    Some(Header {
        application_id: i32::from_be_bytes(be(68)),
        version: u32::from_be_bytes(be(60)),
    })
}

/// `dir` looks like an Eagle `.library` folder: `metadata.json` and `images/`.
pub fn looks_like_eagle_library(dir: &Path) -> bool {
    dir.join("metadata.json").is_file() && dir.join("images").is_dir()
}
