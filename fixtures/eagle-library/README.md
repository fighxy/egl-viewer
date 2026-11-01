# Synthetic Eagle library

A small Eagle `.library` folder made by hand for the import tests of phase 8
(docs/spec.md §9 phase 8, docs/spec.md §10 "Eagle import"). No personal data.

`Synthetic.library/` holds, on purpose:

- nested folders (`Posters` → `2024`), a folder with its own tags, description and colour,
  an empty folder, and a locked folder (`Secret`, `password` in `metadata.json`);
- a smart folder (`Rated`) and a tag group (`Moods`);
- two items with the same original bytes (`EITEMPOSTER1` / `EITEMPOSTER2`): content
  duplicates, which the import reports and keeps as separate items;
- a trashed item (`EITEMTRASH1`, `isDeleted: true`);
- an item without a thumbnail (`EITEMNOTHUMB1`, `noThumbnail: true`, no `_thumbnail.png`);
- an item with a comment (`EITEMLOOSE01`);
- an item whose original file is missing (`EITEMBROKEN1`): imported with a report line.

The original and thumbnail files are placeholder bytes, not real images: the repository
holds no binary fixtures (D-034). Tests that need decodable pixels copy the fixture into a
temporary folder and overwrite the placeholders with generated images (media fixtures).
