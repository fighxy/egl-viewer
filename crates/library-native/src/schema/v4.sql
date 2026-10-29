-- Format version 4 (D-056): a YubiKey wrap names its retired PIV slot and whether that
-- folder opens with a touch or with the PIN. The table was empty, so it is rebuilt.
DROP TABLE folder_piv_wrap;
CREATE TABLE folder_piv_wrap (
  folder_id        TEXT NOT NULL REFERENCES folder(id) ON DELETE CASCADE,
  card_serial      TEXT NOT NULL,
  slot             INTEGER NOT NULL CHECK (slot BETWEEN 130 AND 149),
  policy           TEXT NOT NULL CHECK (policy IN ('touch', 'pin')),
  recipient        TEXT NOT NULL,
  wrapped_identity BLOB NOT NULL,
  created_at       INTEGER NOT NULL,
  PRIMARY KEY (folder_id, card_serial)
) STRICT, WITHOUT ROWID;
