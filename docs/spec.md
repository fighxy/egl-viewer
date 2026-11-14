# egl-viewer (Windows first) — Technical Specification

Handoff document for an implementation agent. egl-viewer is a **standalone digital-asset manager**: it keeps images in its own library, with folders, tags, ratings, notes, search, filters and smart folders. It does not need Eagle. **Compatibility with Eagle** (eagle.cool) is a secondary feature: the app imports Eagle libraries, can connect to a running Eagle on the same PC or in the network, and exports into Eagle (section 8).

One codebase: a **Rust core** and a **GPUI Kit desktop host** (Rust). **The current priority is the Windows desktop app, without AI features.** Android and iOS are later targets: the design keeps them possible, but they are not in the current scope. It is not a port of Eagle, not a cloud service, and it never loads Eagle's own UI.

Source of truth for the Eagle parts: Eagle **4.0.0** (build 20260401), Electron 22.3.7, Windows install. Local API modules inside `resources/app.asar`: `app/js/api-server.js` (v1), `app/js/api-server-v2.js` (v2), `app/js/api-v2-playground-config.js` (parameter catalog), `resources/my_modules/json-rest-light`. Public docs that match this build: https://developer.eagle.cool/web-api

> **Companion documents. Read them together with this spec.**
>
> - [`architecture.md`](architecture.md): stack, crates, the native library format, the media pipeline, the query engine, write rules, the Eagle transport, sync and offline, smart folder rule semantics, the `.eaglepack` format, plus 41 numbered corrections found by reverse engineering Eagle 4.0.0 with REA and by live API calls. Facts marked **[live]** there were verified against a running Eagle.
> - [`decisions.md`](decisions.md): decisions taken during development, with dates and reasons.
> - [`library-format.md`](library-format.md): the on-disk format of a native library: the schema, the lock, opening and closing, adding a file.
> - [`fixtures/eagle-api/`](../fixtures/eagle-api/): 49 real request and response captures from Eagle 4.0.0, for HTTP mock tests (`wiremock`).
>
> "arch §6" means section 6 of the architecture document; "correction 7" means row 7 of its section 1; "D-013" is an entry in `decisions.md`. The spec says *what* the app does; the architecture document says *how* it is built. If you find a conflict, the architecture document wins and this spec must be fixed.
>
> History: the first versions of this spec described a client for a running Eagle. On 2026-10-28 the product became a standalone app with Eagle compatibility (D-013). The Eagle protocol content is unchanged and lives in section 8.

---

## 1. Product

egl-viewer keeps a collection of images and design references on the user's disk and makes it easy to browse, organise and find them.

- **A library** is a folder `Name.eglib` (D-016). It holds a SQLite database with all metadata, the original files and regenerable thumbnails (arch §15.3). The user can have several libraries; one window shows one library.
- **Adding files copies them** into the library (D-014). The library is self-contained: it can be moved, backed up or copied to another PC as one folder.
- **Organising:** folders (nested, many-to-many: an item can be in several folders), tags (case-sensitive strings), tag groups, a rating of 0–5 stars, a note, a source link.
- **Finding:** full-text search, filter chips (tags, type, rating, shape), sorting, smart folders (saved rules).
- **Trash:** items go to the trash first; the trash can be restored or emptied.
- **Eagle compatibility (secondary, section 8):** import an Eagle `.library` folder into a native library; connect to a running Eagle and browse or edit its library; export a native library into Eagle.

v1 file formats (D-015): JPEG, PNG, GIF, WebP, SVG, TIFF, BMP, the composite image of PSD files and HEIC/HEIF (D-063) get dimensions, thumbnails and colour palettes. AVIF files are stored with a placeholder until a decoder is chosen (D-035). Any other file can still be added: it is stored and shown with an extension placeholder. Video, audio, PDF, fonts and RAW previews come later.

### Non-goals for v1

- AI features: semantic search, search by image, auto-tagging.
- A cloud service, accounts, sync between devices (later, arch §15.8).
- A browser extension, a plugin runtime, scripting.
- Linked folders (indexing files where they are, without copying): later.
- Reimplementing Eagle, loading Eagle's own UI, Eagle's license flow, talking to `core.eagle.cool`.
- Writing files inside an Eagle `Name.library` folder, directly or through any file sync. Eagle is the only writer of its library (section 8.6).
- Calling Eagle's `POST /api/script/inject` or `GET /api/check`.

---

## 2. Stack

| Concern | Choice |
|---|---|
| Shell | **GPUI Kit** in `crates/gpui-desktop` (D-126…D-133): a native Rust desktop host over `desktop-session`. |
| Targets | **Windows 10/11 now**. Android (minSdk 26, target 37) and iOS are later targets: nothing in the core or the UI may depend on Windows. macOS and Linux stay possible later. |
| Core | Rust workspace (arch §3): `library-core` (no IO: models, query engine, edits, the `LibrarySource` trait), `library-native` (the native library: SQLite, files, thumbnails), `media` (hashing, probing, thumbnails, palettes), `app-store` (settings, recent libraries, secrets); for Eagle: `eagle-folder` (an Eagle `.library` folder, read only) and `eagle-api` (a running Eagle). The desktop host is a thin layer of views over `desktop-session`. |
| Storage | SQLite through `rusqlite` with the bundled SQLite (WAL, FTS5), on a blocking thread pool (arch §15.3). |
| Media | `image` (its JPEG decoder is `zune-jpeg`), `fast_image_resize`, `resvg`, `psd`, `kamadak-exif`, `webp` (libwebp, for lossy thumbnails), `blake3` (arch §15.4, D-033). |
| HTTP | Only for Eagle: `reqwest` with rustls, `tokio`. **Only Rust talks to Eagle.** The UI never calls the Eagle API and never sees the token. |
| JSON | `serde` / `serde_json`. |
| UI | GPUI Kit (Rust). The window frame, the menu bar, the grid and the dialogs are all built in the host crate (D-126…D-141). |
| Images in the UI | Thumbnails and originals are read from the library folder by the host crate; nothing is served through a web view. |
| Settings | Small JSON files written by `app-store` in the app's local data folder: recent libraries, the last query per library, preferences and the Eagle endpoint (D-022). |
| Secrets | Only the Eagle token, in Windows Credential Manager (`keyring` crate). Never in the UI, never in the store file. |
| Files | The system file dialog for pickers; drag-and-drop and clipboard events give file paths to Rust. |
| Background | Imports and thumbnail generation run as tasks owned by the app state. No background service when the window is closed. |
| Native code | None for Windows. Later, for phones: Kotlin and Swift for the Keystore, the local network permission and share targets. |
| Tooling | `rustup` (toolchain pinned in `rust-toolchain.toml`), pinned versions for every crate, rustfmt and clippy for Rust, GitHub Actions on Windows. |

Do not use a web-view shell, React Native, Flutter or a WASM frontend for this plan.

Workspace layout (details in arch §3):

```
crates/
  library-core/    model/, query/ (pipeline, matchers, sorter, lock filter, views), edit/ (ItemEdit, cycle guard), the LibrarySource trait
  library-native/  the native library: schema and migrations, library.lock, load_scope in SQL, FTS5 search, writes, change_log, backups
  media/           hashing, probing, thumbnails, palettes
  app-store/       settings, recent libraries, SecretStore trait
  eagle-folder/    an Eagle .library folder, read only: import
  eagle-api/       a running Eagle: transport (EagleHttp, escape_non_ascii, EagleError, page_all), dto/ (folder passwords dropped here), plan, api_v2
  gpui-desktop/    the desktop host: window, menu bar, views, imports, settings (D-126…D-141)
(later)            Android and iOS shells over library-ffi, plus a small native plugin for phones
```

---

## 3. The library

### Library folder

```
Name.eglib/
  library.db       SQLite in WAL mode: the single source of truth for metadata
  files/           the original files, one folder per item
  thumbs/          thumbnails: regenerable, may be deleted at any time
  backups/         automatic copies of library.db
  library.lock     held while one app instance writes the library
```

The exact layout, the schema and the protocols are in [`library-format.md`](library-format.md) (format version 1).

- A folder is a library when it contains `library.db` with egl-viewer's SQLite application id and a format version the app knows (D-019). The `.eglib` suffix is a convention for people and for the folder picker; the app does not depend on it.
- **One writer.** While one app instance has a library open, a second instance opens it read-only, says where and since when it is open, and offers **Open for editing** once the other one has closed it. Killing the app releases the lock at once; the next open notices the unclean close and checks the database (D-020).
- **Local disks only.** Cloud sync tools (OneDrive, Dropbox, Google Drive, iCloud, Syncthing and similar) copy a live database mid-write and corrupt it, and SQLite's WAL mode does not work over a network share. A new library cannot be created in a sync folder, on a network path, or inside another library or an Eagle `.library` folder. A library found in a sync folder opens read-only, with a message to move the folder out while the app is closed; one on a network path is refused. Detection is best effort (D-021).
- **Default location** for new libraries: `%USERPROFILE%\egl-library`. Not Pictures or Documents: Windows' OneDrive folder backup often moves them into OneDrive, and the app never puts a library into OneDrive on its own. The Create dialog can pick another local folder, and the default itself is a preference the user can change later (D-021).
- **Versioning.** A library made by a newer app version is refused with a clear message, and nothing is written to it. An older library is migrated forward after an automatic backup.
- **Backups.** A copy of `library.db` in `backups/` when the library opens (at most once a day), before every migration, on Back Up Now and before a restore; the last 10 are kept. Library Settings restores one into the open library, after backing up the current state; files are not part of a backup, so Check Library then lists files without an item and items without a file (D-051). Distinct from **Backup…** / **Export…**, which copy originals out of the library (§7.4 item 12, D-064).

### Items

| Field | Meaning |
|---|---|
| `id` | ULID, created by the app. Opaque everywhere else. |
| `name` | Title without extension. At most 10 000 characters (UTF-16 units, the same limit as Eagle). |
| `ext` | Lowercase extension, no dot. |
| `width`, `height` | Pixels, when known. |
| `size` | Bytes. |
| `duration`, `bpm` | Seconds and beats per minute, for audio and video (later formats; kept from Eagle imports). |
| `tags` | Set of strings. A tag's identity **is** the string, case-sensitive: `"Poster"` and `"poster"` are different tags. |
| `folders` | Set of folder ids. Many-to-many. |
| `url` | Source link, often empty. |
| `annotation` | Note, at most 10 000 characters. |
| `star` | Rating 0–5; 0 means unrated. |
| `palette` | Up to 8 colours with ratios, largest first: `[{ "color": [r, g, b], "ratio": 47.5 }, …]`. The first colour is the grid placeholder while a thumbnail loads. |
| `exif` | Camera, ISO, aperture, focal length, shutter, capture time, when the file has them. |
| `addedAt` | Date added (when the file entered the library). |
| `fileCreatedAt`, `fileModifiedAt` | The source file's dates when it was added. Missing values fall back to `addedAt` when sorting. |
| `contentHash` | BLAKE3 of the original; finds duplicates. |
| `deletedAt` | Set while the item is in the trash. |
| `comments` | Notes on a rect of the image or a video time. Imported from Eagle; shown read-only in v1. |

### Folders

