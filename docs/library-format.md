# Native library format, version 1

The on-disk format of a native egl-viewer library (`Name.eglib`): the folder layout, the
`library.db` schema, the lock, opening and closing, and how a file enters the library. It was
written before the phase 1 code (2026-10-28); the code follows it, and a change to the format
changes this document in the same commit. For the on-disk format this document is the reference;
[`architecture.md` §15.3](architecture.md) summarises it. Decisions: D-014, D-016, D-019 to D-024.

Status: format version 1, not implemented yet.

## 1. Folder layout

```
My Library.eglib/
  library.db                    SQLite in WAL mode: the single source of truth for metadata
  library.db-wal, library.db-shm   SQLite's own files while the library is open
  library.lock                  held with an OS file lock while a writer has the library open (§5)
  files/
    .tmp/                       files being copied in; emptied by the writer on open
    7Q/01JB…7Q/IMG_1042.jpg     originals: <shard>/<item id>/<stored file name>
  thumbs/7Q/01JB…7Q.webp        thumbnails (phase 2): regenerable, may be deleted at any time
  backups/library-2026-10-28.db copies of library.db (VACUUM INTO), the last 10 kept
```

- **Shard:** the last two characters of the item id (a ULID in Crockford base32), so at most
  1024 folders.
- **Stored file name:** the original file name, with the part before the extension cut to at
  most 100 UTF-16 units. Paths over 260 characters break Explorer and many other Windows apps.
  `item.name` keeps the full name. A pasted image without a name is stored as
  `Pasted 2026-10-28 10-42-05.png`.
- **Paths** in the database are relative to the library folder, with `/` separators. A closed
  library can be moved or copied as one folder.
- **Recognition.** A folder is a library when it holds `library.db` whose header carries the
  application id and a format version the app knows (§2). The `.eglib` suffix is a convention
  for people and the folder picker (D-016).
- **Local disks only.** WAL needs shared memory on one computer and does not work over a network
  file system, and cloud sync tools copy a live database mid-write. Where libraries may live is
  in [`spec.md` §3](spec.md) and D-021.

## 2. Identification and versions

| Where | Value | Meaning |
|---|---|---|
| `PRAGMA application_id` | `1162300482` (`0x45474C42`, ASCII `EGLB`) | This SQLite file is an egl-viewer library |
| `PRAGMA user_version` | `1` | The format version. There is no other version number |
| `meta.created_with`, `meta.last_written_with` | app version, for example `0.1.0` | Diagnostics only; never used to decide anything |

Checks when a library is opened (the full order is in §7):

1. No `library.db` → not a library. If the folder looks like an Eagle library (`metadata.json`
   and `images/`), the message says so; Eagle import is phase 8.
2. A read-only connection reads the header. Another `application_id` → not a library.
   A `user_version` above the newest one the app knows → refused ("made by a newer egl-viewer"),
   and nothing is written, not even `meta.open_by`.
3. An older `user_version` → migrated forward by the writer, after a backup (§8). A read-only
   instance refuses a library that needs a migration and asks to close the other instance.

**When the version goes up.** With every schema change, and also when the meaning of stored data
changes while the schema stays the same. Locked folders (phase 4) use columns and tables that
exist from version 1, but the first locked folder in a library raises the version: an older app
would otherwise add files to a locked folder as plaintext. A library that never had a locked
folder keeps its version and still opens in older builds
([`age-locked-folders.md`](age-locked-folders.md) §6).

## 3. Connection settings

Set once when the library is created. They cannot be changed later without rewriting the file:

```sql
PRAGMA page_size = 4096;
PRAGMA auto_vacuum = INCREMENTAL;     -- emptying the trash can give space back without a full VACUUM
PRAGMA journal_mode = WAL;            -- stored in the file
PRAGMA application_id = 1162300482;
PRAGMA user_version = 1;
```

The text encoding is UTF-8 (the default). On every writer connection:

```sql
PRAGMA foreign_keys = ON;
PRAGMA synchronous = FULL;            -- a commit survives power loss; adding a file (§6) relies on it
PRAGMA busy_timeout = 5000;
```

