-- Format version 3: the search index finds any part of a word, without case, as Eagle's search
-- does (D-047). Tags and folder names are matched through their own tables, so the index holds
-- only the item's own texts. Encrypted items have no row.
DROP TABLE item_fts;
CREATE VIRTUAL TABLE item_fts USING fts5(
  name, annotation, url,
  tokenize = 'trigram'
);
INSERT INTO item_fts (rowid, name, annotation, url)
  SELECT seq, name, annotation, url FROM item WHERE encrypted = 0;