- A tree with a name, description, colour (`red`, `orange`, `yellow`, `green`, `aqua`, `blue`, `purple`, `pink` or none), its own order among siblings, an optional sort override and an optional cover item.
- **Membership is direct.** An item is **in** a folder iff its `folders` contains that folder's id. Opening a folder does **not** show items that are only in child folders, unless the "Include subfolders" switch is on: then an item matches if any of its folders is the current folder or a descendant (a union, not an intersection). Being in a child does not make an item a member of the parent.
- **Uncategorized** means `folders` is empty.
- **Folder tags** are labels on the folder, not on its items. When a file is added into a folder, the tags of that folder and of its ancestors are added to the new item (as in Eagle, correction 12). Changing a folder's tags later does not change items already inside.
- **Moving** a folder under itself or under one of its descendants is rejected before anything is written (cycle guard, arch §7.3).
- **Deleting** a folder deletes it and all its subfolders and confirms first with the number of affected items. Items stay in the library: they lose those folder ids, and items left without any folder become Uncategorized. The dialog has an option "Also move items that are only in these folders to the trash".
- **Locked folders** hide their content in every scope until unlocked for the session, and they encrypt that content at rest. A lock covers its folder and every subfolder; a write that changes what a lock covers (adding to or removing from a covered folder, moving a folder in or out, deleting one) needs that lock unlocked and is refused before anything is written otherwise (D-045). (D-018, [`age-locked-folders.md`](age-locked-folders.md)). A library can hold several locked folders. Each one has its own password, its own Argon2id salt and parameters, its own age identity, and its own recovery secret. Unlocking one folder does not unlock the others. An item in two locked folders has its own item key, wrapped separately by each folder's identity. Folder A's password decrypts the item in A and does not unlock folder B. The password and the recovery secret are never stored; each wraps that folder's age identity. Originals in `files/`, thumbnails in `thumbs/`, and the sensitive item columns (name, annotation, url, exif, palette) are age ciphertext. Each locked folder has its own user-editable name and tag (a label and a colour), stored in plaintext on the folder row; they are not encrypted fields, and the tag is not an item tag. The left sidebar has an expandable **Locked folders** section that lists them with that name and tag. Choosing a row prompts for that folder only. At creation the user is shown a recovery secret once: a 128-bit key, the same bits as a word phrase, or an optional plaintext file outside the `.eglib` folder. Recovery unwraps the same identity so a new password can be set without re-encrypting files. The dialog says the lock encrypts the folder on disk. An optional YubiKey 5C is a third wrap of the same identity (`age-plugin-yubikey`, a retired PIV slot 82–95, ECDSA P-256). The user either touches the key or enters its PIN. The PIN replaces the touch. It is not a second step, and it is not app login. Without the key the password and the recovery secret still work. Unlocking one folder does not unlock the others. Several keys can wrap one folder. See [`age-locked-folders.md`](age-locked-folders.md) §9.

### Tags

- The vocabulary is every tag stored on items, plus tags kept in tag groups. Each tag shows its item count (trash excluded). A tag that only items of a locked folder carry stays out until that folder is unlocked.
- Rename a tag everywhere, merge one tag into another, delete a tag from every item (with confirmation that names the tag and the item count). Renaming onto a name that is already a tag merges the two; the dialog says so first (D-046).
- **Tag groups** are named, coloured sets of tag names. Editing or deleting a group never changes item tags.
- The app keeps its own list of recently used tags.

### Trash

Trashed items disappear from every scope except Trash. Trash shows them with the date of deletion. Restore puts an item back with its folders and tags. Delete permanently and Empty trash remove the rows and the files after a confirmation; there is no undo. Empty trash shows its progress in a window and can be cancelled between items; the same window then says how many items went and how much disk was freed, or why it stopped, until Close (D-172); sidebar counts and the Trash menu info refresh as soon as the rows are gone (D-074). Items of a locked folder stay in the trash until the folder is unlocked (D-051). The right-click menu of the Trash row shows how many items the trash holds and their size on disk, and offers Restore All and Empty Trash.

### Smart folders

A smart folder is a saved rule set evaluated against the library. Rules use Eagle's rule language (`match` AND/OR, rules with `property`, `method`, `value`), stored as JSON, so smart folders survive import from and export into Eagle unchanged (arch §16). v1 evaluates a subset locally in `library-core` (section 5); a smart folder that uses a rule outside the subset is shown with a warning and its unsupported rules are listed. Such a rule matches nothing; the editor keeps it unless it is removed.

Smart folders live in their own tree (nested, with their own order).

---

## 4. Adding files and the media pipeline

### Adding

- From a toolbar button (the system file dialog, multi-select, default filter "Images" plus "All files"), by dropping files or folders on the window, and by pasting an image or files copied in Explorer from the clipboard (Edit → Paste, `Ctrl+V`).
- **Import Folder…** (File menu): the user picks one folder. Rust walks it and returns a preview (counts, bytes, a folder tree). Images of the v1 formats are preselected; other files are listed and unselected. A folder can be unchecked, which excludes its children. Nothing is copied until Import. Cancel before confirm leaves the library unchanged. The same add-file pipeline runs after confirm (tmp → hash → rename → row); source files stay where they were. A read-only library refuses the command. A locked target folder encrypts each copied file as it is added. Duplicates follow the same skip / add / attach choice as a normal import (D-061).
- Dropped folders are walked recursively; hidden and system files (`Thumbs.db`, `desktop.ini`) are skipped. A dropped folder becomes a library folder of the same name inside the target (an existing folder of that name there is reused), with its subfolders below it; each file goes into the folder it came from (D-043). Dropped folders are not reviewed: every non-junk file is taken.
- Target: the open folder when a folder is open, otherwise Uncategorized. Optional tags and rating for every file of the batch come with the tags and editing slices.
- Rust receives the paths and reads the files; file bytes never leave the Rust side.

### What happens to one file (arch §7.4, §15.4)

1. Hash with BLAKE3 while copying it to a temporary file inside the library, then flush the copy to disk. A crash leaves only a temp file that the next open removes.
2. **Duplicates:** if an item with the same hash exists (in the trash too, which the report says), the batch asks once what to do: skip duplicates, add them anyway, or cancel the import. When the files go into a folder, a fourth choice adds the existing items to that folder instead.
3. Record the target path in a journal, move the copy to `files/<shard>/<id>/<file name>`, then insert the item and clear the journal entry in one transaction. A row never points at a missing file, and a crash never leaves a file without a row ([`library-format.md`](library-format.md) §6). The stored file keeps the original name, cut to 100 characters before the extension; a pasted image gets a name like `Pasted 2026-10-28 10-42-05.png` (UTC).
4. Probe and generate the thumbnail (512 px on the long edge, lossy WebP, EXIF orientation applied) and the palette in the background. The item is visible in the grid at once, with the extension placeholder, until its thumbnail exists. Previews that a crash or a cancel left undone are made the next time the library opens.
5. If decoding fails, the item stays, with the placeholder and a note in the import report.

Imports run as tasks owned by the app state, so navigating away does not cancel them. Progress reaches the UI as events: a progress bar in the status area, a list of failures, a cancel button (already added items stay).

Library maintenance: "Rebuild thumbnails" regenerates `thumbs/` from the originals; "Check library" finds missing originals and orphan files.

---

## 5. Browsing

### Scopes

All items, a folder (with or without subfolders), Uncategorized, Untagged, a smart folder, Trash.

### Filters

Applied on top of the scope; all of them combine with AND:

- **Tags** with a mode (default Any):

  | Mode | Match |
  |---|---|
  | Any | At least one selected tag is on the item |
  | All | Every selected tag is on the item |
  | Exact | All, and the item has no other tags |
  | None | None of the selected tags is on the item |

  Compare with exact string equality, not lowercase.

- **Extra folders** with the same four modes (default Any).
- **Type:** an extension, or a bucket (`image`, and later `video`, `audio`, `font`, `document`).
- **Rating:** a star value, or "unrated".
- **Shape:** square, landscape, portrait.
- **Search text** (below).

Later, still local: file size, dimensions, dates, camera and EXIF, colour against `palette`, note, link.

```rust
fn matches_tags(item_tags: &[String], selected: &[String], mode: TagMode) -> bool {
    let has = |t: &String| item_tags.contains(t);
    match mode {
        TagMode::Any => selected.is_empty() || selected.iter().any(has),
        TagMode::All => selected.iter().all(has),
        TagMode::Exact => item_tags.len() == selected.len() && selected.iter().all(has),
        TagMode::None => !selected.iter().any(has),
    }
}

fn item_in_folder(item: &Item, folder_id: &str, include_descendants: bool, tree: &FolderIndex) -> bool {
    if !include_descendants {
        return item.folders.iter().any(|f| f == folder_id);
    }
    let allowed = tree.descendant_ids(folder_id); // includes folder_id
    item.folders.iter().any(|f| allowed.contains(f.as_str()))
}
```

### Search

The top search field searches names, notes, links, tags and folder names, from 2 characters, debounced 300 ms, within the current scope. A word matches any part of those texts, without case (`post` finds `Poster`, `плак` finds `ПЛАКАТ`). Syntax as in Eagle: words are AND-ed, `OR` (in capitals) between words, parentheses, a leading `-` excludes a word or a group; `"…"` keeps spaces inside one word. The native library answers from an FTS5 trigram index (arch §15.3, D-047). `Ctrl+F` focuses the field, `Esc` clears it.

### Sort

| UI label | Key | Comparator |
|---|---|---|
| Date added | `added` | `addedAt`, default newest first |
| Date created | `btime` | `fileCreatedAt`, else `addedAt` |
| Date modified | `mtime` | `fileModifiedAt`, else `addedAt` |
| Title | `name` | Natural, case-insensitive (`icu_collator` with numeric ordering) |
| Extension | `ext` | Same collator |
| File size | `size` | u64 |
| Dimensions | `resolution` | `width * height` |
| Rating | `star` | Integer |
| Duration | `duration` | Number, missing as 0 |
| Tags | `tag` | The item's smallest tag by the collator; items without tags first |
| Random | `random` | One shuffle per query identity, stable while the query is unchanged |
| Manual | `manual` | The item's position in the open folder; items without one (they joined after the last reorder) come after, by `addedAt`. Drag to reorder inside a folder in the native library. |

A folder or smart folder can carry its own sort (`orderBy`, ascending flag). It applies when the screen opens, until the user picks another sort; it replaces the global sort for that screen only. Picking Manual in a folder without its own sort makes Manual that folder's sort; other picks there change the global sort. A folder's own sort is set in its properties (D-043).

Missing values count as the smallest, except a missing manual position (above). Ties fall back to date added, then the id, so the order is total and the same on every run. Names, extensions and tags compare through collation sort keys computed once per item. Duration waits for video: it is not in the Sort By menu, though the sorter supports it. Manual is in the menu while a folder is open.

Layout is presentation and does not affect the query: a grid whose column count follows the window width and a size slider, and a list.

### Smart folder rules in v1

The local evaluator supports: `tags` and `folders` with `union`, `intersection`, `equal`, `identity`, `empty`, `not-empty`; `type`; `rating`; `shape`; `name`, `annotation`, `url` with `contain`, `uncontain`, `is`, `startWith`, `endWith`; `createTime`, `mtime`, `btime` with before, after, between and "within the last N days"; `width`, `height`, `fileSize` with comparisons. The exact semantics, including Eagle's quirks, are in arch §16; the native library matches Eagle where that is sensible and documents every difference in `decisions.md`.