A read-only instance opens the database with `mode=ro`. A library found in a sync folder opens
with `mode=ro&immutable=1`, so SQLite creates no `-wal` or `-shm` file there (§5, D-021).

## 4. Schema

Rules:

- Every table is `STRICT`: a value of the wrong type is an error, not silently stored.
- Ids are ULIDs stored as `TEXT` (26 characters), readable by any SQLite tool. Timestamps are
  `INTEGER` milliseconds since the Unix epoch, UTC. Booleans are `INTEGER` 0 or 1.
- `CHECK` constraints only for invariants that never change (the star range, booleans, the hash
  length). Lists that can grow (folder colours, sort keys) are validated in Rust: relaxing a
  `CHECK` later means rebuilding the table.
- Tags use the default `BINARY` collation: `"Poster"` and `"poster"` are two tags.
- **Reserved for locked folders (phase 4):** `item.encrypted`, `item.sealed`, the lock columns of
  `folder`, and the tables `item_key_wrap` and `folder_piv_wrap`. They exist from version 1 and
  stay empty until phase 4; code before phase 4 never writes them, and a test checks that they
  exist and are empty. While an item is encrypted, its name, note, link, EXIF and palette are one
  age message in `item.sealed` (JSON with the five fields), the plaintext columns are empty, and
  the item has no row in `item_fts`.

