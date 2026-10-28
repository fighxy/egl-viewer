-- Format version 1 of the native library. Kept identical to the schema block of
-- docs/library-format.md §4; a test compares them.

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
  thumb_version    INTEGER NOT NULL DEFAULT 0,      -- 0 not made yet, -1 no preview possible, then 1, 2, …
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