"Save as smart folder" turns the current scope and chips into rules: a folder becomes `folders union` (with its subfolders when Include subfolders is on), Uncategorized `folders empty`, Untagged `tags empty`, the tag and folder chips `union` / `intersection` / `equal` / `identity` for Any / All / Exact / None, type, rating and shape `equal`. Inside a smart folder the new one goes under it, so its rules apply too. A query that uses something the rules cannot express (search text, the trash) is refused with an explanation. Where the native library differs from the desktop is in D-048.

### How a screen is computed (arch §6)

One screen is a `LibraryQuery`:

```rust
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQuery {
    pub scope: Scope,                  // All, Folder { id, include_descendants }, Unfiled, Untagged, Smart { id }, Trash
    pub tag_mode: TagMode,
    pub tags: Vec<String>,
    pub extra_folder_ids: Vec<String>, // filter chips beyond the open folder
    pub folder_mode: TagMode,
    pub ext: Option<String>,
    pub rating: Option<u8>,            // None = no rating filter; Some(0) = unrated
    pub shape: Option<String>,
    pub text: Option<String>,
    pub sort: SortKey,
    pub ascending: bool,
}
```

1. The source loads the **scope** as a list of light items (`load_scope`). The native library does that with one SQL query per scope; a running Eagle with API calls (section 8.3).
2. The local pipeline in `library-core` runs on a blocking thread: lock filter → scope union (subfolders) → tags → extra folders → type, rating, shape → search ids ∩ list → sort.
3. Rust keeps the sorted result as a **view** and returns `{ viewId, total }`. The UI renders a virtualized grid and asks for windows of at most 200 cells with `view_range(viewId, start, len)`. **Never send the whole list over IPC.**
4. After a write the affected views are recomputed and the UI gets a `views-invalidated` event.

Changing chips or sort on the same scope does not reload the scope.

---

## 6. Editing