```sql
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT, WITHOUT ROWID;
-- library_id         ULID, made with the library; a copy of the folder keeps it
-- name               the display name; renaming the folder by hand does not change it
-- created_at         ms
-- created_with       app version that created the library
-- last_written_with  app version of the last writer
-- open_by            JSON {"computer", "pid", "since", "app"} while a writer has the library open (§5)

CREATE TABLE item (
  seq              INTEGER PRIMARY KEY,             -- stable rowid, the key of item_fts; VACUUM keeps it
  id               TEXT NOT NULL UNIQUE,            -- ULID
  name             TEXT NOT NULL,                   -- '' while encrypted
  ext              TEXT NOT NULL,                   -- lowercase, no dot
  file_path        TEXT NOT NULL UNIQUE,            -- files/<shard>/<id>/<stored file name>
  size             INTEGER NOT NULL,                -- bytes
  width            INTEGER,
  height           INTEGER,
  duration         REAL,                            -- seconds
  bpm              REAL,
  exif             TEXT,                            -- JSON: camera, iso, aperture, focal length, shutter, capture time
  star             INTEGER NOT NULL DEFAULT 0 CHECK (star BETWEEN 0 AND 5),
  annotation       TEXT NOT NULL DEFAULT '',
  url              TEXT NOT NULL DEFAULT '',
  added_at         INTEGER NOT NULL,                -- Eagle modificationTime
  file_created_at  INTEGER,                         -- Eagle btime
  file_modified_at INTEGER,                         -- Eagle mtime
  content_hash     BLOB NOT NULL CHECK (length(content_hash) = 32),   -- BLAKE3 of the plaintext original
  palette          TEXT,                            -- JSON in the shape of Eagle palettes
  thumb_version    INTEGER NOT NULL DEFAULT 0,
  deleted_at       INTEGER,                         -- set while in the trash
  updated_at       INTEGER NOT NULL,
  eagle_id         TEXT UNIQUE,
  encrypted        INTEGER NOT NULL DEFAULT 0 CHECK (encrypted IN (0, 1)),   -- reserved, phase 4
  sealed           BLOB                                                      -- reserved, phase 4
) STRICT;

CREATE TABLE folder (
  id               TEXT PRIMARY KEY,
  parent_id        TEXT REFERENCES folder(id) ON DELETE CASCADE,
  name             TEXT NOT NULL,
  description      TEXT NOT NULL DEFAULT '',
  icon_color       TEXT,                            -- red, orange, yellow, green, aqua, blue, purple, pink or NULL
  sort_key         TEXT NOT NULL,                   -- fractional index among siblings
  order_by         TEXT,                            -- sort override
  sort_ascending   INTEGER CHECK (sort_ascending IN (0, 1)),
  cover_item_id    TEXT REFERENCES item(id) ON DELETE SET NULL,
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL,
  eagle_id         TEXT UNIQUE,
  -- reserved, phase 4: locked folders (docs/age-locked-folders.md)
  locked           INTEGER NOT NULL DEFAULT 0 CHECK (locked IN (0, 1)),
  label            TEXT,                            -- the lock's tag caption; plaintext; not an item tag
  label_color      TEXT,                            -- the lock's tag colour; plaintext
  kdf_salt         BLOB,                            -- Argon2id salt; NULL until a password is set
  kdf_params       TEXT,                            -- Argon2id parameters, JSON
  wrapped_identity BLOB,                            -- age identity wrapped by Argon2id(password)
  recovery_wrap    BLOB                             -- the same identity wrapped by the recovery secret
) STRICT;

CREATE TABLE item_folder (
  item_id   TEXT NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  folder_id TEXT NOT NULL REFERENCES folder(id) ON DELETE CASCADE,
  position  REAL,                                   -- manual order inside the folder
  PRIMARY KEY (item_id, folder_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE tag (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE CHECK (name <> '')      -- BINARY collation: case-sensitive
) STRICT;

CREATE TABLE item_tag (
  item_id TEXT NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  tag_id  INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  PRIMARY KEY (item_id, tag_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE folder_tag (
  folder_id TEXT NOT NULL REFERENCES folder(id) ON DELETE CASCADE,
  tag_id    INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  PRIMARY KEY (folder_id, tag_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE tag_group (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  color       TEXT,
  description TEXT NOT NULL DEFAULT '',
  sort_key    TEXT NOT NULL,
  eagle_id    TEXT UNIQUE
) STRICT;

CREATE TABLE tag_group_tag (
  group_id TEXT NOT NULL REFERENCES tag_group(id) ON DELETE CASCADE,
  tag_id   INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  PRIMARY KEY (group_id, tag_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE smart_folder (
  id             TEXT PRIMARY KEY,
  parent_id      TEXT REFERENCES smart_folder(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  conditions     TEXT NOT NULL,                     -- Eagle's rule JSON, unchanged (architecture §16)
  order_by       TEXT,
  sort_ascending INTEGER CHECK (sort_ascending IN (0, 1)),
  sort_key       TEXT NOT NULL,
  eagle_id       TEXT UNIQUE
) STRICT;

CREATE TABLE comment (
  id         TEXT PRIMARY KEY,
  item_id    TEXT NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  annotation TEXT NOT NULL,
  x INTEGER, y INTEGER, w INTEGER, h INTEGER,       -- a rect of the image
  at_seconds REAL,                                  -- or a video time
  updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE change_log (
  seq       INTEGER PRIMARY KEY,
  entity    TEXT NOT NULL,                          -- item, folder, tag, tag_group, smart_folder, meta
  entity_id TEXT NOT NULL,
  op        TEXT NOT NULL,                          -- insert, update, delete
  at        INTEGER NOT NULL
) STRICT;

CREATE TABLE pending_file (                         -- the journal of adding a file (§6)
  path       TEXT PRIMARY KEY,                      -- the target file_path
  created_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- reserved, phase 4: the key of an item in a locked folder, wrapped once per locked folder
CREATE TABLE item_key_wrap (
  item_id     TEXT NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  folder_id   TEXT NOT NULL REFERENCES folder(id),  -- no cascade: a locked folder is decrypted before it is deleted
  wrapped_key BLOB NOT NULL,                        -- the item key, age-encrypted to the folder identity
  PRIMARY KEY (item_id, folder_id)
) STRICT, WITHOUT ROWID;

-- reserved, phase 4: YubiKey PIV wraps of a folder identity, one row per card
CREATE TABLE folder_piv_wrap (
  folder_id        TEXT NOT NULL REFERENCES folder(id) ON DELETE CASCADE,
  card_serial      TEXT NOT NULL,
  recipient        TEXT NOT NULL,                   -- age-plugin-yubikey recipient of slot 9d
  wrapped_identity BLOB NOT NULL,                   -- the folder identity, age-encrypted to that recipient
  created_at       INTEGER NOT NULL,
  PRIMARY KEY (folder_id, card_serial)
) STRICT, WITHOUT ROWID;

CREATE INDEX item_added_at        ON item(added_at);
CREATE INDEX item_content_hash    ON item(content_hash);   -- duplicates; not unique, "add anyway" is allowed
CREATE INDEX item_trash           ON item(deleted_at) WHERE deleted_at IS NOT NULL;
CREATE INDEX item_folder_folder   ON item_folder(folder_id);
CREATE INDEX item_tag_tag         ON item_tag(tag_id);
CREATE INDEX folder_parent        ON folder(parent_id);
CREATE INDEX smart_folder_parent  ON smart_folder(parent_id);
CREATE INDEX comment_item         ON comment(item_id);
CREATE INDEX item_key_wrap_folder ON item_key_wrap(folder_id);

CREATE VIRTUAL TABLE item_fts USING fts5(
  name, annotation, url, tags, folders,
  tokenize = 'unicode61 remove_diacritics 2'
);
-- rowid = item.seq. Written by the app together with the item; encrypted items have no row.
```

