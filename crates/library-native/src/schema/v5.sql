-- Format version 5: the import state of an Eagle library import (docs/spec.md §8.2,
-- docs/architecture.md §15.6). One row per imported item: the Eagle id, its lastModified
-- on the Eagle side (for a later incremental re-import) and when the import stored it.
-- Like version 4, the step is idempotent: a test turns a new library back into version 1
-- (keeping the new tables) and reopens it through every migration.
DROP TABLE IF EXISTS eagle_import;
CREATE TABLE eagle_import (
  eagle_id      TEXT PRIMARY KEY,
  last_modified INTEGER NOT NULL,
  imported_at   INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