- **One item:** name (renames the stored file too), note, link, rating, tags, folders. A change is an intent (`ItemEdit`: add or remove tags, add to or remove from folders, set the folders, set the rating, …), applied to the current state of the item in Rust (arch §7.1).
- **Several items:** with a multi-selection in the grid, add or remove tags, add to or remove from folders, set the rating, move to the trash. Dropping items on a folder adds them to it. Dropping them on a tag row in the left bar (a tag in Tag groups or Recent tags) adds that one tag and leaves their other tags. If every dropped item already has the tag, the drop writes nothing. Dropping items on Trash moves them to the trash, the same edit and confirmation as Delete (D-073). A read-only library refuses these drops. One transaction per batch; progress for large batches.
- **Optimistic UI:** the UI shows the intended state at once. On an error, Rust returns the stored state, the view is recomputed and the UI rolls back.
- **Validation before writing:** rating 0–5, name not empty, name and note at most 10 000 UTF-16 units, no empty tags or folder ids.
- **Undo:** the last item edits (not a permanent delete) can be undone with `Ctrl+Z`, one at a time, while the library is open: up to 50, kept in memory (D-050). Folder and tag-vocabulary changes are not undone. Later: a longer history from `change_log`.
- **Folders:** create, rename, description, colour, move (drag in the tree or "Move to…"), reorder siblings, delete, lock and unlock, set the sort override.
- **Tags:** rename, merge, delete everywhere; tag groups: create, rename, colour, add and remove tags, delete.
- **Open:** "Open with the default app" and "Show in Explorer" for the original file; "Copy file" puts the file on the clipboard; dragging the preview (or a selection's stack of thumbnails) out of the right bar drops the original files into other apps. Items of locked folders stay inside: their originals are encrypted (D-052).

---

## 7. Screens

One window and one library per window (D-024). The layout is desktop-first and responsive, so a phone layout can be added later without restructuring. **UI language** defaults to English; Russian and Chinese (Simplified) are selectable in Settings (D-069). Strings go through the message catalogs in the host crate. How the menu, the commands and desktop behaviour are built is in arch §8.

### 7.1 Window and menu

- **Frame.** The native Windows title bar: the window title (`My Library — egl-viewer`), minimize, maximize with Snap Layouts, close, light or dark with the app theme. Under it the app's own **menu bar**, not the classic Win32 menu. A custom title bar that merges both rows is an option later (D-025).
- **Menu.** Every item and shortcut comes from one command registry (arch §8). Items of later features do not exist before their slice, and a menu without items is hidden. In a read-only library, write commands are disabled, with a tooltip that says why.

| Menu | Items |
|---|---|
| **File** | New Library… , Open Library… `Ctrl+O` , Open Recent ▸ , Close Library `Ctrl+W` · Add Files… , Import Folder… · Import Eagle Library… , Connect to Eagle… , Export to Eagle… · Settings… `Ctrl+,` · Exit `Alt+F4` |
| **Edit** | Paste `Ctrl+V` · Undo `Ctrl+Z` , Select All `Ctrl+A` , Copy File `Ctrl+C` , Move to Trash `Delete` , Find `Ctrl+F` |
| **View** | Theme ▸ System / Light / Dark , Full Screen `F11` · Sidebar `Ctrl+B` , Details Panel `Ctrl+I` , Filter Bar · Grid / List , Larger / Smaller Thumbnails `Ctrl+=` / `Ctrl+-` , Show Names , Sort By ▸ · Preview `Space` · Include Subfolders |
| **Library** | New Folder `Ctrl+Shift+N` · Lock All Folders `Ctrl+L` · Library Settings… , Backup… (D-064), Back Up Now , Check Library , Rebuild Thumbnails , Show in Explorer |
| **Help** | Keyboard Shortcuts `F1` , Check for Updates… (D-062), About egl-viewer , Open Log Folder |

- **GPUI host (D-140).** The grid, the list and the folder tree have right-click menus. Shortcuts beyond the table: `F2` Rename (the selected item, else the open folder), `1`–`5` rate and `0` removes the rating, `Ctrl+Shift+J` Add to Folder(s)…, `Ctrl+J` Go to Folder, `Shift+Delete` Remove from Current Folder, `Ctrl+T` Add Tags…, `Ctrl+Shift+C` / `Ctrl+Shift+V` Copy / Paste Tags, `Ctrl+Alt+C` Copy File Path; in the preview of an animated GIF, `P` play / pause and `,` / `.` previous / next frame (D-154). In the preview of a video or audio item, `P` or `K` play / pause, `,` / `.` step a frame, `J` / `L` jump 5 s and `M` mutes (D-170). On this host Switch Library is `Ctrl+L` and Lock All Folders is `Ctrl+Shift+L` (D-137).

- **Closing the window quits the app**: there is one window and no tray icon (arch §9). The window close (X) and File → Exit both ask for confirmation first; Cancel keeps the app open. Running imports or exports use the stop-and-quit wording (D-072). **Close Library** returns to Welcome and does not quit.

### 7.2 Welcome

No library open; the first screen after login (D-017). The login gate is section 12.

- Primary: **Create library** (name and location; makes `Name.eglib`; the location starts at the default folder, `%USERPROFILE%\egl-library` unless changed, and sync folders and network paths are refused), **Open library** (folder picker; an Eagle library folder gets its own message), **Recent libraries** (name, path, last opened; a missing library is marked and can be removed from the list).
- Secondary, under "Eagle": **Import an Eagle library** (section 8.2) and **Connect to a running Eagle** (section 8.3).
- On start the app reopens the last library unless it was closed explicitly, the user holds `Shift`, or another instance has it open (then Welcome says so).

### 7.3 Library screen

```
┌ My Library — egl-viewer ──────────────────────────────── ─ □ ✕ ┐
│ File  Edit  View  Library  Help                                 │
├─────────────┬──────────────────────────────────────┬────────────┤
│ left bar    │ toolbar                              │ right bar  │
│             ├──────────────────────────────────────┤ (metadata) │
│             │ filter bar (when shown)              │            │
│             ├──────────────────────────────────────┤            │
│             │ grid or list                         │            │
├─────────────┴──────────────────────────────────────┴────────────┤
│ status bar                                                      │
└─────────────────────────────────────────────────────────────────┘
```

- The left bar is 200–360 px wide and the right bar 280–480 px; both can be resized and are toggled with `Ctrl+B` and `Ctrl+I`. Widths and open state are remembered. Below about 1200 px of window width the right bar slides over the grid instead of taking space. The smallest Tauri window is 900 × 600. The GPUI host's minimum is 960 × 600 (D-138).
- A read-only library shows a banner above the toolbar with the reason: open in another instance (with **Open for editing**), inside a sync folder, or on a read-only medium. The window title and the status bar show a badge; so does a connected Eagle.

**Left bar**, top to bottom:

```
My Library            ⋯
All                12 480
Uncategorized         312
Untagged            1 004
Trash                  27
▾ Folders          ⊕  ⇲
   ● Posters          210
     ▸ 2024            88
▸ Locked folders
▸ Smart folders    ⊕
▸ Tag groups
▸ Recent tags
```

- The library name, with a menu (Library Settings, Show in Explorer, Close Library) and the read-only badge.
- All, Uncategorized, Untagged and Trash with their counts. Items dropped on Trash go to the trash (same confirmation as Delete; D-073).
- **Folders:** the tree with colour dots (one of Eagle's eight colours or any colour, D-159) and counts (direct items with subfolders off, items including subfolders with it on). ⊕ creates a folder. ⇲ is the **Include subfolders** switch, also in the View menu; it changes the counts, so it sits next to the tree. Items dropped on a folder are added to it. Folders are dragged to move them (onto a folder: inside it; near its top or bottom edge: before or after it). The context menu has New Subfolder, Rename (`F2`), Colour, Move To… and Delete… (`Del`).
- **Locked folders:** each locked folder with its name, its tag (label and colour) and a lock. Locked folders appear only here, not again in Folders (D-059). Choosing a row prompts for that folder only; choosing an unlocked one opens it. The section's menu has Unlock…, Lock Now, Change Password…, New Recovery Key…, Add YubiKey…, Remove YubiKey… and Remove Lock…. Folders' menu still has Lock… for an unlocked folder. Library → Lock All Folders (`Ctrl+L`) locks every open folder again.
- **Smart folders:** their own tree.
- **Tag groups** and **Recent tags.** In the GPUI host each group is a row in its colour that unfolds into its tags, followed by Ungrouped; groups and tags have context menus (D-162). The full tag vocabulary is not in the left bar, because it grows to thousands of tags; it is in the Tags filter (7.4). Choosing a tag here opens All with that tag as a filter: tags are filters, not scopes (section 5). Items dropped on a tag row are given that tag; their other tags stay. The row highlights while the items are over it, the same way a folder does when items would be added to it. If every dropped item already has the tag, nothing is written. A read-only library refuses the drop. The name is kept exactly: `"Poster"` and `"poster"` are different rows (D-057).
- Sections collapse and stay collapsed. Arrow keys and `Enter` move through the rows.

**Toolbar**, one row over the grid:

```
Posters › 2024   88 items          Search…          Filter (2) │ Date added ↓ │ Grid List │ ━━●━━ │ + Add files │ ⓘ
```

- Where the user is (a nested folder shows its path) and the item count.
- Search (`Ctrl+F`, section 5): results follow the typing (250 ms after the last key; Enter or leaving the field searches at once), an × clears the field, and the field keeps its own editing keys (Backspace, arrows, Ctrl+A …) (D-179); centered in the free space between the title and Filter. The field is about 24rem wide and shrinks when that space is narrower, so it does not cover Filter, sort or Add files and does not stretch across the row. **Filter** with the number of active filters; sort (key and direction); grid or list; the thumbnail size slider (also `Ctrl` + mouse wheel); **Add files** as the primary button, disabled with a tooltip in a read-only library (hidden in the trash; Empty Trash takes its place — D-071); the right bar toggle.

**Filter bar**, under the toolbar. The Filter button shows and hides it, and it appears on its own while a filter is active:

```
[Tags: Poster, Logo · Any ▾] [Folders ▾] [Type ▾] [Rating ▾] [Shape ▾]        Clear all   Save as smart folder
```

Each filter is a button with a dropdown panel. Colour is a popover: Any colour, Documents (light, colourless images such as photographed papers), 16 swatches and the share of the image the colour must cover (D-176). Later filters (size, dates) join the same row.

**Grid**, in the middle:

- The **grid** is justified (D-067): rows share one height, each cell's width follows the item's aspect ratio so the row fills the container; the size slider sets the target row height. Gutters stay small (`8px`). Missing width/height use a square. **List** shows a small thumbnail, name, type, size, dimensions, date added, rating and tags. Masonry can still come later as another View mode.
- A cell shows the thumbnail, or the first palette colour and the extension until the thumbnail exists; stars when rated; the name under the image when View → Show Names is on (same caption density as before — name only). Selection is an outline. An animated GIF plays in its cell while the pointer is over it and returns to its thumbnail when the pointer leaves; never with Windows animation effects off (D-154). A video cell shows its poster frame and its length, an audio cell its length; moving the pointer across it shows the frames along the video, with a thin progress line (D-169, D-170). The toolbar has All · Photo · Video · Audio modes, a segmented control in the middle of the row, which set the Type filter to that bucket; the size slider sits in the right group (D-171).
- Click, `Ctrl`, `Shift`, a rubber band and `Ctrl+A` select. Double-click or a short `Space` opens the full-size preview; a second short `Space` closes it. Holding `Space` peeks (opens while held, closes on release) (D-073). `Enter` moves focus to the right bar and opens it if hidden. `Delete` moves the selection to the trash after a confirmation; the confirmation has a "Don't ask again" checkbox that turns the confirmation off for good, and Settings → General turns it back on.
- Files dragged onto the window show an overlay "Add to <scope>".
- Empty states: a new library ("Drop images here", a big drop target and Add files), a folder without items, no results (with Clear filters), an empty trash.

**Right bar** (metadata):

```
[ preview — click for full size ]
■■■■■■                        palette
Name     IMG_1042             edited in place
★★★★☆
Tags     [Poster ×] [Logo ×] [+ add…]
Folders  [Posters ×] [+ add…]
Note     …
Link     https://…  ↗
▾ Info      JPG · 2.4 MB · 4000 × 3000 · Added · Created · Modified
▸ EXIF      camera, lens, ISO, aperture, shutter, focal length, date
▸ Comments  read-only
[Show in Explorer]  [Open with default app]
```

- **One item:** as above. A field saves on `Enter` or when focus leaves it, and `Esc` reverts it; there is no Save button. The UI shows the change at once and rolls back on an error (section 6). Tags get vocabulary suggestions and accept new names; the folder picker is the tree with multi-select; the link opens in the system browser. Fields are disabled in a read-only library.
- **Several items:** "N items selected" with a stack of thumbnails (the editing slice; folders have the count and Add to folder…). Tags on every selected item look normal, tags on only some of them are dimmed. Batch rating, add or remove tags, add to or remove from folders, move to the trash; the total size.
- **Nothing selected:** the open folder's properties (name, description, colour: the eight dots plus a custom-colour picker in a native library, D-159; folder tags, sort override); outside a folder, "Select an item".
- **In the Trash:** Restore and Delete permanently instead of the file buttons, and "In the trash since …".

**Status bar:** the item count, the selection count, import progress (a click opens the Imports panel), the read-only badge.

**What is on this screen, by slice:**

| Slice | Adds |
|---|---|
| Frame | The whole frame: menu bar, both bars with resizable widths, toolbar, status bar. The left bar with All, Uncategorized, Untagged and Trash; the grid area with the new-library empty state; the right bar with "Select an item" |
| Adding files | Add Files, Import Folder… (review before copy), Edit → Paste, files dropped on the window, counts in the left bar and the toolbar, import progress in the status bar, the duplicate question, the Imports panel |
| Grid | The grid and the list, selection, sort, thumbnail size, the read-only right bar, the full-size preview |
| Folders | The folder tree, Locked folders, Include subfolders, folder properties in the right bar |
| Tags and filters | The Filter button and the filter bar with the Tags filter, tag groups and recent tags, editing tags (one item and a selection) |
| Search and smart folders | Search, the other filters of the filter bar, Save as smart folder, Smart folders |
| Editing and trash | Editing the other fields, batch edits, the trash, file actions |

### 7.4 Other screens and dialogs

1. **Full-size preview.** A layer over the whole window, not a new window. The original at window size, zoom and pan, rotate clockwise (toolbar button or `R`), previous and next in the current view; `Esc`, a short `Space`, or a left/right click on the empty letterbox beside the image closes it; holding `Space` peeks while the key is down (D-073). Rotation is display-only for this preview session (D-058). It opens with a short zoom out of its cell (D-040). Originals the host cannot decode for display (TIFF, PSD) show their 512 px thumbnail; the host decodes them, and HEIC, at full resolution in the background (D-143). An animated GIF plays, at any rotation, with a player bar at the bottom of the stage: previous frame, play / pause, next frame, a track of the frames to click or drag along, the frame number, the loop length and the speed (0.25×, 0.5×, 1×, 1.5×, 2×). `P` plays and pauses, `,` and `.` step one frame back and forward and pause. Delays of 10 ms or less play at 100 ms, as in browsers. With Windows animation effects off a GIF opens paused (D-154). A video or audio item plays from its original (FFmpeg, D-168): the poster first, then the video, zoomable and pannable; audio shows a card with its name and length. The bar has previous frame, play / pause, next frame (video), a seek track to click or drag (key frames while dragging, the exact frame on release), the time, mute, a volume of ten steps and the speed (0.25×–2×, pitch kept). At the end it pauses; play starts over. Volume and mute carry over for the session (D-170).
2. **Tags filter** (the tag sheet): the dropdown panel of the Tags filter. Search box over the vocabulary, mode segment Any / All / Exact / None, counts, recent tags, groups as sections; rename, merge and delete from a context menu.
3. **Smart folder editor.** Name, AND/OR, the rules of the v1 subset.
4. **Folder dialogs.** Create, rename, move, delete (with the item count and the trash option), lock and unlock. Lock and unlock, password change, and recovery are for the chosen folder. The lock dialog edits that folder's name and tag.
5. **Imports panel.** Opens by itself when an import starts, and from View → Imports; the toolbar shows an Imports button only while something is imported (with the count) or a finished import has an unread report (with a dot). Running and finished imports, newest first, each with its kind and stage in words, a bar while it runs, the counts added, duplicates skipped and failed (D-175).
6. **Library settings** (dialog, Library menu). Name, location (reveal in Explorer), size on disk, backups (make one now, restore), rebuild thumbnails, check library.
7. **Settings** (`Ctrl+,`; a dialog with its sections on the left). **General**: theme (System, Light, Dark), **language** (English, Russian, Chinese Simplified; D-069), reopen the last library on start, whether `Delete` asks before moving items to the trash (the trash confirmation's "Don't ask again" turns this off; the switch turns it back on), and the default folder for new libraries (`%USERPROFILE%\egl-library`, checked like any library location, D-021). Further sections arrive with their features, for example the Eagle connection.
8. **About egl-viewer** (dialog). Icon, name, version, build date and commit, links, and **Third-party licenses**: a file generated by the release build, because the MIT and Apache licences of the dependencies require showing their texts.
9. **Keyboard Shortcuts** (`F1`): every command with its shortcut, read from the command registry.
10. **Check for Updates…** (Help menu, D-062 / D-153). Asks the rolling `gpui-latest` prerelease for `latest-gpui.json` (not GitHub Latest). If a newer build exists, confirms with the user. On Windows: downloads the NSIS setup, exits, installs silently, and relaunches. On macOS/Linux: opens the download URL. Authenticode / minisign remain deferred. A silent check after launch uses the same flow; skip with `EGL_SKIP_UPDATE_CHECK`.
11. **Eagle screens:** the import wizard (section 8.2), Connect (section 8.3), export (section 8.4).
12. **Backup** (Library menu, D-064) and **Export** (context menus of folders and tags): one dialog. It picks what to export (the whole library, chosen folders with or without subfolders, chosen tags), shows the item count and bytes from the database, the format (a folder tree, a ZIP or a 7z archive with a compression level), an optional passphrase for the archive (at least 8 characters): for a ZIP either an age file `name.zip.age`, encrypted while it is written so no plaintext copy touches the disk (D-110), or a standard ZIP with AES-256 entries that 7-Zip, WinRAR and Keka open, its names visible (D-112); a 7z is AES-256 encrypted with its names hidden (D-113), and the destination folder (the last one is remembered). While it runs, a progress bar; afterwards, the result: what was written where, skipped missing files, **Show in Explorer**. The export runs in the background and can be cancelled; the partial output is removed, after a failure too. A destination inside an Eagle or egl-viewer library is refused (D-088). It reads only, so a read-only library exports too; items of locked folders stay home and are counted as hidden. `egl-export.json` at the export root carries the exported items' metadata. Distinct from Back Up Now, which copies the database (§7.4 item 6). **File → Decrypt Archive…** opens such a `name.zip.age` with its passphrase into `name.zip`, next to it by default (D-110); the iPhone app has it in Settings.

### 7.5 Look and motion

- **Theme:** follows Windows by default; View → Theme and Settings override it. The grid background is a neutral grey, so it does not tint the images.
- **Desktop density**, not web density: rows in the left bar and in menus about 28–32 px high, body text 13–14 px.
- **Motion is short and only where it helps:** menus, popovers and dialogs fade and scale (120–150 ms), panels slide (150–200 ms), a thumbnail fades in when it loads, the full-size preview grows out of its cell. Scrolling, sorting, filtering, selection and hover never animate. All motion is off when Windows animation effects are off. Rules in arch §8.
- **A desktop app, not a web page:** no browser context menu, reload, printing, page zoom or text selection in the chrome (arch §8).

---

## 8. Eagle compatibility

### 8.1 Levels

| Level | What | How |
|---|---|---|
| 1. Import | Turn an Eagle `.library` folder into a native library, Eagle may be closed | `eagle-folder` → `library-native` (8.2, arch §15.6) |
| 2. Live | Browse and edit a running Eagle's library from egl-viewer, on the same PC or from another computer | `eagle-api` source (8.3, arch §4–§11) |
| 3. Export | Put native items, folders, tags and smart folders into a running Eagle | The Eagle API (8.4, arch §15.7) |
| — | Write Eagle's `.library` files | **Never** (8.6) |

The app's screens, query engine and edit rules work the same for every source through the `LibrarySource` trait; `capabilities` tell the UI what the open source cannot do (for example a running Eagle cannot delete folders or empty the trash through its API).

### 8.2 Import from an Eagle library folder

An Eagle library is a folder `Name.library`:

```
Name.library/
  metadata.json        folders, smart folders, quickAccess, tagsGroups, modificationTime
  mtime.json           { "<id>": <lastModified ms>, …, "all": <item count incl. trash> }
  tags.json            { "historyTags": [], "starredTags": [] }
  saved-filters.json
  backup/              backups of metadata.json
  images/{id}.info/{name}.{ext}
  images/{id}.info/{name}_thumbnail.png     missing when noThumbnail is true
  images/{id}.info/metadata.json            the item, plus lastModified
```

To find the original inside `{id}.info`, match by extension instead of rebuilding the file name. Eagle's thumbnail edge is 320 px; small images often have none (`noThumbnail: true`, correction 29).

The wizard:

1. Pick the `.library` folder. The app checks `metadata.json`, `mtime.json` and `images/`, and shows the library name, item count and size.
2. Pick the target: a new native library (default; suggested name from the Eagle library) or the open one (then the Eagle folders go under a new folder named after the Eagle library).
3. Options: import items from Eagle's trash into the trash (default off).
4. Import, with progress; it can be cancelled and resumed later.
5. Report: imported, skipped (missing originals, unreadable metadata), duplicates by content hash, locked folders.

Rules (arch §15.5, §15.6):

- Reading is read-only. Eagle may have the library open: Eagle writes `metadata.json` files in place, so a reader can catch half-written JSON and must retry (correction 37). `mtime.json` is the manifest of what to read.
- Never read or write Eagle's cache in `%APPDATA%\Eagle` (correction 36).
- Mapping: name, ext, size, dimensions, duration, bpm, note, link, rating (absent means 0), date added (`modificationTime`), file dates (`btime`, `mtime`), palettes, tags (case preserved), folders (same tree and order), folder colours, descriptions, folder tags, per-folder sort, covers, smart folders (`conditions` unchanged), tag groups (`tagsGroups`), comments, EXIF from `rawMetas` (correction 38), manual order from `order` (correction 39). Eagle ids are kept as `eagle_id`.
- Eagle's `_thumbnail.png` is used as the first thumbnail when present, so the import is fast; missing thumbnails are generated in the background.
- **Folder passwords are not imported.** Locked Eagle folders are imported as locked folders with no password, no salt, and no identity. The report lists them. Files stay plaintext, and the content stays hidden, until the user sets a password or removes the lock. The Locked folders section offers **Set Password…** (the same dialog as Lock…, with a recovery key at the end) and **Remove Lock…**, which asks for no password (D-088).
- Duplicates inside the Eagle library are kept as separate items (they are separate in Eagle). Eagle's own notion of a duplicate is a metadata fingerprint, not content (correction 41).

On the phones (D-116) the import starts from Welcome and always makes a new library in the app's own storage; the same core task runs through the C ABI (`egl_eagle_library_info`, `egl_import_eagle`). Step 2's "the open one" is desktop-only.

Later: `.eaglepack` files as a second import source (arch §17); "Follow Eagle", an incremental re-import through `mtime.json`.

### 8.3 Connecting to a running Eagle

"Connect to a running Eagle" opens a session with the library that Eagle has open, through Eagle's local API on port 41595. The app works on the PC that runs Eagle (it connects to `127.0.0.1`) and from another computer in the network or over Tailscale. Some things are simpler on the same PC: files are added by path, and thumbnails are read from disk.

What works in a live session: browsing (scopes, filters, search, sort, smart folders), editing items (rating, note, name, tags, folders, trash), folder create, rename, move and colour, tag rename and merge, adding files, thumbnails. What does not: deleting folders, deleting tags globally, emptying the trash, manual reorder, writing comments (Eagle's API has none of these, or only with restrictions).

#### Base URL

`http://{host}:41595`

Default port is **41595**. The server listens on all interfaces. Windows Firewall often blocks inbound connections. The Connect screen must say so when the socket times out. Over Tailscale the host is the PC's Tailscale address or MagicDNS name.

#### Auth

A request is accepted when either:

- the `Host` header is exactly `localhost:41595`, `127.0.0.1:41595`, or `0.0.0.0:41595`, or
- `token` equals Preferences → Developer → API Token in the desktop app.

From another computer the Host is the PC's network address, so the token is mandatory. An empty desktop token rejects every remote call with HTTP 401. On the same PC (`127.0.0.1`) Eagle accepts requests without a token, but the client still sends it, so there is one code path.

Send the token like this:

- GET and DELETE: query `?token=`.
- POST and PUT: JSON field `token` **only**. The server would also read `?token=` on a POST, but Eagle writes the URL of every POST into its log file, so a query token ends up on disk (correction 6).

Store the token in the OS secret store, and only after a connection with it succeeded. Do not log it. Do not pass it to the UI. An export into Eagle uses the token typed in its dialog or else the saved one for that host and port (D-089). The session holds one source at a time: opening a native library ends a live Eagle connection, and connecting closes the open library.

The token is not a real security boundary. A LAN request without a token but with the header `Host: localhost:41595` is accepted (verified live, correction 5). The client never uses this trick. The Connect screen tells the user to limit the Windows Firewall rule for 41595 to the Private profile and, ideally, to the client's IP. For access outside the LAN use Tailscale or WireGuard; never forward the port.

#### Request bodies

Escape every non-ASCII character in the serialized JSON as `\uXXXX` before sending. Eagle concatenates body chunks as strings, so a multibyte UTF-8 character split across two TCP chunks becomes `U+FFFD` (verified live with a Cyrillic note, correction 7). Escaped bodies are pure ASCII and arrive intact.

#### Connection rules

- At most 2 concurrent requests to Eagle (a `tokio::sync::Semaphore` with 2 permits in the transport). Every API call runs on Eagle's UI thread and re-filters the whole library (correction 10).
- Page loops are sequential.
- Idle pooled connections are dropped after 4 s (`reqwest` `pool_idle_timeout`). Eagle's Node server closes idle sockets after 5 s (correction 21).
- Timeouts: connect 3 s (8 s over Tailscale), whole request 60 s.

#### JSend

Success:

```json
{ "status": "success", "data": {} }
```

Most v2 handlers reject a call by throwing a plain `Error`. That produces **HTTP 500** with `"status": "error"` and the text in `message`. v1 handlers reject with a bare string: then the text is in `data` and `message` is empty. Some calls report "not found" as a **success**. The client branches on `status` and on the HTTP code, and reads the error text as `message`, or else `data` if it is a string (correction 4, verified live).

| HTTP | When |
|---|---|
| 200 + `success` | Call worked |
| 200 + `success` + `data: false` or no `data` | Object not found: `item/update` returns `false`, `folder/update` returns no `data` |
| 200 + `fail` or `error` | Handler threw an explicit JSend failure (rare in v2) |
| 400 | Body is not JSON |
| 401 | Token missing or wrong |
| 404 | Unknown path. On `/api/v2/app/info` it means Eagle is older than 4.0 |
| 405 | Wrong method |
| 500 + `error` | Handler rejected the call: validation (`name too long (max 10000)`), missing id, bad rule, wrong unlock password |

The error model in code is `EagleError`, mapped one to one onto the flat `SourceError` and serialized to the UI as `{ kind, detail }` (arch §4.4, D-004).

#### Pagination

These list handlers are paginated: `item.get`, `item.query`, `folder.get`, `tag.get`, `tag.getRecentTags`, `tag.getStarredTags`, `tagGroup.get`, `library.history`, `smartFolder.get`, `smartFolder.getItems`.

The handler returns a full array. The server then slices it.

| Field | Default | Max |
|---|---|---|
| `offset` | 0 | — |
| `limit` | 50 | 1000 |

The slice is wrapped again, so the wire shape is **nested**:

```json
{
  "status": "success",
  "data": {
    "data": [],
    "total": 0,
    "offset": 0,
    "limit": 50
  }
}
```

Non-list calls put the payload directly in `data` (`library/info`, `item/add` → `{ "id": "..." }` or `{ "ids": [] }`, `item/update`, and so on).

Loop with `limit=1000` until `offset >= total`. Never assume one page is the whole folder. Restart the loop if `total` changes between pages, and deduplicate by id: every page is recomputed on the server, so a concurrent import shifts offsets (arch §4.5).

`item/get {id}` is also returned as a page. `item/countAll` and `item/getComments` are **not** paginated: `data` is the number or the array (correction 28).

Arrays (`ids`, `folders`, `tags`, `keywords`, `fields`, `items`) must be sent with **POST**. GET query strings cannot carry those arrays reliably. `smartFolder/getItems` ignores `fields` unless it is an array, so use POST there too (correction 17).

#### Endpoints to implement

Prefix `/api/v2`. Token on every call.

**Session**

| Method | Path | Body / query | Returns |
|---|---|---|---|
| GET | `/app/info` | — | Version and platform. Use as the connection test. |
| GET | `/library/info` | — | Reads `metadata.json` from disk: name, path, `modificationTime`, folders, smart folders, and tag groups under the key **`tagsGroups`**. Not paginated. New folders appear here only after Eagle saves (about 2 s, verified live). No counts. Folder nodes carry `password` fields. Returns `{}` when no library is open. Use it for library identity and change detection, not for the folder tree (correction 8). |
| GET | `/library/history` | offset, limit | Recent library paths on the PC. |
| POST | `/library/switch` | `libraryPath` | Switch the open library. Optional, easy to misuse. Hide behind an advanced action. |
| GET | `/library/icon` | `libraryPath` | Icon. v1 streams raw bytes (`/api/library/icon`). v2 JSON-wraps a buffer (correction 19). Prefer v1 for the icon if you show it. |

**Items**

| Method | Path | Notes |
|---|---|---|
| POST | `/item/get` | Primary list. See "Folder and tag filters on the server" for the intersection trap. |
| POST | `/item/query` | Full text: `query`, `offset`, `limit`. Supports `OR`, parentheses, leading `-`. No folder parameter. No `fields` and no sort: returns full item objects (verified live). Take only the ids from it. |
| GET | `/item/countAll` | Count. **Includes trashed items** (verified live). Use only as a rough library size. Not paginated. |
| POST | `/item/add` | Create. The client sends `base64` as a **data URL** (`data:image/jpeg;base64,…`) plus `name`, `tags`, `folders`, `annotation`, `website` (source link, stored as the item's `url`). Raw base64 without `data:` is treated as a URL and fails silently while the API still returns an id (correction 1). The call is **asynchronous**: the id comes back before the item exists (correction 2). `star` is ignored, except for batch items with `path`, which keep `star` and `modificationTime` (corrections 11, 35). Single mode ignores `modificationTime` and does not validate folder ids. The folder's `extendTags` are added to the item's tags. Batch: `items` (max 1000) returns `{ ids }`. Single returns `{ id }`. `path` is a filesystem path **on the Eagle PC**: the client uses it only when it runs on that same PC (arch §7.4); from another computer it sends a data URL. |
| POST | `/item/update` | `id` required. `tags` and `folders` **replace the whole array**. `star` is 0–5; other values are silently ignored. `name` and `annotation` are limited to 10 000 characters, otherwise HTTP 500. `isDeleted: true` moves to trash. Returns the full updated item. Unknown id returns success with `data: false`. |
| GET | `/item/getComments` | `id`. Returns a plain array, not a page. |
| POST | `/item/addComment` | `id`, `annotation`, and either a rect (`x`,`y`,`width`,`height`) or `duration` (video seconds). A comment without a rect or a duration is rejected (correction 18). |
| POST | `/item/updateComment` | `id`, `commentId`, fields to change. |
| POST | `/item/removeComment` | `id`, `commentId`. |

Do not call `setCustomThumbnail` or `refreshThumbnail` from the client. They take a PC file path.

**Folders**

| Method | Path | Notes |
|---|---|---|
| GET or POST | `/folder/get` | Without arguments: the live in-memory tree (root folders with nested `children`). With `id`: one folder. Paginated over root folders. **Primary source of the folder tree**: new folders appear immediately, and nodes carry `imageCount`, `descendantImageCount` and `extendTags`. Nodes have **no `parent` field**; derive the parent from nesting (corrections 8, 32). |
| POST | `/folder/create` | `name` required. Optional `description`, `parent`, `iconColor`. Returns Eagle's internal folder object (service keys, no `parent`). Take only `id` and re-read the tree (correction 33). |
| POST | `/folder/update` | `id` required. **Always send the full desired state**: `name`, `description`, `tags`, `iconColor`, `parent`. An omitted `parent` moves a nested folder to the root. An omitted `iconColor` clears the colour. An explicit `parent: null` moves a root folder to the end of the root list. Express "no colour" and "at the root" by **omitting** the field, never by `null` (corrections 3, 31, verified live). `tags` here are **the folder’s own labels**, not item tags. A move into the folder's own subtree is silently ignored (HTTP 200, nothing changes). Unknown id returns success without `data`. |
| POST | `/api/folder/unlock` | **v1.** `folderId`, `password`. Only way to reveal children of a locked folder. Unlocks the folder in the desktop UI too, until Eagle restarts. The error text arrives in `data`. A password with characters outside Latin-1 fails with HTTP 500 (correction 20). |

There is no v2 folder-delete.

**Smart folders**

| Method | Path | Notes |
|---|---|---|
| GET or POST | `/smartFolder/get` | Definitions, including `conditions`. |
| POST | `/smartFolder/create` | `name`, `conditions`. Optional `description`, `iconColor`, `parent`. |
| POST | `/smartFolder/update` | `id` plus fields. |
| POST | `/smartFolder/remove` | `id`. This deletes a saved filter, not items. |
| GET or POST | `/smartFolder/getItems` | `smartFolderId`. Optional `orderBy`, `fields`, `offset`, `limit`. |
| GET | `/smartFolder/getRules` | Property → method → value schema. |

**Tags**

| Method | Path | Notes |
|---|---|---|
| GET or POST | `/tag/get` | Vocabulary of `{ name, imageCount, groups, pinyin }` objects. `imageCount` excludes trashed items. Optional `name` filter (case-insensitive substring), `offset`, `limit`. |
| GET | `/tag/getRecentTags` | Read only. Not updated by API writes, so it is often empty (correction 27). |
| GET | `/tag/getStarredTags` | Read only. v2 cannot star a tag. |
| POST | `/tag/update` | Rename everywhere: `originalName`, `name`. |
| POST | `/tag/merge` | `source` removed, `target` kept on items. |
| GET | `/tagGroup/get` | Groups. |
| POST | `/tagGroup/create` | `name`, optional `tags`, `color`, `description`. |
| POST | `/tagGroup/update` | `id`, fields. `tags` replaces the group’s name list. |
| POST | `/tagGroup/remove` | Deletes the group only. Item tags stay. |
| POST | `/tagGroup/addTags` | `groupId`, `tags`. Optional `removeFromSource`. |
| POST | `/tagGroup/removeTags` | `groupId`, `tags`. Does not strip tags from items. |

**AI search: out of scope.** The `/aiSearch/*` endpoints exist in Eagle 4.0.0, but the client does not call them.

#### Item JSON the client needs

Request `fields` on large lists so responses stay small. For the grid and for local sorting:

```
id, name, ext, width, height, size, star, tags, folders, palettes, modificationTime, mtime, btime, duration, noThumbnail
```

For detail and for local rules, also ask:

```
url, annotation, comments, rawMetas, bpm, isDeleted
```

`item/get {id}` ignores `fields` and always returns the full item (correction 14). A real item is in `fixtures/eagle-api/item-get.by-id.1.json`. When a folder uses manual sort, add `order` to the list fields (correction 39).

| Eagle field | Meaning |
|---|---|
| `id` | Stable id, base36 time + random, uppercase. The client does not mint ids for `item/add`; the server does when `id` is omitted. |
| `name`, `ext`, `width`, `height`, `size` | As in section 3. |
| `tags` | `string[]`, case-sensitive. |
| `folders` | `string[]` of folder ids. |
| `url` | Source URL, often empty for local files. |
| `annotation` | Note. |
| `star` | Rating 0–5. The filter parameter is called `rating`. The key is **absent** until a rating is set; treat it as 0. |
| `palettes` | `[{ "color": [r, g, b], "ratio": 47 }, …]`, sorted by `ratio` descending; `ratio` can be fractional. |
| `comments` | Rect notes or video timestamps. |
| `modificationTime` | Date added (import time). |
| `mtime`, `btime` | File modified and created. Fall back to `modificationTime`. For items added through the API, they equal the time the PC wrote the file (correction 30). |
| `duration` | Seconds for audio and video. The key is absent for images. |
| `isDeleted` | Trash. List endpoints already drop these. |
| `lastModified` | On disk only (`images/{id}.info/metadata.json` and `mtime.json`). The API does not return it (arch §12.2). |
| `noThumbnail` | No generated preview; the desktop then shows the original file (correction 29). |
| `rawMetas` | EXIF strings: `camera`, `isoSpeed`, `aperture`, `focalLength`, `shutter`, `timestamp` (correction 38). |
| `bpm` | Beats per minute, for audio. |
| `order` | `{ "<folderId>": number }`: the item's manual position inside that folder (correction 39). |

| Eagle folder field | Meaning |
|---|---|
| `id`, `name`, `description` | |
| `children` | Nested folders. |
| `parent` | Request field on create and update only. `folder/get` nodes never have it: derive the parent from nesting. |
| `tags` | Labels **on the folder**, not on its items. |
| `extendTags` | Folder tags plus ancestor folder tags, display only. Do not write it. Do not filter items by it. |
| `icon`, `iconColor` | Colour name from the list in section 3, or empty. |
| `coverId` | Item id used as the cover, if set. |
| `modificationTime` | |
| `orderBy`, `sortIncrease` | Optional per-folder sort override. `sortIncrease == true` means ascending. |
| `imageCount`, `descendantImageCount` | `folder/get` only. Items directly in the folder, and items in the folder plus all descendants (own items included). May be absent right after `folder/create`; show "—", not 0. |
| `password`, `passwordTips` | Present in `library/info` nodes (empty string when not locked) and in locked in-memory nodes. The stored value is the password in base64. Drop both fields at parse time and keep only `is_locked = !password.is_empty()`. Never log raw bodies of `library/info` or `folder/get`. |
| `isUnLock` | In-memory flag after a successful unlock. Locked means `is_locked && !is_unlock`. Not verified live: the API cannot set a password. |

#### Folder and tag filters on the server

Membership semantics are those of section 3: direct membership, subfolders as a union. `POST /item/get` applies `folders` and `tags` as **intersection** ("must contain every id / every name"), AND-ed with ext, rating, shape, annotation, url and name keywords. Verified live: an item in a child folder is not returned by `item/get { folders: [parent] }`.

| User action | Correct request |
|---|---|
| Open folder F | `folders: [F]`. One id. Intersection of one id is "in F". |
| Show subfolders of F | Do **not** send `[F, child1, child2, …]`. Intersection would require the item to sit in every one of those folders. Collect descendant ids in the client and keep items whose `folders` overlap that set (union). |
| Folder filter mode Any | Same union in the client. |
| Folder filter mode All | `folders: [id, …]` is correct. |
| One tag chip | `tags: ["poster"]` is correct. |
| Tag mode All | `tags: [a, b]` is correct (intersection). |
| Tag mode Any, Exact or None | Do **not** send the array to `item/get`. Match locally. |

`isUnfiled: true` and `isUntagged: true` work. The handler only checks that the value is a boolean, then keeps empty folders or empty tags. **Sending `false` still filters to the empty set.** Omit the key when the user is not on that screen. `isUnfiled` matches only `folders: []` and skips items without the key; items added through the API get `folders: []` (correction 15).

Lists drop `isDeleted == true`. `smartFolders: [id, …]` on `item/get` is OR across those saved filters.

Lists do **not** drop items from locked folders. The desktop hides an item when any of its folders, or any ancestor of those folders, is locked and not unlocked. The client applies the same rule locally (lock filter, arch §6.4) in every scope (correction 9).

After the filter, v2 sorts with internal token `CREATEDATE` unless the call was `isSelected`. That order is not the user's sort. For any sort other than the raw server order, download every matching page (`limit=1000`) and sort in the client. Sorting one page of 50 is wrong. Never use v1 `GET /api/item/list` for sorting: it sorts again by import date.

`keywords` is name-only (`property: name`, method `contain`), AND across the words. It does not search tags. The top search uses `POST /item/query` (from 2 characters, debounced 300 ms); it has **no** folder argument, so intersect its ids with the loaded list of the current scope.

`ext` accepts an extension or the categories `video`, `audio`, `font`, `word`, `excel`, `powerpoint`, `presentation`; `rating: 0` matches unrated items (correction 40).

#### Query plan for a running Eagle

The principle: **the network loads the scope; everything else is computed in the Rust core** (arch §6).

1. `plan(query, library_count)` turns the query into a `FetchPlan`. The plan is the cache key: changing tags, ext, rating, shape, text or sort keeps the same plan and does not touch the network.
   - `Smart`: `smartFolder/getItems` (POST, `limit=1000`, loop).
   - Unfiled: `item/get { isUnfiled: true }` and nothing else. Untagged: `item/get { isUntagged: true }`.
   - Folder without descendants: `item/get { folders: [id] }`.
   - Folder with descendants: the whole library (`item/get` without filters), or one `folders: [d]` request per descendant when there are few of them (up to 20) and the library is large. Never put several ids into one `folders` array.
   - All: the whole library when `library_count` is at most 5000 (`LARGE_LIBRARY`). Above that, safe server prefilters: a single tag or mode All → `tags`; Any with several tags → one request per tag, union by id; a single `ext`, `rating`, `shape`. Loading All without chips above the threshold needs a confirmation. `library_count` comes from `item/countAll`, which includes trash; that is good enough for this decision.
   - Trash: not available (the API lists no trashed items).
2. The repository executes the plan: `item/get` pages at `limit=1000` with the grid `fields`, sequentially, restarting when `total` changes, deduplicated by id. Results live in an LRU of 6–8 snapshots with a 60 s TTL, served stale-while-revalidate. Progress goes to the UI through a Tauri `Channel`.
3. The local pipeline and the views of section 5 run unchanged.
4. After a write, `item/update` returns the full item. Patch it into every cached snapshot whose `ServerFilter` still matches it locally (folders, tags, unfiled, untagged, ext), otherwise remove it there. Snapshots with `rating` or `shape` filters are dropped. Affected views are recomputed. `item/add`, `tag/update`, `tag/merge` and folder edits drop all snapshots and the tree.

Load the tree from `GET /folder/get` without arguments, `limit=1000`, looping over root folders, and build `parent_of` from nesting. Fall back to `library/info.folders` only if `folder/get` fails. A single page of 50 folders is not the tree.

Refresh the tree and the open view when the window regains focus and on demand; there is no push channel from Eagle.

#### Writes to a running Eagle

- **Items:** `ItemMutator` re-reads the item with `item/get {id}` right before the write, applies the `ItemEdit` and sends complete `tags` and `folders` arrays (arch §7.1). Writes are serialized. On an error, Rust re-reads the item and the UI rolls back.
- On HTTP 401, go back to the Connect screen. On `Rejected` (HTTP 500 `error`, or 200 `fail`/`error`), show the message. On `success` with `data: false`, treat it as "not found" and remove the item.
- **Tags:** rename everywhere with `tag/update { originalName, name }`, merge with `tag/merge { source, target }`; do not loop over items. There is no endpoint to delete a tag globally; the action is hidden for this source. Recent and starred tag lists are read-only.
- **Folders:**
  - Create: `folder/create`, then re-read the tree.
  - Rename, description, colour, move: `folder/update` with the **full desired state** built from a fresh `folder/get {id}` (`FolderUpdateBody`, arch §7.2). Never send a partial body: it can move the folder to the root or clear its colour.
  - Move: reject cycles in the client before the call; the server ignores them silently. After the call, check in the re-read tree that the parent actually changed.
  - Folder's own tags: `folder/update.tags` replaces that list. This does not tag items already inside (verified live).
  - Locked folder: render a lock, children hidden. Unlock through v1 `/api/folder/unlock`, and warn that this also unlocks it on the PC until Eagle restarts. On failure, show the server message (from `data`). Do not cache the password.
  - Delete, reorder, cover and manual item order: not available.
- **Comments:** read-only (`item/getComments`).

#### Adding files to a running Eagle

- On the same PC as Eagle (`127.0.0.1` or `localhost`), Rust sends `item/add` in the batch form, `{ items: [{ path, … }] }`, even for one file. Eagle copies the file and leaves the source in place, and the batch form keeps `star` and `modificationTime` (correction 35, verified live).
- From another computer, Rust reads the file (size limit, for example 50 MB), encodes standard base64 without line breaks and sends `data:<mime>;base64,…` with the MIME type from the extension:

  ```
  POST /api/v2/item/add
  { "base64": "data:image/jpeg;base64,/9j/4AAQ…",
    "name": "IMG_1042",
    "folders": ["CURRENT_OR_EMPTY"],
    "tags": ["optional"],
    "website": "optional source link" }
  ```

  The `data:` prefix is mandatory (correction 1). The data URL is verified live for JPEG only; check other formats live. Captures: `fixtures/eagle-api/item-add.single.json`, `item-add.batch.json`.
- Then Rust polls `item/get {id}` (0.25 → 0.5 → 1 → 2 → 4 s, about 20 s in total) until the item appears; a 16 KB JPEG appeared in about 0.3 s live, path imports within milliseconds. For data URLs only: if the user chose a rating, Rust sends `item/update {star}` after the item appears.
- One file per request for data URLs, files one after another. Put `folders: [openFolderId]` when a folder is open, after checking the id against the tree.
- An "Importing…" cell stays in the grid until the item appears.

#### Thumbnails from a running Eagle

The API does not return image bytes. `GET /api/item/thumbnail?id=` returns a **Windows path as a string** inside JSend `data` (arch §11). Rust serves images through the same `thumb` URI scheme with a disk cache keyed by id and `modificationTime`. Sources, in order:

1. **Same PC as Eagle.** Rust reads the file whose path v1 `item/thumbnail` returns, only if it lies inside the library folder. When `noThumbnail` is true, it takes the original from `images/{id}.info/` (matched by extension) and downscales it.
2. **Another computer.** A tiny read-only helper **on the Eagle PC**, preferably an **Eagle plugin** (plugins can listen on ports: the MCP Server plugin uses 41596). It serves `GET /thumb/{id}`; when `noThumbnail` is true it serves the original downscaled. Do not build this helper inside the client.
3. If `item.url` is an `https` image, Rust may fetch it through the same scheme.

Until an image exists, the grid cell shows extension, star and the first `palettes` colour.

#### Connect screen

Host, port (default 41595), token. If Eagle runs on the same PC, the screen offers `127.0.0.1`. The Connect button calls `connect_eagle` in Rust, which does `GET /app/info` then `GET /library/info`. Persist on success (host and port in the store, token in Windows Credential Manager). Distinct error copy: wrong token (401), connect timeout (firewall), connection refused (Eagle not running or wrong port), unreachable (wrong network), Eagle older than 4.0 (404 on `app/info`), no library open (`library/info` returns `{}`). Help text: limit the firewall rule on the Eagle PC to the Private profile and this computer's IP. A Back button returns to Welcome.

### 8.4 Export into Eagle

Only through the API of a running Eagle, so that Eagle stays the writer of its library (arch §15.7):

1. The user connects to a running Eagle from the export screen and picks what to export: the whole library, or selected folders.
2. Folders top-down with `folder/create`, then `folder/update` with the full set of fields where needed; map ids; re-read the tree.
3. Items: on the same PC, batch `item/add` with `path` (keeps tags, folders, notes, the source link, `modificationTime` and `star`; up to 1000 items per call). From another computer, data URLs one by one with the size limit, then `item/update {star}`.
4. Comments with a rect or a video time, tag groups, smart folders (`conditions` unchanged).
5. Locked folders cannot be created through the API: they are exported unlocked after a warning.
6. Each new Eagle id is stored, so a second export updates instead of duplicating.

Later, without a running Eagle: write a `.eaglepack` (arch §17). Eagle imports it with new ids and a new date added, so it suits a one-time hand-over.

### 8.5 Later

- **Offline cache** of a running Eagle's library for a laptop that is sometimes away from the Eagle PC (arch §12.3).
- **Syncthing mirror** of an Eagle library, browsed read-only (arch §12.4).
- `.eaglepack` import and export (arch §17), "Follow Eagle" incremental re-import.

### 8.6 What the app never does with Eagle

- Write files inside `Name.library`, directly or through Syncthing, OneDrive or SMB. Eagle reloads newer files wholesale, and conflicting edits are lost silently (arch §12.2).
- Read or write Eagle's caches in `%APPDATA%\Eagle`.
- Call `/api/script/inject` or `/api/check`, or the `aiSearch` endpoints.
- Log the token, keep folder passwords, or show password material.
- Use the `Host: localhost:41595` trick.

---

## 9. Out of scope

- More formats: video (frames through a bundled LGPL `ffmpeg` process), audio, PDF (`pdfium`), fonts, RAW.
- A full-disk reviewed scan (pick a drive, skip Windows / Program Files / caches) stays out of v1; Import Folder… covers the reviewed case for one chosen folder (D-061, EGL-31).
- Linked folders; hard links instead of copies for imports from the same NTFS volume.
- Writing comments; a longer undo history.
- Sync between devices and remote access to a native library (arch §15.8).
- The Eagle extras of section 8.5.
- **Localisation (further languages):** English, Russian and Chinese Simplified ship now (D-069). Further locales reuse the same catalogs.
- **macOS:** the same app; Keychain for the token, signing and notarization.

AI features are not planned.

---

## 10. Tests the agent must write

Every change comes with tests (`CLAUDE.md`). Rust unit tests run with `cargo test`, without a window and without a network.

Core (`library-core`):

| Case | Expect |
|---|---|
| Item folders `[child]`, open parent, subfolders off | Hidden |
| Same item, subfolders on | Shown |
| `tags [a,b]`, mode Any, selected `[a]` | Match |
| Mode All, selected `[a,b]`, item tags `[a]` | No match |
| Mode Exact, item tags `[a,b,c]`, selected `[a,b]` | No match |
| Mode Exact, item tags `[a,b]` | Match |
| Mode None, selected `[a]`, item tags `[a,b]` | No match |
| `Poster` vs `poster` | No match |
| Folder move under its own descendant | Rejected before any write |
| Item in a locked folder, or under a locked ancestor | Hidden in every scope |
| Chip or sort change on the same scope | The scope is not reloaded |
| `view_range` | Never returns more than the requested window |
| Every sort key, with missing values | Expected order; random is stable for one query |
| Smart folder rules of the v1 subset | Match the semantics of arch §16 |

Native library (`library-native`, `media`), with temporary folders:

| Case | Expect |
|---|---|
| Create, close, open | Same name, id, `application_id` and `user_version` |
| Reserved locked-folder columns and tables | Exist and stay empty |
| Open a library from a newer format version | Refused, nothing written |
| Open a folder without `library.db`, a SQLite file with another application id, an Eagle library folder | Refused as not a library; the Eagle case says so |
| Create with an invalid name (`CON`, `a<b`, `name.`, empty) or an existing target | Refused, nothing created |
| Create fails midway (simulated) | No `Name.eglib`; the temporary folder is gone |
| Second open while locked | Read-only, reason `otherInstance`; `files/.tmp/` untouched, no backup, nothing written |
| Writer killed (the OS released the lock, `meta.open_by` left) | The next open takes the lock, reports the unclean close and runs `quick_check` |
| Library in a sync folder (fake OneDrive variable, registered sync root, `.stfolder`) | Create refused; open read-only with `immutable`, no `-wal` or `-shm` created |
| Network path | Create and open refused |
| Migration | Backup first, then the new version |
| `add_file`: crash after the copy, after the journal commit, after the rename (simulated) | After the next open: no row without a file, no file without a row, `files/.tmp/` empty |
| Source file changes during the copy | Error, nothing added |
| Same file added twice; the first copy in the trash | Reported as a duplicate by hash, with `inTrash` |
| Each v1 format, including a PSD | Dimensions, a thumbnail and a palette |
| Undecodable file | Added with a placeholder |
| Import of a folder with hidden and system files | They are skipped; nested folders are walked |
| `preview_import` of a folder with images, text and a subfolder | Totals and tree; the picked folder is the root; nothing copied |
| Reviewed import: exclude a subfolder, include a text file | Only the confirmed files are added; excluded folder not created |
| Duplicates in one batch | Asked once; skip, add anyway or cancel holds for the rest |
| Crash during an import, then open | No row without a file; previews not made yet are made; adding again reports duplicates |
| Scopes All, Uncategorized, Untagged, Trash | One query each; trashed items only in Trash; tag strings shared |
| An item whose stored path points outside the library folder | The original is refused, nothing is read |
| Folder delete | Subfolders gone, items stay, items without folders are Uncategorized |
| Tag rename into an existing tag | Merged, no duplicate rows |
| Search syntax `a OR b`, `-a`, parentheses, Cyrillic | Correct FTS5 query and results |
| Trash, restore, empty | Folders and tags restored; files removed on empty |
| Rename an item, also with a crash before or after the row commits | The original keeps its bytes under the new name; after the next open exactly one name is left |
| Back up, change things, restore the backup | Items, folders and fields as they were; the lock still held; a backup of the state before; an older backup migrated |
| Check Library after deleting an original and adding a stray file | Both reported; repair removes only what was asked |
| Undo after rating, renaming, trashing a selection | Each item back to its own earlier value, one edit per `Ctrl+Z` |
| Export preview: whole library, folders with/without subfolders, overlapping folders, tags | Counts once; unknown folder id refused; locked items in `hidden` |
| Export mirror of a folder tree | Files under `<root>/<folder>/…`, `egl-export.json`, a second run gets ` (2)` |
| Export name collision in one folder | `name (2).ext` |
| Export ZIP | Entries mirror the tree and read back; optional age passphrase decrypts to a ZIP; the encrypted export leaves no plaintext file and stops on cancel while it encrypts; a passphrase under 8 characters is refused; Decrypt Archive opens it again and refuses a wrong passphrase, a damaged or a foreign file without leaving anything (D-110) |
| Export with a missing original | Listed in `missing`; other files and the manifest still written |
| Export cancel; destination inside the library; encrypt without ZIP | Partial output removed; refused |
| `load_eagle_export` of the whole library | Folders top-down (parent before child); items, tag groups and smart folders present |
| `store_eagle_ids` then `load_eagle_export` again | Every stored folder and item carries its Eagle id; an imported row keeps its origin `eagle_id` (D-087) |
| `load_eagle_export` of one folder | Only that folder's subtree |

Settings (`app-store`), with temporary folders:

| Case | Expect |
|---|---|
| Two processes update recent libraries at the same time | Both entries kept |
| More than 20 recent libraries | The oldest is dropped |
| A recent library that is gone | Listed as missing, not removed |
| Default location with `Pictures` inside OneDrive | `%USERPROFILE%\egl-library`, outside OneDrive |
| Default location changed to a folder inside OneDrive or on a network path | Refused; the old default stays |

Eagle import (`eagle-folder`), with `fixtures/eagle-library/`:

| Case | Expect |
|---|---|
| Fixture library | Every item, folder (same order), tag, tag group and smart folder imported |
| A truncated `metadata.json` (Eagle writing in place) | Retried, then skipped with a report line |
| Locked folder | Imported locked with no password and no identity; files stay plaintext; the password string is not kept anywhere |
| Interrupted import, run again | Finishes without duplicates |

Running Eagle (`eagle-api`), unit tests and HTTP tests with `wiremock` and `fixtures/eagle-api/`:

| Case | Expect |
|---|---|
| Query plan: two tags, mode Any | Does not put both names in `item/get.tags` |
| Query plan: Uncategorized | Body has `isUnfiled: true` and no `folders` key |
| Query plan: normal folder | Body has no `isUnfiled` key at all |
| Query plan: folder with descendants | No multi-id `folders` key; the union is computed locally |
| Tag mode change on the same scope | Same `FetchPlan`, no network call |
| `FolderUpdateBody` for a nested folder | Always contains `parent` and the current `iconColor` |
| `FolderUpdateBody` for a root folder or no colour | `parent` / `iconColor` keys absent, never `null` |
| `FolderDto` with a non-empty `password` | `is_locked == true`, the password string is not kept anywhere |
| Serialized request body with Cyrillic | Pure ASCII (`\uXXXX`) |
| POST request | `token` in the body, not in the query |
| `item/add` body from another computer | `base64` starts with `data:image/` |
| `add_files` with host `127.0.0.1` | `item/add` body has `items` with `path`, no `base64` |
| HTTP 500 with `message`, or with a string in `data` | `Rejected` with that text |
| `success` with `data: false` | `NotFound` |
| `page_all` when `total` changes mid-loop | Restarts and deduplicates |
| Nested page | Parsed from `body.data.data` |
| Same-PC thumbnail path outside the library folder | Refused, nothing is read |
| `tagGroup/create`, `smartFolder/create`, `item/addComment` | Return the new id (or succeed); bodies match the captures |

Desktop UI tests (in `crates/gpui-desktop`): the Welcome screen, the read-only banner, the menu and shortcuts built from the command registry (no items of later features, write commands disabled in a read-only library), the Settings and About dialogs, Connect error copy, the grid window loading, selection (click, Ctrl, Shift, arrow keys, Ctrl+A), sort, the list, the preview, `views-invalidated`, dialogs and empty states.

Manual checks on Windows:

- Create a library, add a folder of mixed images, browse, tag, search, trash, restore.
- Import a copy of a real Eagle library; compare folder counts with the desktop app.
- Against a live Eagle (a separate test library, never the user's real one without asking): wrong token → 401 copy; folder counts with subfolders off and on; add a JPEG and see it in the desktop app; rename a tag and see it in the desktop inspector.
- Export a native library into an empty Eagle test library (Export… → Into a running Eagle): folders, tags and ratings match; a second export after edits updates instead of duplicating.

Already verified live on Eagle 4.0.0 on 2026-10-27 (captures in `fixtures/eagle-api/`): data URL `item/add`, asynchronous import, ignored `star`, the error model, the UTF-8 split, folder updates with partial and full bodies, cycle handling, folder counts; on 2026-10-28 `item/add` with `path` in batch and single mode. Checked in code: Eagle's library cache, smart folder rules (arch §16), the `.eaglepack` format (arch §17). Still unverified: locked folders (`password`, `isUnLock`, `folder/unlock`), non-empty tag groups and smart folders, the silent failure of raw base64.

---

## 11. Pitfalls (do not "fix" these by guessing)

General:

- Tags are case-sensitive. Never lowercase them for matching or storage.
- Folder membership is direct. Subfolders are a union, computed in the core; a parent does not contain its children's items.
- Folder tags are not item tags; they reach an item only when a file is added into the folder.
- Never send a whole item list to the UI. Serve windows of a view.
- Never animate a re-sort or a filter change of the grid, and never let the window chrome reload, print or zoom the page.
- Menu items and shortcuts come only from the command registry; never bind a key inside a component.
- Never copy or sync a library folder while it is open; use the backups.
- Never create or write a library on a network path or in a cloud-synced folder: WAL needs a local disk, and sync tools copy a live database mid-write.
- `item_fts` is keyed by `item.seq`, an explicit `INTEGER PRIMARY KEY`. Implicit rowids change on `VACUUM` and in backups.
- A read-only instance never cleans `files/.tmp/` and never recovers `pending_file`: the writer may be using them.
- The lock on `library.lock` is mandatory on Windows, so nobody else can read the file; the holder is in `meta.open_by`. Never delete `library.lock`.
- File bytes never leave the Rust side; Rust reads files by path.

Eagle:

- Nested page JSON is `body.data.data`, not `body.data` as the array.
- `item/get.folders` and `item/get.tags` are intersection, while the desktop's default chip mode is union.
- `isUnfiled: false` and `isUntagged: false` do not mean "the opposite". Omit them.
- `extendTags` are not item tags.
- `item/update` replaces `tags` and `folders`. A partial array deletes the rest.
- Tag group edits do not edit items.
- `item/query` ignores the open folder unless the client filters afterward. `keywords` does not search tags.
- Server list order is not the sort. Sort after the full filtered set is in memory. v1 `/api/item/list` re-sorts by import date.
- The thumbnail endpoint returns a path string.
- `item/add` needs a `data:` URL. Raw base64 fails silently after a success response.
- `item/add` is asynchronous: the id exists before the item does. It ignores `star`, except for batch items with `path`.
- `item/add` with `path` works only when the client runs on the Eagle PC; from another computer send a data URL.
- EXIF values are in `rawMetas`, not at the top level of an item.
- Eagle writes `metadata.json` files in place: a reader of a `.library` folder can catch half-written JSON and must retry.
- Smart folder rules have quirks (unescaped `startWith`, lowercased regex patterns, `rating contain "none"` matching everything); see arch §16 before evaluating them locally.
- `folder/update` with a partial body moves a nested folder to the root and clears its colour; `parent: null` reorders root folders. Always send the full desired state, and omit empty values instead of sending `null`.
- `folder/update` ignores cycles silently.
- `folder/get` nodes have no `parent`. `library/info` lags behind, contains folder password fields, and names tag groups `tagsGroups`.
- Handler errors are mostly HTTP 500. "Not found" is often `success` with `false`.
- `item/countAll` includes trash.
- `item/get` returns items from locked folders; hide them in the client.
- `item/get {id}` ignores `fields`; `item/query` has no `fields`.
- Non-ASCII text in a request body can be corrupted at a chunk boundary unless it is `\uXXXX`-escaped.
- A token in the query of a POST is written to Eagle's log file.
- The header `Host: localhost:41595` bypasses the token, so the API is open to the whole LAN.
- Never write files in `Name.library`. The UI never calls Eagle and never holds the token.
- Same-PC file reads from an Eagle library (thumbnails) must stay inside the library folder.

---

## 12. Security

Two different locks. Logging into the app does not unlock a locked folder. Unlocking a folder does not log the user into the app.

### 12.1 What protects what

- **App login** (this section) is the gate before Welcome. It does not wrap age identities and does not decrypt files.
- **Locked folders** encrypt originals, thumbnails, and the sensitive item columns with age (D-018, [`age-locked-folders.md`](age-locked-folders.md)). Each folder has its own identity.
- **Password** of a folder wraps that identity with Argon2id.
- **Recovery secret**, shown once at lock creation, is a second wrap of the same identity.
- **YubiKey PIV** is a third wrap of that same identity (`age-plugin-yubikey`, a retired PIV slot 82–95 with an ECDSA P-256 key). In that flow the user either touches the key or enters the PIN. The PIN replaces the touch. It is not an extra step, and it is not app login. Unlocking one folder does not unlock the others. Without the key, the password and the recovery secret still work.
- **Eagle token** stays in Windows Credential Manager. The database as a whole, settings, and unlocked folders stay plaintext.

### 12.2 App login

The user picks exactly one login method. The app never combines them and never requires two.

1. **Password.** A login password, separate from every folder password. The app stores an Argon2id hash of it, not the password. It is not a key for age.
2. **PIN.** A numeric login PIN. The app stores an Argon2id hash of it. For the PIV folder unlock, this same choice replaces the touch: the user enters the PIN instead of touching the key, not after it.
3. **WebAuthn.** The app uses the system WebAuthn API from Rust (Windows WebAuthn via `webauthn.dll`, through a small crate that contains unsafe). Login is a YubiKey 5C touch: the credential is created at enrollment and asserted at login. The app stores only the credential id and public key. WebAuthn is only for app login. It does not use the PIV slot and does not unwrap folder identities. Choosing WebAuthn means login does not ask for a PIN.

With no method enabled, the app opens straight to Welcome. Switching methods replaces the previous login and does not re-encrypt libraries. The app does not keep a recovery secret for login. A folder password and a folder recovery secret never satisfy the login gate.