Why `item.seq`: a table without an explicit `INTEGER PRIMARY KEY` gets implicit rowids, and
`VACUUM` (also `VACUUM INTO`, used for backups) may renumber them. `item_fts` would then point at
the wrong items after a restore. Other tables refer to items by `id`.

## 5. One writer

- `library.lock` is created with the library and **never deleted**: deleting a lock file lets two
  processes lock two different files.
- **The writer** opens `library.lock` and calls `File::try_lock()` from the Rust standard library
  (`LockFileEx` on Windows, `flock` elsewhere; no `unsafe`). It holds the handle until the library
  closes. The OS releases the lock when the process ends, also after a crash or a kill, so there
  is no heartbeat and no stale-lock timeout.
- **Who holds it.** On Windows the lock is mandatory: other processes cannot read
  `library.lock` while it is held. The holder is therefore in `meta.open_by` (computer, process
  id, since, app version), which the writer sets right after taking the lock and removes on a
  clean close.
- **Lock taken by someone else** (`TryLockError::WouldBlock`): the library opens read-only,
  reason `otherInstance`. The banner names the computer and the time from `meta.open_by` when it
  is there. Any other failure to open or lock the file (read-only medium, no permission): read-only,
  reason `readOnlyMedium`.
- **Unclean close:** the lock is free but `meta.open_by` is set. The writer logs it and runs
  `PRAGMA quick_check`; a failure stops the open with a message that points to the backups. The
  normal open steps (§7) then recover `pending_file` and empty `files/.tmp/`, as on every open.

**A read-only instance**

- writes nothing: no `open_by`, no cleanup of `files/.tmp/` (the writer may be copying there),
  no `pending_file` recovery, no backups, no migrations;
- compares `PRAGMA data_version` when the window regains focus, and reloads the tree and the
  views when another process has written (from phase 3);
- offers **Open for editing**, which retries `try_lock` and, when it succeeds, reopens the library
  as the writer (§7). It is not offered for reason `syncFolder`.

Read-only reasons reach the UI in `LibraryInfo.readOnlyReason`: `otherInstance`, `syncFolder`,
`readOnlyMedium`.

## 6. Adding a file

```
1. copy     source → files/.tmp/<ulid> in one pass, BLAKE3 over the bytes;
            source size before == bytes copied == source size after, otherwise "source changed"
2. flush    File::sync_all() on the temp file
3. dupe?    item with the same content_hash, the trash included → Duplicate { existing, inTrash },
            unless the call says "add anyway"; the temp file is deleted
4. journal  INSERT pending_file(path)                         -- its own transaction, committed
5. rename   create files/<shard>/<id>/, rename the temp file to path; retry for up to 1 s on a
            sharing violation or access denied (antivirus scanners hold new files briefly)
6. commit   INSERT item (+ change_log, item_fts); DELETE pending_file   -- one transaction
```

