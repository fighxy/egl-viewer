-- Format version 6: the Eagle ids that an export into a running Eagle created, apart from
-- `eagle_id`, which names where an imported object came from. One column for both let an export
-- overwrite an import's origin (a resumed import then duplicated folders) and let an import of
-- the exported Eagle library collide with the export's ids.
DROP TABLE IF EXISTS eagle_export;
CREATE TABLE eagle_export (
  kind      TEXT NOT NULL CHECK (kind IN ('item', 'folder', 'tag_group', 'smart_folder')),
  native_id TEXT NOT NULL,
  eagle_id  TEXT NOT NULL,
  PRIMARY KEY (kind, native_id)
) STRICT, WITHOUT ROWID;

-- An item's eagle_id that no import recorded was written by an export.
INSERT INTO eagle_export (kind, native_id, eagle_id)
  SELECT 'item', id, eagle_id FROM item
  WHERE eagle_id IS NOT NULL AND eagle_id NOT IN (SELECT eagle_id FROM eagle_import);
UPDATE item SET eagle_id = NULL
  WHERE eagle_id IS NOT NULL AND eagle_id NOT IN (SELECT eagle_id FROM eagle_import);

-- Folders, tag groups and smart folders keep no import record: in a library that never
-- imported from Eagle, every eagle_id came from an export. Elsewhere they stay origins; an
-- export checks every id against the target Eagle before it reuses one.
INSERT INTO eagle_export (kind, native_id, eagle_id)
  SELECT 'folder', id, eagle_id FROM folder
  WHERE eagle_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM eagle_import);
UPDATE folder SET eagle_id = NULL
  WHERE eagle_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM eagle_import);
INSERT INTO eagle_export (kind, native_id, eagle_id)
  SELECT 'tag_group', id, eagle_id FROM tag_group
  WHERE eagle_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM eagle_import);
UPDATE tag_group SET eagle_id = NULL
  WHERE eagle_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM eagle_import);
INSERT INTO eagle_export (kind, native_id, eagle_id)
  SELECT 'smart_folder', id, eagle_id FROM smart_folder
  WHERE eagle_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM eagle_import);
UPDATE smart_folder SET eagle_id = NULL
  WHERE eagle_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM eagle_import);