- An error before step 4 deletes the temp file; nothing is in the database. An error in step 5
  deletes the temp file and the journal row. An error in step 6 rolls the transaction back and
  removes the renamed file and the journal row; if that cleanup fails too, the next open does it.
- **A row never points at a missing file:** the file is in place before its row commits. **No
  file stays without a row:** `pending_file` names it until the row exists. Recovery looks only at
  `pending_file`, never walks `files/`.
- `synchronous = FULL` makes each commit durable, which this order relies on. NTFS journals the
  rename itself; the temp file's data is flushed in step 2.
- The source file is only read; it stays where it was.
- **Phase 1:** `library-native` exposes `add_file(source, duplicates)` with
  `duplicates: Report | AddAnyway`, with tests, no command and no UI. Phase 2 adds batches,
  target folders and tags, the media pipeline, progress and cancel; phase 4 adds "add the existing
  item to the target folder" and encryption for a locked target folder.

## 7. Opening and closing

The writer opens a library in this order:

1. **Location** (spec §3): a network path is refused; a sync folder opens read-only with
   `immutable=1` and stops here.
2. **Header** on a read-only connection (§2). Nothing is written before this passes.
3. **Lock** (§5). Not taken → read-only, stop here.
4. **Writer connection** with the settings of §3. If `meta.open_by` is set, the last session
   ended without a clean close: `PRAGMA quick_check`.
5. Set `meta.open_by` and `meta.last_written_with`.
6. **Migrations** if `user_version` is older: a backup first, then the steps (§8).
7. **Recover `pending_file`:** for each row, if an item has that `file_path`, delete the row;
   otherwise delete the file if it exists, its item folder if empty, and the row.
8. Empty `files/.tmp/`.
9. **Daily backup** when the newest backup is older than a day.
10. Load the folder tree and the tag vocabulary.

**Closing** (another library, Welcome, quitting): finish or cancel running imports after asking,
`PRAGMA wal_checkpoint(TRUNCATE)`, remove `meta.open_by`, close the connection, release the lock
and close its handle.

If the library folder disappears or a write fails with an I/O error (a USB drive pulled out), the
app emits `library-closed` and returns to Welcome with a message.

## 8. Backups and migrations

- Backups are `VACUUM INTO 'backups/library-YYYY-MM-DD.db'`, at most one a day on open, and
  `backups/library-YYYY-MM-DD-vN.db` before a migration from version N. The last 10 are kept.
- `VACUUM INTO` keeps `item.seq`, an explicit `INTEGER PRIMARY KEY`, so a restored backup keeps
  `item_fts` aligned with `item`.
- Migrations run forward only. Each version step is one transaction that also sets
  `user_version`. Every released format version gets a fixture library under
  `fixtures/library/vN/`; a test migrates each older fixture to the newest version.

## 9. Creating a library

- **Input:** a parent folder and a name. The parent defaults to `%USERPROFILE%\egl-viewer` (D-021).
- **Name:** not empty after trimming; at most 100 characters; none of `< > : " / \ | ? *` and no
  control characters; not `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9` or `LPT1`–`LPT9`, also not
  with an extension; no trailing dot or space. An invalid name is refused with the reason; it is
  never fixed silently.
- **Target** `<parent>/<name>.eglib` must not exist.
- **Location** (spec §3): not a network path, not inside a sync folder, not inside another native
  library or an Eagle `.library` folder.
- **Steps:** build everything in `<parent>/.<name>.eglib.creating-<ulid>/` (`files/`,
  `files/.tmp/`, `thumbs/`, `backups/`, `library.lock`, and `library.db` with the settings of §3,
  the schema and `meta`), close every handle, rename the folder to `<name>.eglib`, then open it as
  the writer (§7). On failure the temporary folder is removed, so a half-made library never
  appears under its name. The next create in the same parent removes leftover `.creating-`
  folders.
- `meta.name` is the name as typed.
