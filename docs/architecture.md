# egl-viewer: stack and architecture (Tauri 2, Rust, React)

Companion to [`spec.md`](spec.md). The spec describes behaviour. This document describes **how the app is built**, and records the corrections to the original spec that came out of reverse engineering Eagle.

The app is a Tauri 2 application with a Rust core and a React UI. **It is a standalone asset manager with its own library format** (§15.3); compatibility with Eagle (import, a live connection to a running Eagle, export) is a secondary feature (D-013). **The current priority is the Windows desktop app, without AI features.** macOS, Android and iOS stay possible: the core and the UI are portable, and platform code is kept in one place, but those targets are not part of the current scope.

The core talks to every kind of library through one `LibrarySource` trait (§15.2): the native library, an Eagle library folder (read only, for the import) and a running Eagle. Sections 4, 5, 7.2 and 12 describe the Eagle parts; everything else applies to every source, and the native library is the one built first.

The spec and this document were rewritten together for this stack on 2026-10-28, and re-prioritised for the standalone app on the same day. If you find a conflict, this document wins and the spec must be fixed.

Verification source: Eagle 4.0.0 (build 20260401). `app.asar` was analysed with REA (`rea analyze-javascript-application`, `rea extract-artifact`). References like `app/js/plugin/index.js:1546` point to files inside `resources/app.asar`.

Live verification (2026-10-27): two test JPEGs were added to the open library through `item/add` with a data URL, the API responses were captured, and both items were then moved to the trash. Two test folders were created to check `folder/get` and `folder/update`. The mark **[live]** means the finding was confirmed on a running Eagle, not only in code. The captures are in `fixtures/eagle-api/` at the repository root (the `request` field is what was sent, the `response` field is the HTTP status and body). Use them as fixtures for HTTP mock tests.

---

## 0. Decisions at a glance

| Question | Decision |
|---|---|
| Product | **A standalone asset manager** with its own library; Eagle compatibility is secondary (D-013) |
| Targets | **Windows 10/11 now.** macOS, Android, iOS later; nothing in the core or the UI may depend on Windows |
| Shell | **Tauri 2.12** (stable). Not Tauri 3, which is in alpha (§2) |
| Core | Rust workspace: `library-core` (no IO), `library-native` (the native library), `media` (thumbnails, palettes, hashes), `app-store` (settings, recent libraries, secrets), and for Eagle `eagle-folder` and `eagle-api`; the Tauri app crate is a thin layer of commands |
| Library sources | One `LibrarySource` trait in `library-core`. Implementations: the native library, an Eagle library folder on disk (read only), a running Eagle (API) (§15.2) |
| Library format | A folder `Name.eglib` (D-016): SQLite (`library.db`) as the single source of truth, the original files copied in (D-014), regenerable thumbnails; documented in `library-format.md` and versioned (§15.3) |
| Media | v1 previews for JPEG, PNG, GIF, WebP, SVG, TIFF, BMP and PSD composites; AVIF and other files are stored with a placeholder (D-015, D-035, §15.4) |
| UI | React + TypeScript (strict) + Vite, Tailwind + shadcn/ui; TanStack Virtual for the grid, TanStack Query for data from Rust, TanStack Router for screens. The first screen is Welcome (D-017). The native window frame with the app's own menu bar, built from one command registry (§8) |
| Types across IPC | `ts-rs` generates TypeScript from the Rust structs that cross IPC |
| Where filtering happens | The source loads the **scope** (a folder, Uncategorized, a smart folder): one SQL query in the native library, API pages for a running Eagle. Tags, type, rating, shape, sort and search are combined **in Rust** (§6) |
| Grid data | Rust keeps the sorted result as a **view**; the UI virtualizes and asks for windows of cells (§6.5). Whole lists never cross IPC |
| Writes | A change is an intent (add a tag, remove from a folder), applied to the current state of the item: in one SQLite transaction for the native library; for Eagle, Rust re-reads the item right before sending and builds the full array from fresh data (§7) |
| Adding files | Native library: copied into the library through the media pipeline. Running Eagle: `item/add` with the file **path** on the same PC, a data URL from another PC (§7.4) |
| Thumbnails | A custom `thumb` URI scheme served by Rust. Native library: generated WebP files in the library. Running Eagle: same-PC files or a helper plugin in Eagle (§11) |
| Who talks to Eagle | **Only Rust.** The UI never calls the Eagle API and never sees the token |
| HTTP | Only for Eagle: `reqwest` with rustls on `tokio`, our own `EagleHttp` with all protocol rules (§4) |
| Secrets | Only the Eagle token, in Windows Credential Manager (`keyring`); later the macOS/iOS Keychain and Android Keystore |
| Settings | `app-store`: JSON files in the app's local data folder for recent libraries, the last query per library, preferences, the Eagle endpoint; not `tauri-plugin-store` (D-022) |
| Background | Imports and thumbnail generation run as tasks of the open app. No background service; a running Eagle is refreshed when the window regains focus and on demand |
| Sync | Native: one writer per library (an OS file lock), local disks only (no cloud folders, no network shares), backups; sync between devices later (§15.8). Eagle: only Eagle writes its library; every change goes through the API (§12) |
| Eagle compatibility | Import of `.library` folders without Eagle running; live work through the API; export into Eagle through its API. The app never writes Eagle's files (§15.5) |
| AI | Out of scope: no `aiSearch` calls, no semantic or image search |

---

## 1. What reverse engineering Eagle revealed (corrections to the spec)

The "Spec said" column quotes the wording of the **first** version of the spec (Android-only, a client for a running Eagle). All of these points are already reflected in the current spec, mostly in its section 8 (Eagle compatibility), which uses the numbers as references ("correction 7"). Section and phase numbers inside the table refer to that first spec.

### 1.1 Critical: something breaks without these fixes

| # | Spec said | What actually happens | What the client does |
|---|---|---|---|
| 1 | §10: "try raw base64 first" | `base64` is handed to the downloader **as a URL** (`plugin/index.js:1546` → `uploadUrls`). A string without `data:` gets an `http://` prefix (`downloader/downloader.js:403`) and the download fails silently. **The API still answers `success` and returns an `id`**. **[live]** The data URL works. Raw base64 was not sent live, to avoid an import error on the desktop | Always send `data:<mime>;base64,<no line breaks>`. The decoder looks for `;base64,` and the MIME type (`global.js:906`) |
| 2 | §13 phase 7: "after add, call item/get?id=" | `item/add` is asynchronous: the id comes back at once and the file is queued for download and processing. **[live]** Right after the response, `item/get {id}` returned `total: 0`. A 16 KB JPEG appeared after ~0.3 s | Poll `item/get {id}` with a growing delay (0.25 → 0.5 → 1 → 2 → 4 s, about 20 s in total). While waiting, the grid shows an "Importing…" cell |
| 3 | §5 "Tree edits": `parent` and `iconColor` are optional | `folder.save` compares the sent value with the current one (`plugin/index.js:2495`, `:2499`). **If `parent` is not sent, a nested folder moves to the root. If `iconColor` is not sent, the colour is cleared.** A cycle or a non-existent parent is silently ignored, and the folder is returned unchanged. **[live]** `folder/update {id, name}` on a child folder moved it to the root and cleared its `red` colour (`folder-update.without-parent-and-color.json`). The same request with the full set of fields put the folder back under its parent with its colour, and a rename with the full set of fields left it in place. Moving the parent under its own descendant returned HTTP 200 `success`, and the tree did not change (`folder-update.cycle.json`) | Always send the **desired** `parent` and `iconColor`. "At the root" and "no colour" mean the field is omitted (`skip_serializing_if = "Option::is_none"`). Check cycles in the client (§7.3) |
| 4 | §3 JSend: "a handler failure is usually HTTP 200 + fail" | v2 handlers throw a plain `Error`, which produces **HTTP 500 + `status: "error"`** with the text in `message` (`jsend_response.js:83`). v1 handlers reject the promise with a string: then the text is in `data` and `message` is empty (for example `folder/unlock`). "Not found" is a **success**: `item/update` → `data: false`, `folder/update` → no `data`. **[live]** `item/update` with a name longer than 10 000 characters → HTTP 500, `status: "error"`, `message: "name too long (max 10000)"`. Unknown id → HTTP 200, `status: "success"`, `data: false` | Error model in §4.4. The message is `message`, or `data` if it is a string |
| 5 | §3 Auth: "the token is the protection" | The Host check can be bypassed: a LAN request **without a token** but with the header `Host: localhost:41595` is accepted. Verified live on this machine (`sever_proc.js:89`). The token protects nothing: anyone on the network can call the API, including `/api/script/inject` | The client never uses the bypass and always sends the token. The Connect screen advises limiting the firewall rule for 41595 to the Private profile and to the subnet or the client's IP (§10) |
| 6 | §3: "POST: token in the body, query allowed" | For every POST, Eagle writes `request.url` into its log file (`sever_proc.js:107`). A token in the query ends up in a log on disk | POST: token **only in the body**. GET: in the query, there is no other way (only `console.log` there) |
| 7 | Not mentioned | The request body is concatenated as `content += chunk` (`sever_proc.js:189`). If a multibyte UTF-8 character (Cyrillic in tags and names) lands on a chunk boundary, it turns into `U+FFFD`. The risk is real for large bodies (`item/add` with a file and Cyrillic tags). **[live]** An `item/update` body with a Cyrillic note was sent as two TCP writes split inside the letter «ж»: two `�` characters were stored. The same body with `\uXXXX` arrived intact (`item-update.utf8-split.json`) | Escape every non-ASCII character in the JSON as `\uXXXX` before sending (§4.2). The body becomes pure ASCII and cannot break. **[live]** A Cyrillic tag sent this way was stored correctly |
| 8 | §5, Phase 2: tree from `library/info` | `library/info` reads **`metadata.json` from disk** (`plugin/index.js:2932`). It has no counts, no "unlocked" flag, and changes made through the API may not be written yet. **Folder passwords are stored there in base64** (`api-server.js:76` compares `btoa(password)`). The tag group key is `tagsGroups`, not `tagGroups` (verified live). If no library is open, the response is `{}`. **[live]** Right after `folder/create` the new folders were missing from `library/info`; they appeared after 2 s. `library/info` nodes contain `password` and `passwordTips` keys (empty strings when there is no password) and no counts. `folder/get` showed the new folders immediately, with counts and `extendTags`; its `images` and `imagesMappings` are empty, about 0.6 KB per folder. **`folder/get` nodes have no `parent` field** | Use `library/info` only for name, path and `modificationTime`. Take the tree from `folder/get` without arguments. Derive the parent from `children` nesting. Drop `password` at parse time (§5) |
| 9 | §5: "locked: show an icon" | `item/get` **ignores** locked folders (it reads `$bodyScope.raw`). The desktop hides items from locked folders and their descendants in All and every other view (`app.bundle.js:28847`) | The client computes `locked_ids` itself (folders with a password and without `isUnLock`, plus all descendants) and excludes their items in every scope (§6.4) |
| 10 | Not mentioned | Every API call runs in Eagle's renderer (`$bodyScope`, Angular), i.e. on the desktop UI thread. Every `item/get` page re-filters and re-sorts the whole library | At most 2 concurrent requests to the host. Load pages sequentially. The snapshot cache removes repeated requests (§6.3) |

### 1.2 Important, but nothing breaks

| # | Observation | Consequence |
|---|---|---|
| 11 | `item/add` with a data URL (single and batch) **ignores `star`**. In single mode `modificationTime` is always `Date.now()`. The source link is sent in the **`website`** field, not `url`. The name is sanitized: `%`, `&lt;` and `&gt;` are removed and the length is truncated. **[live]** `star: 3` and `star: 4` were not stored; the items have no `star` key at all. `website` was stored in the item's `url` field. Batch mode kept the given `modificationTime`. A follow-up `item/update {star: 3}` worked | If the user set a rating when adding through a data URL, send `item/update {star}` after the item appears. Batch items with `path` keep it (correction 35) |
| 12 | When adding into a folder, the folder's `extendTags` are automatically added to the item's tags (`app.bundle.js:30736`) | After import, show the tags from the server, not from the form |
| 13 | In single mode `item/add` does not validate folder ids (batch mode does) | Check `folders` against the tree before sending |
| 14 | `item/get {id}` returns the **whole** item: `fields` is ignored (`:2284`). **[live]** Confirmed. The page shape is the usual one: `{data:[item], total:1, offset:0, limit:50}` | Good for re-reading before a write |
| 15 | `isUnfiled` matches only `folders.length === 0`. Items without a `folders` key are not returned. **[live]** Items added through the API without a folder get `folders: []` and are returned | Local Uncategorized check: `folders` missing or empty. Small differences from the server are possible |
| 16 | `item/query` **does not support `fields`** and does not sort: it returns full objects (`:2252`). **[live]** Confirmed | Start the search at 2 characters with a 300 ms debounce. Take only `id` from the response |
| 17 | `smartFolder/getItems` accepts `fields` only as an array (i.e. via POST) and can sort on the server with `orderBy` (`smart-folder-handlers.js:227`) | POST only |
| 18 | `addComment` requires either a rect or a `duration` (`:1770`). A comment without a rect is impossible | Comments are read-only. Alternatively, a rect covering the whole image, if that meaning is acceptable |
| 19 | v2 `library/icon` returns a `Buffer` serialized to JSON (`{"type":"Buffer","data":[…]}`). v1 `/api/library/icon` returns raw bytes without `Content-Type` | Use v1 and detect the format by its signature |
| 20 | Unlocking through v1 `folder/unlock` sets `isUnLock = true` **in the desktop's memory**: the folder also opens on the PC until a restart. The password is compared through `btoa()`, so a password with non-Latin-1 characters causes a 500 | Warn in the unlock dialog |
| 21 | The Node HTTP server (Electron 22 / Node 16) closes keep-alive connections after 5 s | `pool_idle_timeout(4 s)` in `reqwest`; otherwise errors on a reused, already closed socket are possible |
| 22 | Eagle has no mDNS or Bonjour | The address is entered by hand (`127.0.0.1` is offered when Eagle runs on the same PC). Optional later: scan the /24 with `GET /api/v2/app/info`. A 401 response mentioning "Preferences > Developer" identifies Eagle unambiguously |
| 23 | The sort keys `mtime`, `btime`, `duration` were only in the spec's detail field list | Add them to the list fields, otherwise local sorting does not work |
| 24 | §8 step 7: "in-memory pager of 60" | Rust keeps the sorted list as a view; the UI virtualizes the grid and asks for windows (§6.5) |
| 25 | **[live]** `item/countAll` **includes trashed items**: after both test items were moved to the trash it returned 2, while `item/get` returned `total: 0` | Use `countAll` only as a rough library size (the `LARGE_LIBRARY` threshold). Numbers shown in the UI come from the view's `total` |
| 26 | **[live]** `tag/get` returns `{name, imageCount, groups, pinyin}` objects. `imageCount` excludes the trash: after the move to the trash the tags disappeared from the vocabulary | Show `imageCount` next to each tag in the sheet. Ignore `pinyin` |
| 27 | **[live]** `tag/getRecentTags` is not updated by API writes: after `item/add` with tags it stayed empty | Hide the Recent block when it is empty. Keep the client's own recent tags locally |
| 28 | **[live]** `item/countAll` and `item/getComments` are not paginated: `data` is the number or the array directly | In `api_v2` these methods parse `data` directly, without `Page<T>` |
| 29 | **[live]** Both test images (640×400 and 320×320) have `noThumbnail: true`: Eagle does not create `_thumbnail.png` for small images | Thumbnails (§11) must fall back to the original file when `noThumbnail` is set, or small images stay without a preview |
| 30 | **[live]** Images have no `duration` key. `mtime` and `btime` equal the time the PC wrote the file during import, not the dates of the source file. `isDeleted: false` is present explicitly | "Date created" and "Date modified" sorts for files added through the API match "Date added". This is expected |
| 31 | **[live]** An explicit `parent: null` on a root folder **moves it to the end of the root list**: the order `[parent, child]` became `[child, parent]`. Omitting the field keeps the order | Never send `null`. `skip_serializing_if` does this automatically (§7.2) |
| 32 | **[live]** Counts in `folder/get`: a child folder with one item has `imageCount 1 / descendantImageCount 1`, its parent `0 / 1`. `descendantImageCount` includes the folder's own items. Right after `folder/create` the node has no counts yet: the desktop computes them later. `item/get {folders:[parent]}` returned 0, so membership in a child folder is not inherited | In the tree show `imageCount` when the Subfolders switch is off and `descendantImageCount` when it is on. Show a missing count as "—", not 0 |
| 33 | **[live]** `folder/create` and `folder/update` return the desktop's internal objects with service keys (`$$hashKey`, `vstype`, `styles`, `covers`, `index`, and so on). They have no `parent` field | Take only `id` and what you sent from the response. Re-read the tree with `folder/get` after an edit |
| 34 | **[live]** The parent folder's tags immediately appeared in `extendTags` of the parent and the child. The tags of an item already in the folder did not change | Show `extendTags` as a hint and never write them. They reach an item's tags only on `item/add` (correction 12) |
| 35 | **[live]** `item/add` with `path` on the same PC. **Batch** items keep `tags` (Cyrillic too), `folders` (unknown folder ids are dropped), `annotation`, `website` (as `url`), `modificationTime` and **`star`**. **Single** mode keeps tags, folders, annotation and website, but ignores `star` and `modificationTime`. Eagle copies the file and leaves the source in place. The items were visible within milliseconds (`item-add.path-batch.json`, `item-add.path-single.json`) | On the same PC always use the batch form, even for one file. No follow-up `item/update {star}` is needed |
| 36 | Outside the `.library` folder Eagle keeps a cache per library: `%APPDATA%\Eagle\library-caches\<id>.txt`, one item JSON per line with `lastModified`. `<id>` is an FNV-1a 32-bit hash of the library path (`background.js:2309`; verified on this PC: `c7f437ad` for the open library). `%APPDATA%\Eagle\eagle-backup\<id>.json` holds a copy of `metadata.json`. On start Eagle loads the cache, then re-reads only new ids and items changed after the cache file (`:2324`) | Never read or write it: it exists only on the PC that opened the library and may be stale. Deleting it only costs Eagle a full rescan |
| 37 | Eagle writes item `metadata.json` files and the library `metadata.json` **in place**: it truncates the file, then writes (`background.js:5598`, `:5409`). Only `mtime.json` goes through a temp file and a move (`:3166`) | A reader can see an empty or half-written JSON file. On a parse error, retry after a short pause (3 × 100 ms) and skip the item for this pass if it still fails |
| 38 | EXIF data is not at the top level of an item: it lives in `rawMetas` (`camera`, `isoSpeed`, `aperture`, `focalLength`, `shutter`, `timestamp`, all strings). `bpm` is top-level. The first spec listed `camera`, `iso`, … as item fields | Request `rawMetas` and `bpm` in the detail fields |
| 39 | Items can carry `order: { "<folderId>": number }`: the item's manual position inside that folder, used by the desktop's manual sort and by `.eaglepack` import (`background.js:5034`) | The "Manual" sort orders by `order[folderId]` ascending, falling back to `modificationTime`. Request `order` in the list fields when a folder uses manual sort |
| 40 | `item/get` filters go through the smart folder rules (§16). `ext` uses the `type` rule, so categories work (`video`, `audio`, `font`, `word`, `excel`, `powerpoint`, `presentation`); `rating: 0` matches unrated items | Type buckets can be server prefilters for large libraries (§6.2) |
| 41 | Eagle detects duplicates by a metadata fingerprint (size plus dimensions, duration or font ids, depending on the type), not by file content (`app.bundle.js:2127`) | The native library uses a BLAKE3 content hash (§15.4); the two may disagree on what a duplicate is |

---

## 2. Stack

| Concern | Choice | Note |
|---|---|---|
| Shell | `tauri` 2.12.x, `tauri-build`, `@tauri-apps/cli`, `@tauri-apps/api` | Pin exact versions. WebView2 on Windows |
| Async | `tokio` | The runtime Tauri already uses |
| Storage | `rusqlite` (`bundled`, FTS5 enabled), `ulid` | The native library (§15.3); on a blocking thread pool |
| Media | `blake3`, `image`, `fast_image_resize`, `resvg`, `psd`, `kamadak-exif`, `webp` (libwebp), `rayon` | The media pipeline (§15.4, D-033) |
| Passwords | `argon2`, `age`, `secrecy` | Each locked folder wraps its own age identity with Argon2id; files and sensitive columns are age ciphertext (D-018) |
| HTTP | `reqwest` (`rustls-tls`, default features off) | Only for Eagle; one client per endpoint |
| Serialization | `serde`, `serde_json`, `ciborium` (Eagle offline cache, later), `base64` | |
| Errors and logs | `thiserror`, `tracing`, `tracing-subscriber` | Logs never contain the token or raw folder bodies |
| Secrets | `keyring` (Windows Credential Manager), `secrecy` for the Eagle token in memory | Later: macOS/iOS Keychain, Android Keystore |
| Collation | `icu_collator` (ICU4X) with numeric ordering | Pure Rust, identical on every platform |
| Files | `tauri-plugin-dialog` (file and folder pickers), Tauri drag-and-drop events (file paths) | |
| Settings | `app-store` (own JSON files, atomic replace) | Recent libraries, the last query per library, preferences, the Eagle endpoint; several app processes share them (D-022). `tauri-plugin-store` is not used |
| Links | `tauri-plugin-opener` | Opens an item's `url` in the system browser (limited to `https` and `http`), an original in its default app, and the library folder in Explorer |
| Types across IPC | `ts-rs` | Generated files are committed and checked in CI |
| UI | React, TypeScript (strict), Vite, `@tanstack/react-virtual`, `@tanstack/react-query`, `@tanstack/react-router` | |
| UI kit | Tailwind CSS + shadcn/ui on Radix primitives (D-006) | Accessible dialogs, menus, sheets |
| Tests | `cargo test`, `wiremock`, Vitest + Testing Library + `mockIPC`, optional `tauri-driver` | §13 |
| Tooling | `rustup` (toolchain pinned in `rust-toolchain.toml`), pnpm, GitHub Actions | |
| Lint and format | rustfmt and clippy for Rust; Biome for TypeScript, CSS and JSON | ESLint is not used: typescript-eslint does not support TypeScript 7 yet (decisions.md, D-005) |

**Why this stack.** The library engine (storage, the media pipeline, the query engine) and the Eagle protocol rules (§1) are the hard part, and they live in pure Rust crates that `cargo test` covers without a window or a device. The UI uses the most common web stack, with ready-made solutions for the hard UI problems (a virtualized grid over tens of thousands of items, caching, routing). Tauri on Windows uses the WebView2 that is already in the system, so the installer is small.

**Why Tauri 2.12 and not Tauri 3.** As of 2026-10-28 the latest stable release is `tauri` 2.12.5 (2026-10-21), and Tauri 3 is at `3.0.0-alpha.5` (2026-10-14). The alphas break APIs between releases: alpha.3, for example, changed plugin hook signatures and removed `run_on_main_thread` from windows. Tauri 3 also adds an optional CEF (bundled Chromium) runtime. The Tauri layer of this app is thin (§3), so moving to 3.0 after its stable release is a contained change.

---

## 3. Workspace and crates

```
egl-viewer/
  Cargo.toml                 workspace, pinned versions
  crates/
    library-core/            no IO, no async runtime
      model/                 Item, ItemLite, Folder, FolderIndex, Tag, TagGroup, SmartFolder, LibraryInfo
      query/                 LibraryQuery, Scope, TagMode, SortKey; filter.rs (FolderIndex, lock filter, matchers),
                             sort.rs (collation keys, random), view.rs (View, Views, ItemCell, ViewSource)
      edit/                  ItemEdit, cycle guard
      source.rs              the LibrarySource trait, Capabilities, SourceError (§15.2)
    library-native/          the native library: SQLite + files + thumbnails (§15.3); no Tauri
      schema.rs, schema/     format version, schema v1, migrations; docs/library-format.md describes it
      create.rs, name.rs     creating a library in a temporary folder; library names
      library.rs, lock.rs    open, close, read-only modes; library.lock (OS file lock)
      recover.rs, backup.rs  pending_file recovery, files/.tmp; backups
      scope.rs, source.rs    load_scope in SQL, the full item, tags, thumbnail and original bytes; NativeSource
                             (FTS5 search in phase 6)
      write.rs               item edits, folders, tags, trash, change_log
      add.rs                 adding one file: temp copy, hash, duplicates, the pending_file journal, rows
      import.rs, preview.rs  batches (folders walked, one duplicate question, cancel, progress); thumbnails, counts
    media/                   decoding, thumbnails, palettes, EXIF (§15.4); no Tauri
      decode.rs, thumbnail.rs, palette.rs, metadata.rs; fixtures.rs makes test files on the fly (D-034)
    app-store/               file IO; no Tauri
      settings.rs            the Eagle endpoint
      preferences.rs         preferences, layout, window state; json_file.rs: lock + atomic replace (D-022)
      recent.rs              recent libraries, the last library, "closed explicitly"
      location.rs            default library folder; sync folder and network path checks (Windows code behind a trait)
      secrets.rs             SecretStore trait + keyring implementation (the Eagle token)
      offline.rs             (later) offline index of the Eagle source (CBOR), pinned thumbnails, outbox
    eagle-folder/            an Eagle `.library` folder on disk, read only: the import (§15.6)
    eagle-api/               a running Eagle as a LibrarySource; depends on library-core; no Tauri
      transport.rs           EagleHttp, escape_non_ascii, parse_jsend, page_all
      error.rs               EagleError, NetworkError
      dto/                   ItemDto, FolderDto (passwords dropped here), TagDto, … → model
      plan.rs                plan() → FetchPlan, ServerFilter: the Eagle-specific query plan (§6.2)
      api_v2.rs              typed methods per route, FolderUpdateBody
  src-tauri/                 the app crate: thin
    commands.rs              libraries, imports, views and items now; folders, tags, eagle later (§8)
    session.rs               the open library, its imports and views, the app's files, without Tauri (tested)
    views.rs                 view IPC types, the source read on a blocking thread, refreshing views after
                             writes, parsing `thumb` URLs (the scheme itself is registered in lib.rs)
    window.rs                window size and position, showing the window after the first paint
    capabilities/            Tauri permissions: only our commands and the plugins in use
  ui/
    src/ipc/                 generated types (ts-rs) + typed invoke wrappers
    src/commands/            the command registry: menu bar, shortcuts, the Keyboard Shortcuts dialog (§8)
    src/routes/              (when routing arrives, D-010) welcome, library, item/$id, eagle/connect, eagle/import, eagle/export
    src/features/            welcome, library, browse (grid, list, selection, details, preview), imports, connect;
                             later tree, tag-sheet, filter-sheet, folder-dialogs
    src/components/ui/       shadcn/ui components
    src/lib/                 thumbUrl(), platform helpers
```

Rules:

- `library-core` has no IO and no `tokio`: only data and pure functions. Every rule that is easy to get wrong (union and intersection, tag modes, cycles, full arrays) lives here, together with the `LibrarySource` trait (§15.2).
- `library-native` and `media` know the native format and nothing about Tauri, so their tests run on temporary folders.
- `eagle-api` knows the Eagle protocol and nothing about Tauri, so its tests run against `wiremock` and, optionally, a live Eagle. The query plan (`plan()`, `ServerFilter`) is Eagle-specific and lives here.
- Crates appear with their phases (spec §9): `library-native` and `media` first, `eagle-folder` with the import, `eagle-api` grows from its phase-0 stub with the live connection.
- Commands and the UI only know `LibrarySource`. Which library is open (a native library, or a running Eagle) is a runtime choice (§15).
- The app crate only wires things together: commands parse input, call the core and the API, and return results. No business logic in commands.
- The UI only calls the typed wrappers in `src/ipc/`. It never builds Eagle URLs, never sees the token, and never holds a full item list.
- Windows-specific code (credential store, Explorer integration, same-PC file access to an Eagle library) sits behind small traits so that macOS and mobile can be added later without touching the core or the UI.

---

## 4. Eagle transport (`eagle-api`)

This section and §5 apply only to the running-Eagle source (spec §8.3). The native library has no network layer.

### 4.1 Transport

```rust
pub struct EagleHttp {
    client: reqwest::Client,
    endpoint: Endpoint,
    permits: tokio::sync::Semaphore,         // 2 permits: Eagle runs every call on its UI thread
}

pub struct Endpoint {
    pub base: url::Url,                      // http://host:41595/
    pub token: secrecy::SecretString,        // never printed
}

impl EagleHttp {
    pub async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, EagleError> {
        let _permit = self.permits.acquire().await.expect("semaphore is never closed");
        let resp = self.client
            .get(self.endpoint.base.join(path)?)
            .query(query)
            .query(&[("token", self.endpoint.token.expose_secret())])   // GET: query only
            .send()
            .await?;
        parse_jsend(path, resp).await
    }

    pub async fn post(&self, path: &str, mut body: Map<String, Value>) -> Result<Value, EagleError> {
        body.insert("token".into(), Value::from(self.endpoint.token.expose_secret()));  // POST: body only
        let text = escape_non_ascii(&serde_json::to_string(&body)?).into_owned();
        let _permit = self.permits.acquire().await.expect("semaphore is never closed");
        let resp = self.client
            .post(self.endpoint.base.join(path)?)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(text)
            .send()
            .await?;
        parse_jsend(path, resp).await
    }
}
```

The typed layer on top:

```rust
impl EagleApiV2 {
    pub async fn app_info(&self) -> Result<AppInfo, EagleError> {
        decode(self.http.get("api/v2/app/info", &[]).await?)
    }
    pub async fn item_page(&self, filter: &ServerFilter, fields: &[&str], offset: usize) -> Result<Page<ItemDto>, EagleError> {
        let mut body = to_map(filter)?;
        body.insert("fields".into(), json!(fields));
        body.insert("offset".into(), json!(offset));
        body.insert("limit".into(), json!(1000));
        decode(self.http.post("api/v2/item/get", body).await?)
    }
    pub async fn item_by_id(&self, id: &str) -> Result<Option<ItemDto>, EagleError> {
        let page: Page<ItemDto> = decode(self.http.post("api/v2/item/get", map! { "id" => id }).await?)?;
        Ok(page.data.into_iter().next())
    }
    // …
}
```

The permit is held until the response is parsed, so at most two requests are in flight at any moment.

### 4.2 ASCII escaping (correction 7)

```rust
use std::{borrow::Cow, fmt::Write};

/// In serialized JSON, non-ASCII characters occur only inside strings, so the result is always valid JSON.
pub fn escape_non_ascii(json: &str) -> Cow<'_, str> {
    if json.is_ascii() {
        return Cow::Borrowed(json);
    }
    let mut out = String::with_capacity(json.len() + 16);
    for c in json.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units).iter() {
                write!(out, "\\u{unit:04x}").unwrap();
            }
        }
    }
    Cow::Owned(out)
}
```

Characters outside the BMP become two `\uXXXX` escapes (a surrogate pair). This is valid JSON, and `JSON.parse` in Node joins them back together.

### 4.3 Client settings

```rust
reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(if over_tailscale { 8 } else { 3 }))  // a connect timeout almost always means the firewall
    .timeout(Duration::from_secs(60))             // the whole request, including a 1000-item page
    .pool_idle_timeout(Duration::from_secs(4))    // shorter than Node's 5 s keep-alive (correction 21)
    .pool_max_idle_per_host(2)
    .use_rustls_tls()
    .build()?
```

The client is rebuilt when the endpoint changes. `over_tailscale` is true for addresses in `100.64.0.0/10` and for `*.ts.net` names.

### 4.4 Error model

```rust
#[derive(Debug, thiserror::Error, Serialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum EagleError {
    #[error("not connected")]           NotConnected,
    #[error("unauthorized")]            Unauthorized,                        // HTTP 401 → Connect screen
    #[error("Eagle is older than 4.0")] ApiTooOld,                           // 404 on /api/v2/app/info
    #[error("no library open")]         NoLibraryOpen,                       // library/info without folders
    #[error("{message}")]               Rejected { http: u16, message: String }, // fail/error with 200 or 500
    #[error("not found")]               NotFound,                            // success, but data false or missing
    #[error("bad request")]             BadRequest,                          // Eagle could not parse our JSON (400)
    #[error("invalid input: {0}")]      Invalid(String),                     // rejected in Rust before any request
    #[error("network: {0}")]            Network(NetworkError),
}

#[derive(Debug, thiserror::Error, Serialize, TS)]
pub enum NetworkError {
    #[error("connect timeout")]      ConnectTimeout,      // firewall
    #[error("connection refused")]   Refused,             // Eagle not running, or wrong port
    #[error("unreachable")]          Unreachable,         // wrong network
    #[error("local network denied")] LocalNetworkDenied,  // OS permission (later: Android 17+, iOS)
    #[error("{0}")]                  Other(String),
}
```

Mapping a `reqwest` error: a connect error that is also a timeout → `ConnectTimeout`; otherwise find the `std::io::Error` in the source chain and map `ConnectionRefused` → `Refused`, `HostUnreachable` and `NetworkUnreachable` → `Unreachable`, `PermissionDenied` → `LocalNetworkDenied`, `TimedOut` → `ConnectTimeout`; anything else → `Other`.

Parsing order in `parse_jsend()`:

1. Body is not JSON → `Rejected { http, "HTTP {code}" }`.
2. `status == "success"` → `data`.
3. HTTP 401 → `Unauthorized`. HTTP 404 → `ApiTooOld` only for the path `api/v2/app/info`, otherwise `Rejected`. HTTP 400 → `BadRequest`.
4. Otherwise `Rejected { http, message }`, where `message` is the JSend `message`, or `data` if it is a string, or `"HTTP {code}"`.

`NotFound` is decided by the typed layer, because only it knows that `false` means "not found". `EagleError` maps one to one onto the flat `SourceError` of `library-core` (§15.2, D-004). Commands return `Result<T, SourceError>`; the UI receives errors as `{ kind, detail }` and maps `kind` to the copy of the Connect screen and the error banner.

### 4.5 Pagination

```rust
pub async fn page_all<T, F, Fut>(
    mut fetch: F,
    id: impl Fn(&T) -> &str,
    progress: impl Fn(usize, usize),
) -> Result<Vec<T>, EagleError>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = Result<Page<T>, EagleError>>,
{
    for _attempt in 0..3 {
        let mut seen: IndexMap<String, T> = IndexMap::new();
        let mut offset = 0;
        let mut total = None;
        loop {
            let page = fetch(offset).await?;
            match total {
                None => total = Some(page.total),
                Some(t) if t != page.total => break,      // the library changed while loading: start over
                _ => {}
            }
            let received = page.data.len();
            for item in page.data {
                let key = id(&item).to_owned();
                seen.entry(key).or_insert(item);
            }
            offset += received;
            progress(seen.len(), page.total);
            if offset >= page.total || received == 0 {
                return Ok(seen.into_values().collect());
            }
        }
    }
    Err(EagleError::Rejected { http: 200, message: "library keeps changing".into() })
}
```

The response has the shape `body.data.data`. The server recomputes every page, so an `offset` shift from a concurrent import is quite possible: both deduplication and the `total` check are needed.

### 4.6 Platform specifics

**Windows (current target).**

- WebView2 is part of Windows 11 and of current Windows 10 installations. Use the installer's WebView2 bootstrapper option for machines without it.
- Installer: NSIS, per-user (no admin rights needed). Code signing is optional for personal use; without it Windows SmartScreen shows a warning on first launch.
- **Same PC as Eagle.** Connect to `127.0.0.1`. Eagle accepts localhost without a token (its `Host` check), but the client still sends the token, so the code path is the same as for a remote PC. On the same PC the client can also use paths that Eagle returns (`item/add` with `path`, thumbnail files, §7.4 and §11).
- **Another PC.** The Windows Firewall rule for port 41595 on the Eagle PC decides everything; the Connect screen explains it on a connect timeout.
- The token is stored in Windows Credential Manager through `keyring`.

**Later targets** (not in the current scope; listed so that nothing in the design blocks them):

- macOS: WKWebView, Keychain, signing and notarization.
- Android: `ACCESS_LOCAL_NETWORK` runtime permission when targeting SDK 37+, Keystore through a small Kotlin plugin, file input for camera and gallery, image preparation in the WebView. Requests from Rust sockets are not expected to go through Android's Java cleartext policy; verify on a device.
- iOS: a Mac with Xcode, `NSLocalNetworkUsageDescription` (the local network prompt also covers Rust sockets), `NSCameraUsageDescription`, Keychain. App Transport Security applies to Apple's URL loading APIs, not to sockets opened by Rust; verify on a device.

---

## 5. Eagle DTOs and the core models

Eagle's JSON is parsed into DTOs in `eagle-api` and converted into the models of `library-core`; the native library fills the same models from SQL rows (§15.3).

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    pub id: String,
    #[serde(default)] pub name: String,
    #[serde(default)] pub ext: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size: Option<u64>,
    pub star: Option<u8>,                       // [live] no key until a rating is set
    #[serde(default)] pub tags: Vec<String>,
    pub folders: Option<Vec<String>>,           // missing and [] are different cases (correction 15)
    #[serde(default)] pub palettes: Vec<PaletteDto>,
    pub modification_time: Option<i64>,
    pub mtime: Option<i64>,
    pub btime: Option<i64>,
    pub duration: Option<f64>,                  // [live] no key for images
    pub no_thumbnail: Option<bool>,
    pub is_deleted: Option<bool>,
    pub annotation: Option<String>,
    pub url: Option<String>,
    pub comments: Option<serde_json::Value>,
}

// [live] [{"color":[142,83,55],"ratio":47}, …], sorted by ratio descending; ratio can be fractional
#[derive(Debug, Clone, Deserialize)]
pub struct PaletteDto {
    #[serde(default)] pub color: Vec<u8>,
    #[serde(default)] pub ratio: f64,
}
```

A real item: `fixtures/eagle-api/item-get.by-id.1.json`.

**Folders.** The password string is turned into a flag while parsing and is never stored:

```rust
fn non_empty<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(Option::<String>::deserialize(d)?.is_some_and(|s| !s.is_empty()))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDto {
    pub id: String,
    #[serde(default)] pub name: String,
    #[serde(default)] pub description: String,
    #[serde(default)] pub children: Vec<FolderDto>,
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub extend_tags: Vec<String>,
    pub icon_color: Option<String>,
    pub cover_id: Option<String>,
    pub order_by: Option<String>,
    pub sort_increase: Option<bool>,
    pub image_count: Option<u32>,
    pub descendant_image_count: Option<u32>,
    #[serde(rename = "isUnLock", default)]
    pub is_unlock: bool,
    #[serde(rename = "password", default, deserialize_with = "non_empty")]
    pub is_locked: bool,                        // the password string is dropped right here
    // passwordTips and the desktop's service keys (correction 33) are ignored
}
```

- `ItemLite` is what scope lists and snapshots store: the list fields (spec §8.3 for Eagle; the same fields from SQL for the native library), tags interned as `Arc<str>` (a large library repeats the same tag strings many times), and only the first palette colour as `[u8; 3]`.
- **List fields** (correction 23): `id, name, ext, width, height, size, star, tags, folders, palettes, modificationTime, mtime, btime, duration, noThumbnail`.
- `FolderIndex`: `by_id`, `parent_of`, `children_of`, `descendants(id)` (cached), `ancestors(id)`, `locked_ids` (§6.4). For Eagle, `parent_of` is built from `children` nesting: `folder/get` has no `parent` field (correction 8). The native library stores `parent_id` and builds the same nested `Folder` list.
- The order of folders at the root and among siblings is the order of the `children` array (Eagle) or of `sort_key` (native). Keep it as is.
- Types that cross IPC derive `ts_rs::TS` and are exported to `ui/src/ipc/`. DTOs from Eagle and SQL rows never cross IPC; the UI only gets UI-shaped types such as `ItemCell` (§6.5), `FolderNode` and `ItemDetail`.

---

## 6. Query engine

### 6.1 Principle

**The source loads the scope. Everything else is computed locally, in Rust.**

```
LibraryQuery ──► LibrarySource::load_scope(scope, hints) ──► Vec<ItemLite> (snapshot)
     │              native: one SQL query (§15.3)                  │
     │              Eagle:  plan() → FetchPlan → API pages (§6.2–§6.3)
     │                                                              │
     └──────────────────────► pipeline() ◄──────────────────────────┘
                (lock → scope union → tags → ext/rating/shape → text ids → sort)
                                  │
                                  ▼
                      View { id, items } ──► view_range() ──► UI grid
```

Changing the tag mode, the sort or a chip keeps the same scope, so the source is not asked again and the grid updates instantly. The membership semantics (union for folders with descendants, Any and Exact for tags) live in one place, the pipeline, for every source. For Eagle this also avoids its intersection-only server filters.

The native library could push filters and sorting into SQL; that is a later optimisation, measured first (§15.3). The pipeline and the views below are shared by every source.

### 6.2 Scope and the Eagle planner

```rust
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Scope {
    All,
    Folder { id: String, include_descendants: bool },
    Unfiled,
    Untagged,
    Smart { id: String },
    Trash,                                   // native library only; Eagle's API lists no trashed items
}

#[derive(Clone, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerFilter {                    // exactly what goes into item/get; this is the cache key
    #[serde(skip_serializing_if = "Option::is_none")] pub folders: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")] pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")] pub is_unfiled: bool,   // false → key omitted, never false
    #[serde(skip_serializing_if = "std::ops::Not::not")] pub is_untagged: bool,
    #[serde(skip_serializing_if = "Option::is_none")] pub ext: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub rating: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] pub shape: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum FetchPlan {
    ItemGet(Vec<ServerFilter>),              // several filters → union by id
    SmartFolder(String),
}

pub fn plan(q: &LibraryQuery, library_count: usize) -> FetchPlan {
    let only = |f: ServerFilter| FetchPlan::ItemGet(vec![f]);
    match &q.scope {
        Scope::Smart { id } => FetchPlan::SmartFolder(id.clone()),
        Scope::Unfiled => only(ServerFilter { is_unfiled: true, ..Default::default() }),
        Scope::Untagged => only(ServerFilter { is_untagged: true, ..Default::default() }),
        Scope::Folder { include_descendants: true, .. } => only(ServerFilter::default()),  // union computed locally
        Scope::Folder { id, .. } => only(ServerFilter { folders: Some(vec![id.clone()]), ..Default::default() }),
        Scope::All if library_count <= LARGE_LIBRARY => only(ServerFilter::default()),
        Scope::All => FetchPlan::ItemGet(safe_prefilters(q)),  // one tag / All → tags; Any of N tags → N single-tag filters; ext/rating/shape
        Scope::Trash => unreachable!("rejected with SourceError::NotSupported before planning"),
    }
}
```

`ServerFilter`, `FetchPlan` and `plan()` live in `eagle-api`; the native library has no planner (§15.3).

- For `Folder { include_descendants: true }` on a large library there is a cheaper option: one `folders: [d]` request per descendant and a union of the results. Choose it when there are few descendants (up to 20); otherwise load everything.
- `LARGE_LIBRARY` is around 5000. Above this threshold, All without chips loads only after a confirmation. `library_count` comes from `item/countAll`, which includes the trash (correction 25). That is enough to choose a strategy, not to show to the user.

### 6.3 Snapshots and the Eagle cache

**Native library.** A snapshot is the result of `load_scope` for one scope, kept in a small LRU (4–6). The app is the only writer, so there is no TTL: every write knows which items it changed and patches or drops the affected snapshots directly (the same `matches_locally` idea as below, evaluated on the stored item).

**Running Eagle.**

- An LRU of 6–8 `Snapshot { plan, items: Arc<Vec<ItemLite>>, loaded_at }`, behind a `tokio::sync::RwLock` in `AppState`.
- Stale-while-revalidate: after 60 s (TTL) or when the window regains focus, a snapshot is reloaded in the background while the old data stays on screen.
- While loading, progress `{ loaded, total }` goes to the UI through the `Channel` passed to `open_view`, so the UI can show "3000 / 12 450".
- **Update after a write.** `item/update` returns the full item. For each snapshot: if `ServerFilter::matches_locally(&item)`, replace or add the item, otherwise remove it. The check supports `folders`, `tags`, `is_unfiled`, `is_untagged` and `ext`. If the filter has `rating` or `shape`, the snapshot is simply dropped.
- `item/add`, `tag/update`, `tag/merge` and folder operations drop all snapshots and the tree.

### 6.4 Local pipeline

Steps in order, all pure functions in `library-core`, run on `tokio::task::spawn_blocking` for large snapshots:

1. **Lock filter.** `locked_ids` = locked folders that are not unlocked, together with their descendants. Drop items where `folders ∩ locked_ids ≠ ∅`. For Eagle a folder is locked when `is_locked && !is_unlock` (correction 9; the desktop hides such items in every view); for the native library when `locked` is set and the folder was not unlocked in this session, including a folder imported from Eagle that has no identity yet. Unlocking one folder does not unlock the others.
2. **Scope.** For `Folder { include_descendants: true }` keep items where `folders ∩ descendant_ids(root) ≠ ∅`. For `Unfiled` keep items whose `folders` is missing or empty.
3. **Tags.** `matches_tags` (Any / All / Exact / None); strings are compared exactly, case-sensitive.
4. **Extra folder chips**, with the same matcher over ids.
5. **ext, rating, shape.** Type buckets (`image`, later `video`, `audio`, …) come from a local extension table.
6. **Text.** `LibrarySource::search_ids(text)` → a `SearchMatch`: the ids that match (`Only`), or, for a search that only excludes (`-draft`), the ids that do not (`Except`), so no list of every item is needed (from 2 characters; the 300 ms debounce is in the UI). `library-core` parses Eagle's syntax (`OR`, parentheses, a leading `-`, `"…"` for spaces) into a `SearchExpr`; the native library matches each term as a case-insensitive substring of names, notes, links, tags and folder names (§15.3, D-047) and combines them, a running Eagle answers from `item/query`.
7. **Sort.** Keys from §5 of the spec. Names use `icu_collator` (numeric, case-insensitive) through sort keys computed once per item: 100 000 titles sort in about 0.2 s in a release build. Random order uses a seed derived from the query without its sort. Ties fall back to date added, then the id.

### 6.5 Views and IPC windows

A **view** is the sorted result of one `LibraryQuery`:

```rust
pub struct View {                            // library-core: positions into the scope's snapshot
    id: u64,
    query: LibraryQuery,
    snapshot: Arc<[ItemLite]>,               // shared with the snapshot LRU and the other views
    order: Vec<u32>,                         // the pipeline's result
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ItemCell {                        // what one grid cell needs
    pub id: String,
    pub name: String,
    pub ext: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub star: u8,
    pub palette0: Option<[u8; 3]>,
    pub has_thumbnail: bool,                 // native: thumb_version > 0; Eagle: a byte source exists
    pub thumb_version: i64,                  // native: item.thumb_version; Eagle: modificationTime; part of the thumbnail URL
    pub size: Option<u64>,                   // the list's columns
    pub added_at: Option<i64>,
    pub tags: Vec<String>,
}
```

`Views` (library-core) keeps the views and the snapshots and reads the source through `ViewSource`, a blocking adapter the app builds over `LibrarySource`. The Eagle source (`FetchPlan`) keys its own cache.

- `open_view(query, on_progress) → { viewId, total }` builds or reuses the snapshot, runs the pipeline and stores the view. Every call gets its own view, so closing one never closes another with the same query (React runs effects twice in development, D-039). At most 4 views are kept (LRU); the UI closes the previous view once the new one is open.
- `view_range(viewId, start, len) → Vec<ItemCell>`, with `len ≤ 200`.
- `view_ids(viewId, start, len) → Vec<ItemId>`: ids only, for Shift ranges and the rubber band. Select All is a flag in the UI, never a list of ids (D-039).
- After a write, affected views are recomputed **under the same id**, and Rust emits `views-invalidated { viewIds }`. The UI invalidates the TanStack Query keys of those views and refetches only the visible windows, so the scroll position survives.
- UI side: a TanStack Virtual row virtualizer; `columns = floor(width / cellWidth)`, `rows = ceil(total / columns)`. The visible rows are turned into windows of 120 cells, each fetched with TanStack Query under the key `['view', viewId, windowIndex]`.

Whole item lists never cross IPC: JSON serialization of tens of thousands of objects through the WebView bridge is slow and blocks the UI.

---

## 7. Writes

### 7.1 Items: intents, not arrays

```rust
#[derive(Clone, Deserialize, TS)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ItemEdit {
    AddTags(Vec<String>),
    RemoveTags(Vec<String>),
    AddToFolders(Vec<String>),
    RemoveFromFolders(Vec<String>),
    SetFolders(Vec<String>),                 // checklist screen
    SetStar(u8),                             // 0..=5, otherwise the server silently ignores it
    SetAnnotation(String),                   // ≤ 10 000 characters
    SetUrl(String),                          // the source link; http, https or empty
    Rename(String),                          // ≤ 10 000 characters; the stored file is renamed too
    Trash,                                   // native: deleted_at; Eagle: isDeleted: true
    Restore,                                 // native only: back from the trash
}
```

**Native library.** `apply_item_edit` validates the intent, then reads the item, applies the intent and writes the result in one SQLite transaction, with a `change_log` row. A batch edit of a selection is one transaction for the whole batch. The write returns the stored item; snapshots and views are updated from it (§6.3).

**Running Eagle.** Eagle replaces `tags` and `folders` wholesale and other clients (the desktop itself) can change the item at any time, so the intent is applied to a fresh copy:

```rust
pub struct ItemMutator {
    api: Arc<EagleApiV2>,
    write_lock: tokio::sync::Mutex<()>,      // writes are strictly sequential
}

impl ItemMutator {
    pub async fn apply(&self, id: &str, edit: ItemEdit) -> Result<ItemDto, EagleError> {
        edit.validate()?;                                                    // → EagleError::Invalid
        let _guard = self.write_lock.lock().await;
        let fresh = self.api.item_by_id(id).await?.ok_or(EagleError::NotFound)?;  // narrows the race with the desktop
        let body = edit.to_update_body(&fresh);                              // tags/folders are always full arrays
        self.api.item_update(body).await?.ok_or(EagleError::NotFound)       // data: false → NotFound
    }
}
```

Optimistic UI, for every source: the UI shows the intended state at once (TanStack Query optimistic update). On an error, Rust returns the stored state, the view is recomputed, and the UI rolls back. For Eagle, `item/update` recomputes bindings on the desktop, which takes noticeable time in a large library, so writes are strictly sequential.

**Undo** (native library): the app keeps the inverse of the last edits of this session in memory (the item's previous tags, folders and fields), and `Ctrl+Z` applies the inverse as a new edit. Permanent deletes are never undoable. The session reads `item_states` before each edit; `UndoStack` in `library-core` turns them into batches, one per distinct inverse edit, and keeps 50 (D-050).

### 7.2 Eagle folders: always the full set of desired values

The native library writes folder rows directly. A running Eagle needs the following.

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderUpdateBody {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,          // None → field omitted → "no colour"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,              // None → field omitted → "at the root"
}
```

Both branches were verified live. A missing `parent` on a nested folder moves it to the root, so for a nested folder `parent` is mandatory. An explicit `null` on a root folder moves it to the end of the list (correction 31), so `null` is never serialized.

The body is always built from a **fresh** tree node with the edit applied. A stale node risks clearing the colour or moving the folder (correction 3). That is why `folder/get {id}` is re-read before a folder edit.

### 7.3 Cycle guard

`can_move(folder, new_parent)`: `new_parent != folder` and `folder ∉ ancestors(new_parent)`. It lives in `library-core` and runs for every source. If the move is not allowed, nothing is written and no request is sent. For Eagle: if `parent` did not change after the response although we changed it, that is an error: the server refused silently.

### 7.4 Adding files

The Windows app adds files that the user picks in the file dialog (`tauri-plugin-dialog`, multi-select), drops on the window (Tauri drag-and-drop events give file paths; dropped folders are walked) or pastes from the clipboard. Rust reads the files; their bytes never pass through the WebView.

**Native library** (the main path):

```
paths ─► Rust: an import task per batch, owned by AppState; files processed by a bounded pool
      ─► per file (library-native add_file): stream-copy into <library>/files/.tmp/<ulid> while hashing (BLAKE3); sync_all
      ─► duplicate? (content_hash index, the trash included) → the batch's choice: skip | add anyway | add existing item to the target folder
      ─► INSERT pending_file(path), commit; rename the temp file to files/<shard>/<id>/<file name> (retried on sharing violations)
      ─► INSERT item (+ folders, tags, change_log, item_fts) and DELETE pending_file in one transaction
      ─► import-progress event; the item appears in views with the placeholder
      ─► media job (rayon): probe, thumbnail, palette → UPDATE item, thumb_version += 1 → views-invalidated
```

- A crash before the journal entry leaves only a file in `files/.tmp/`; opening the library deletes that folder's content. A crash after it leaves a `pending_file` row, and the next open deletes the file unless an item points at it. So a row never points at a missing file, and no file stays without a row, without walking `files/` (`library-format.md` §6, D-023). `synchronous = FULL` makes each commit durable, which this order relies on.
- `name` is the file name without extension; `ext` the lowercase extension. The stored file keeps the original name with the part before the extension cut to 100 UTF-16 units. Folder ids are checked against the tree, and the tags of the target folder and its ancestors are added to the item (spec §3).
- Files of any type are accepted; only the v1 formats get previews (D-015).
- Cancelling stops the batch after the current file; added items stay.

**Running Eagle:**

```
paths ─► Rust: one import task per file (sequential, through the write semaphore)
      ├─ same PC as Eagle (127.0.0.1 / localhost):
      │     POST item/add { items: [{ path, name, folders:[F]?, tags?, annotation?, star?, modificationTime? }] }
      │     batch form even for one file: keeps star and modificationTime; Eagle copies the file (correction 35)
      └─ another PC:
            read the file (size limit, e.g. 50 MB) → base64 without line breaks
            POST item/add { base64: "data:<mime>;base64,…", name, folders:[F]?, tags?, website? }  (correction 1)
      ─► { id } ─► "Importing…" cell (import-progress event)
      ─► poll item/get {id}: 0.25/0.5/1/2/4 s                (correction 2; ~0.3 s live)
      ─► appeared: (data URL only, if a star was chosen) item/update {star} (correction 11)
      ─► drop snapshots, recompute views, the cell becomes the real item
```

- `name` is the file name without extension. Folder ids are checked against the tree first (correction 13).
- The MIME type comes from the extension. The data URL path is verified live for JPEG only; check PNG, GIF, WebP and video files in phase 10 before relying on them. The same-PC `path` import avoids the question, because Eagle reads the file itself; it is verified live (correction 35).
- Files above the size limit are refused with a message, not sent: Eagle reads the whole JSON body into memory.
- Imports run as tokio tasks owned by `AppState`, so navigating away does not cancel them.

Later, for phones: the WebView prepares the image (`createImageBitmap`, long edge 1920 px, JPEG 0.85; originals for PNG, GIF and formats the WebView cannot decode), sends the bytes as a raw IPC body, and Rust continues with the data URL path above.

---

## 8. UI, IPC and navigation

**Commands** (all return `Result<T, SourceError>`; added with their phases, spec §9):

| Command | Input | Output |
|---|---|---|
| `app_info`, `connection_defaults` | — | version; Eagle connection defaults (phase 0) |
| `default_library_location` | — | the parent folder the Create dialog starts in: the user's preference, else `%USERPROFILE%\egl-library` (D-021) |
| `create_library` | parent folder, name | `LibraryInfo`; the library is open |
| `open_library` | path | `LibraryInfo` with `readOnly` and `readOnlyReason`; on the library that is open read-only it retries the lock ("Open for editing") |
| `close_library` | — | — |
| `recent_libraries`, `forget_recent_library` | —, path | `RecentLibrary[]` (name, path, last opened, missing) |
| `folder_tree` | — | `Folder` tree with counts (no passwords) |
| `open_view` | `LibraryQuery`, `onProgress: Channel<Progress>` | `{ viewId, total }` |
| `view_range` | viewId, start, len (≤ 200) | `ItemCell[]` |
| `close_view` | viewId | — |
| `view_ids` | viewId, start, len | `ItemId[]` (selections) |
| `item_detail` | id | `Item` or none (the details panel; also items in the trash) |
| `apply_item_edit`, `apply_batch_edit` | id or `ItemsTarget` (ids, or a view's Select All minus ids), `ItemEdit` | `Item` / count; phase 4 has the folder intents, phase 5 the tag intents; hidden items are left alone |
| `reorder_items` | folder, ids, the id they go before (none: the end) | — (manual order) |
| `undo`, `undo_status` | — | `UndoStatus` (the kind of edit and how many items) or none; the Edit menu names it |
| `save_folder`, `delete_folder` | `FolderChange` (create, update with every property, move to a parent and index); id and "trash the items only in it" | `Folder` tree |
| `lock_folder` | id, password, label, label colour, `onProgress: Channel<LockProgress>` | `RecoveryKey` (shown once; never stored) |
| `unlock_folder`, `relock_folder` | id (and the password) | — (views recomputed) |
| `recover_folder` | id, recovery key or words, new password | — (the folder is open) |
| `change_folder_password`, `renew_recovery_key` | id, current password (and the new one) | — / `RecoveryKey` |
| `remove_folder_lock` | id, password, `onProgress: Channel<LockProgress>` | — (the folder is decrypted) |
| `set_lock_label` | id, label, label colour | — |
| `save_recovery_file` | path from the Save dialog, folder name, `RecoveryKey` | — (refused inside the library folder) |
| `tag_vocabulary` | — | `Tag[]`: tags of visible items (the trash included) and of tag groups, with counts outside the trash, by name |
| `selection_tags` | `ItemsTarget` | `SelectionTags`: the number of selected items and, per tag, on how many of them it is (read from the view) |
| `tag_groups`, `save_tag_group`, `delete_tag_group` | —, `TagGroupChange` (create, update with every property), id | `TagGroup[]` in their order |
| `rename_tag`, `merge_tags`, `delete_tag` | from and to, source and target, name | — (views recomputed); a rename onto an existing tag merges (D-046) |
| `smart_folders` | — | `SmartFolder` tree, each with the rules this build cannot evaluate (`unsupported`) |
| `smart_rules` | `LibraryQuery` | `SmartDraft { parent, conditions }` for Save as smart folder; refused for search text, the trash, or nothing to keep (D-048) |
| `extensions` | — | `ExtensionCount[]` for the Type filter |
| `save_smart_folder`, `delete_smart_folder` | `SmartFolderChange` (create under a parent, update name and rules), id | smart folder tree; open views of it are recomputed |
| `delete_permanently`, `empty_trash` | `ItemsTarget` / — | counts; only items of the trash, files removed after the rows (restoring is the `restore` intent of `apply_batch_edit`) |
| `add_files` | paths, the open folder or none | `ImportTicket`; dropped folders are mirrored (phase 4); tags and rating for the batch come with phases 5 and 7 |
| `paste_files` | the open folder or none | `ImportTicket` or none: copied files, or a pasted image as a PNG |
| `answer_import` | import id, `skip` / `addAnyway` / `addToFolder` / `cancel` | — (the batch's duplicate question) |
| `cancel_import` | import id | — |
| `imports` | — | `ImportProgress[]`: running, then finished, newest first |
| `library_counts` | — | `LibraryCounts` for the left bar |
| `quit` | — | — (after the user agreed to stop running imports) |
| `open_original`, `reveal_originals`, `copy_originals`, `start_drag` | id / ids | — / count; Rust finds the files and refuses items of locked folders (D-052) |
| `open_link` | url | — (`http` and `https` only) |
| `backups`, `back_up_now`, `restore_backup` | —, —, file name | `Backup[]`, `Backup`, `LibraryInfo` (D-051) |
| `rename_library`, `library_stats`, `reveal_library` | name, —, — | `LibraryInfo`, `LibraryStats`, — |
| `rebuild_thumbnails`, `check_library`, `repair_library` | —, —, `RepairOptions` | items waiting, `LibraryCheck`, `LibraryCheck` |
| `import_eagle_library` | `.library` path, target, options | import id (spec §8.2) |
| `connect_eagle` | host, port, token | `{ app: AppInfo, library: LibraryInfo }`; stores the endpoint and the token (spec §8.3) |
| `eagle_session` | — | `{ host, port, hasToken, connected }` — never the token |
| `disconnect_eagle` | — | — |
| `export_to_eagle` | what to export | export id (spec §8.4) |
| `on_focus` | — | — (revalidates a running Eagle, §9) |

Every command that works on a library goes through the open `LibrarySource`; a source that cannot do something returns `ReadOnly` or `NotSupported`, and the UI hides the control using `capabilities`.

**Events** from Rust: `views-invalidated` (`{ views: [{ viewId, total }] }`, recomputed at most every 500 ms while an import writes and once at its end, before its final progress), `import-progress`, `close-requested` (the window's close was held back because files are being added; the UI asks, then calls `quit`), `library-closed` (the library folder disappeared or the lock was lost), and for Eagle `connection-state` and `library-changed` (another library was opened on the PC).

**UI structure.** Routes: `/welcome`, `/library` with the scope and the filters in typed search parameters, `/item/$id` as a side panel or a dialog, `/eagle/connect`, `/eagle/import`, `/eagle/export`. The tag, filter and sort sheets are components of the library screen, not routes. Server state lives in TanStack Query (keys per view window, per item, for the tree and the vocabulary); local UI state stays in components. The last `LibraryQuery` per library is saved by `app-store` so that the app reopens where it was.

**Layout.** Desktop-first, as in spec §7.3: the menu bar, the left bar, the toolbar and the filter bar over the grid, the right bar with the metadata, the status bar. The panels use shadcn `resizable`; their widths and open state are saved by `app-store`. The layout is responsive, so a narrow phone layout can be added later without restructuring. UI text is English only in v1 and goes through one place per screen (D-007).

**Window and menu** (D-025). The window keeps the native frame in phase 1 (`decorations` on), so Snap Layouts, the system shadow and a dark title bar (`set_theme` on the window) come from Windows. The menu bar is a React component (shadcn `Menubar`), not the native Tauri menu, whose classic Win32 look does not match the app. A custom title bar can merge the title and menu rows in phase 12: `decorations: false`, `data-tauri-drag-region`, the window permissions in the capabilities, and `Alt` to reach the menu from the keyboard. HTML caption buttons lose the Snap Layouts of Windows 11 unless a plugin such as `tauri-plugin-decorum` restores them; that would be a new dependency.

**Commands** (D-025). One registry in `ui/src/commands/`. Each command has an id, an English label, a shortcut, an `enabled(state)` check (open library, `capabilities`, selection) that also returns the reason when it is disabled, and a handler. The menu bar, context menus, keyboard shortcuts, the Keyboard Shortcuts dialog and a later command palette (`Ctrl+K`) are all built from it, so each shortcut is defined once. Commands of later phases are absent, not disabled.

**Motion** (D-028). Only `opacity` and `transform`, 120–200 ms: `tw-animate-css` (already used by shadcn) for menus, popovers, dialogs and toasts; CSS transitions for panels; a fade-in when a thumbnail loads; the full-size preview grows out of its cell through the View Transitions API (`document.startViewTransition`, available in WebView2's Chromium). No animation on scroll, sort, filter, selection or hover, and no layout animation of grid cells. Everything is off under `prefers-reduced-motion`, which WebView2 takes from the Windows animation setting. No animation library (Motion) while CSS and View Transitions are enough.

**Desktop behaviour** (D-029). The WebView must not feel like a browser:

- The default WebView2 context menu is off; right-click opens the app's own context menus (shadcn `ContextMenu`), built from the command registry.
- `F5`, `Ctrl+R`, `Ctrl+P`, the browser's `Ctrl+F`, back navigation and page zoom do nothing browser-like. `Ctrl` + mouse wheel and `Ctrl+=` / `Ctrl+-` change the thumbnail size instead.
- Text in the chrome is not selectable (`user-select: none`); fields and notes are. Images are not draggable as web images; dragging files out of the window is native (phase 7).
- Buttons show the arrow cursor, as desktop apps do. Focus rings appear only for keyboard focus (`:focus-visible`).
- The window starts hidden and is shown after the first paint, so a dark theme never flashes white.
- Scrollbars follow the app theme. `color-scheme` is light or dark with the theme class, and the thumb uses the `ring` token, so a dark grid does not keep the light system bar (D-054).
- Window size, position and the maximized state are saved by `app-store` (D-022).

Some of these are Tauri or WebView2 settings and some are handled in the UI; which is which is settled when phase 1 is built.

**Components** are added when a screen needs them (D-006): `menubar`, `resizable`, `dialog`, `dropdown-menu`, `context-menu`, `tooltip`, `sonner`, `tabs`, `slider`, `scroll-area`, `sheet`, `toggle-group`, later `command`.

---

## 9. Lifecycle and data freshness

**Native library.**

- **Start:** the app reopens the last library unless it was closed explicitly, `Shift` is held (read in Rust at startup: the WebView cannot see a key held before it loads) or `--no-reopen` is passed; otherwise it shows Welcome. A missing or refused library, or one that another instance has open, goes back to Welcome with a message (D-022).
- **Open:** the steps of `library-format.md` §7: the location check, the header check on a read-only connection, the OS lock on `library.lock` (or read-only), `quick_check` after an unclean close, `meta.open_by`, migrations after a backup, `pending_file` recovery, clearing `files/.tmp/`, the daily backup; then the tree and the vocabulary.
- **While open:** the writer is the only writer, so its data never goes stale; writes update snapshots and views directly (§6.3). The lock is an OS file lock held by the process (`File::try_lock`), so a crash or a kill releases it at once; there is no heartbeat (D-020). A read-only instance compares `PRAGMA data_version` when the window regains focus and reloads when another process has written; "Open for editing" retries the lock.
- **Switching** to another library opens and checks the new one first, taking its lock, and closes the current one only on success; on failure the current library stays open (D-024).
- **Close** (another library, Welcome, quitting): finish or cancel running imports after asking, WAL checkpoint, remove `meta.open_by`, close the database, release the lock. Quitting is not an explicit close.
- **Processes:** several app processes may run; each has one window and one library (D-024).

**Running Eagle.**

- **Window focus** (`tauri://focus`, or `document.visibilitychange` in the UI) → `on_focus` → `GET library/info`:
  - `path` changed → another library is open on the PC: drop everything, reload the tree and the views, emit `library-changed`.
  - `modificationTime` changed → folder or smart folder metadata changed: reload the tree.
  - The visible views are marked stale and revalidated.
- Network loss: a single banner; Rust retries with a growing delay and emits `connection-state`.
- Reconnect: the endpoint and the last query come from the store, the token from Credential Manager; the data is loaded again.
- Eagle has no events (WebSocket or push), so no background polling is needed.
- If the PC is unreachable, a later phase shows data from an offline cache (§12.3).

---

## 10. Security

1. The Eagle token lives in Windows Credential Manager (through `keyring`) and in memory as `SecretString`. It never reaches the WebView, the store file, logs or crash reports. `eagle_session` only reports `hasToken`.
2. POST: token in the body; GET: token in the query (correction 6).
3. Folder passwords are dropped while parsing JSON (§5). Raw bodies of `library/info` and `folder/get` are never logged.
4. The Host bypass (correction 5) is Eagle's problem, not the client's. The client never uses it. Help text on the Connect screen: "Allow port 41595 in the firewall for the private network only; better, only for this computer's IP".
5. `/api/script/inject` and `/api/check` are absent from the client entirely: no method, not even a path string.
6. Tauri capabilities allow only the app's own commands and the plugins in use (`dialog`, `opener` limited to `http`/`https`). No `shell`, no `fs`, no `http` plugin: the UI cannot read files or make requests on its own.
7. A strict CSP in `tauri.conf.json`: scripts only from the app bundle; images from `'self'`, `data:`, `blob:` and the `thumb` scheme (`http://thumb.localhost` on Windows).
8. Commands validate their input in Rust (lengths up to 10 000, star 0–5, known folder ids) before any write or request.
9. The `thumb` scheme and the commands that open or reveal files only touch files whose canonical path is inside the open library folder (the native library, or an Eagle library on the same PC). Item ids from the UI are looked up in the database; the UI never passes file paths for them.
10. Paths the user picked in a dialog or dropped on the window are only read (to copy them into the library); the app never writes outside the library folder, the app's local data folder and a folder the user chose for a new library.
11. A native locked folder stores salt, Argon2id parameters, and wrapped age identities on the folder row (D-018). The password, the recovery secret, and the unwrapped identity are not stored. The lock dialog says the lock encrypts that folder on disk. The name and the tag (label and colour) stay plaintext. The Eagle token stays in Windows Credential Manager.

---

## 11. Thumbnails

Rust serves images to the WebView through a custom URI scheme:

```rust
tauri::Builder::default()
    .register_asynchronous_uri_scheme_protocol("thumb", |ctx, request, responder| {
        let app = ctx.app_handle().clone();
        tauri::async_runtime::spawn(async move {
            responder.respond(thumbs::serve(&app, &request).await);  // cache → source → cache
        });
    })
```

- **URL.** `http://thumb.localhost/<id>?v=<thumbVersion>` on Windows (and later Android); `thumb://localhost/<id>?v=…` on macOS, iOS and Linux. The UI builds it with one helper (`thumbUrl(cell)`, on top of `convertFileSrc("", "thumb")`). `/<id>/original` serves the original for the full-size preview. The scheme serves the library that is open in this window, through `LibrarySource::thumbnail` and `LibrarySource::original`; ids other than short ASCII letters and digits get a 404 before any lookup.
- **Headers.** `Content-Security-Policy: default-src 'none'` and `X-Content-Type-Options: nosniff` on every response, so an SVG original is only ever an image.
- **The URL is versioned**, so responses carry `Cache-Control: max-age=31536000, immutable`. A version is never used twice for an item: Rebuild Thumbnails continues the numbers of the thumbnails it replaces (D-051).
- **Source, native library.** `thumbs/<shard>/<id>.webp` inside the library, written by the media pipeline (§15.4). A missing file means "not generated yet": the scheme answers 404, the cell keeps the palette placeholder, and `views-invalidated` with a new `thumb_version` makes it retry. No extra cache: the files are already local.
- **Disk cache, Eagle only.** `app_cache_dir()/eagle-thumbs/<libraryKey>/<id>-<version>`, with a size limit (for example 500 MB, oldest first).
- **Source, same PC as Eagle.** v1 `GET /api/item/thumbnail?id=` returns the thumbnail's file path on this PC. Rust reads that file after checking that it lies inside the library folder. When `noThumbnail` is true there is no `_thumbnail.png` (correction 29): Rust takes the original from `images/<id>.info/` (matched by extension) and downscales it to 320 px. So on the same PC thumbnails work without any helper.
- **Source, another PC.** A read-only helper on the Eagle PC, best built as an **Eagle plugin**. The MCP Server plugin already serves HTTP on `41596` (`preferences.js:1288`), so a plugin can listen on a port. The helper serves `GET /thumb/{id}` (the original downscaled to 320 px when `noThumbnail` is set) and can later serve `GET /changes?since=` for the offline cache (§12.3).
- **No source** (another PC without the helper): the scheme answers 404, and the cell shows the palette placeholder (`<img onError>`).

---

## 12. Sync and offline (Eagle libraries)

This section is about working with Eagle libraries. Native libraries have one writer, automatic backups and no file sync while open (§15.3); sync between devices is a later feature (§15.8). §12.3 and §12.4 are later phases (spec §8.5).

### 12.1 The main rule: only Eagle writes to the library

Every change from the client goes **only through the API** of a running Eagle. The client never writes into the `.library` folder: not directly, and not through Syncthing, OneDrive or SMB. On the same PC the client may **read** files inside the library (thumbnails, §11), never write them.

| Source | When it is used | What it allows |
|---|---|---|
| API | PC reachable | Reads and all changes. This is the source of truth |
| The app's offline cache (§12.3) | PC unreachable | Read only. Files can be queued for import |
| Syncthing mirror (§12.4, optional) | PC unreachable, originals needed | Read only |

While the PC is reachable, data comes from the API even if a cache or a mirror exists: only the API reflects the state in Eagle's memory.

### 12.2 How Eagle stores the library and notices outside changes

The layout was checked on the open library; the behaviour comes from the code in `app/js/background.js`.

```
Name.library/
  metadata.json        folder tree, smart folders, quickAccess, tagsGroups, modificationTime
                       (the same as library/info, without path and name)
  mtime.json           { "<id>": <lastModified, ms>, …, "all": <item count> }
  tags.json            { "historyTags": [], "starredTags": [] }
  saved-filters.json
  backup/              backups of metadata.json (backup-YYYY-MM-DD HH.mm.ss.SSS.json)
  images/<id>.info/
    <name>.<ext>       the original
    <name>_thumbnail.png   not always present: missing when noThumbnail is true (correction 29)
    metadata.json      the item object as in the API plus a lastModified field
```

**Findings about `mtime.json`:**

1. **It is a change log for items.** The key is the id, the value is the item's `lastModified`. Live example: `{"MVQTEQPE8FAQM":1793113655657,"MVQTEQPM2IAAG":1793113467866,"all":2}`.
2. **Trashed items are in it too.** They stay on disk with `isDeleted: true`, and `all` counts them. This explains correction 25: `item/countAll` equals `all` and includes the trash.
3. **`lastModified` exists only on disk.** API item objects do not have it. So the API alone cannot answer "what changed since X": only a full reload is possible.
4. **Eagle polls the files every 4 seconds** (`fs.watchFile`, `interval: 4000`):
   - `metadata.json` (`:1753`): if the file's `modificationTime` is **newer** than the one in memory, the whole folder tree is replaced (`:1783`, `:1791`). Otherwise the file is ignored.
   - `mtime.json` (`:1821`): a new id → the item is loaded; a larger `lastModified` → the item's `metadata.json` is re-read **as a whole** (`:1865`). If `all` does not match the item count, the `images/` folder is rescanned for new and deleted items (`:1873`). A comment in the code admits that this check can miss cases.
5. **Conflicts are resolved by "newest wins", at the level of a whole file or a whole item.** Fields from different versions are never merged.
6. **`mtime.json` is a single file that everyone rewrites.** Eagle updates it on every save. With two-way file sync, Syncthing creates a `.sync-conflict` copy. If the version with the other side's edits loses, Eagle never learns that the item changed: the edit is silently lost, and the next save on the PC overwrites it.
7. **Temporary file.** When saving, Eagle first writes `~$metadata.json.tmp` (`app.bundle.js:64089`). A sync tool must not transfer it.
8. **The format is internal and undocumented.** There is no format version, only `applicationVersion`. The mechanism is designed for sync between Eagle copies where only one side makes changes at any moment.

Conclusion: reading the library files is acceptable; writing them is not.

### 12.3 The app's offline cache (read only, later phase)

Useful for a laptop that is sometimes away from the Eagle PC.

**Modes.** The connection monitor in Rust switches the app to offline after an `EagleError::Network(_)` and back to online after a successful `app/info`. The check repeats on window focus, on network changes and with a growing delay.
- **Online:** everything as in §4–§9. Every loaded snapshot is also written to the cache.
- **Offline:** data from the cache, banner "PC unreachable. Data as of 21:40". Edit controls are disabled.

**Storage.** No database is needed: the pipeline (§6.4) already works on in-memory lists, so the cache is serialized snapshots:

```
app_data_dir()/offline/<libraryKey>/
  index.cbor        LibraryInfo + folder tree + tag vocabulary + Vec<ItemLite> of the whole library + synced_at
  thumbs/<id>-<version>      pinned thumbnails, kept as received
app_data_dir()/outbox/       files waiting for import
```

- `libraryKey` = a hash of `path` from `library/info`. Each library has its own cache.
- CBOR through `ciborium`. Writes are atomic: first `index.cbor.tmp`, then rename.
- **Items from locked folders never enter the cache** (lock filter, §6.4). Otherwise they could be viewed offline without the password.

**Index refresh.** There is no background worker: everything happens while the app is open.
- **Without the helper:** a full reload of the light `item/get` fields in pages of 1000 when the index is older than an hour. A change in `library/info.modificationTime` (folders) or in `item/countAll` (adds or deletes) is a reason to refresh earlier. These signals do not catch item edits, so the TTL is mandatory.
- **With the helper plugin:** `GET /changes?since=<ms>` is built on `mtime.json` and returns `[{id, lastModified}]` and `all`. The client fetches only those ids with `POST item/get {ids:[… up to 1000], fields}`. An id missing from the response means the item is in the trash or deleted, and it is removed from the index. If `all` does not match the index, reconcile the full id list (`item/get {fields:["id"]}` is cheap).
- **Same PC as Eagle:** an offline cache is pointless there, since Eagle and the client stop together.

**Offline thumbnails.** The thumbnail disk cache (§11) evicts old entries and is not a guarantee. A "Keep offline" action on a folder (including its descendants) pins the thumbnails of its items into `offline/<libraryKey>/thumbs/`. A storage limit is set in settings, and the used size is shown.

**Adding offline.** Files can be queued: they are copied into `outbox/`, and when the app is back online the usual path from §7.4 runs. An add creates a new item and conflicts with nothing. **Offline edits are not queued:** meanwhile the item may have been changed or deleted on the PC. If this is ever needed, only `ItemEdit` intents may be queued. `ItemMutator` re-reads the item before sending anyway, and on `NotFound` the edit is dropped with a notification.

### 12.4 Syncthing mirror (optional, read only)

It makes sense only if **originals** are needed without the Eagle PC (for example on a laptop) and the device has enough storage.

**Setup:**
- On the Eagle PC the `.library` folder is **Send Only**, on the other device **Receive Only**. Local changes on the receiving side then never reach the Eagle PC.
- The library **must not live in OneDrive** or another cloud folder. In the current setup it does: `C:\Users\kirkw\OneDrive\Pictures\ggwp.library`. Two sync tools on one folder write the same files. Besides, with files on demand some files exist only in the cloud, and Syncthing would start downloading them or fail on them. Before enabling the mirror, move the library and open it in Eagle from the new location.
- `.stignore`:

  ```
  /backup
  ~$*
  *.tmp
  ```

  A "thumbnails only" mirror is unreliable: small images have no `_thumbnail.png` (correction 29), and `.stignore` rules cannot separate their originals from large ones.

**Reading in the app** is the `eagle-folder` source (§15.2), the same reader that imports Eagle libraries (§15.6), opened read-only:
- On Windows (and later macOS) it is plain file access to the mirrored folder chosen in the folder dialog.
- The root `metadata.json` is parsed with the same DTO as `library/info`. The `password` field is dropped the same way (§5).
- An item's `metadata.json` is parsed as `ItemDto` plus `lastModified`. Items with `isDeleted: true` are skipped.
- The first scan reads every `images/*.info/metadata.json`. After that, `mtime.json` tells which items have a larger `lastModified`, and only those are re-read. This is the same change log Eagle itself uses.
- The original file is found in the `.info` folder by extension, not rebuilt from `name`: the rules for encoding file names on disk are not guaranteed.
- The mirror may be partially synced: metadata has arrived but the file has not. A missing file is not an error: the cell shows the palette and the load is retried later.
- If the Eagle PC is reachable, the mirror is not used (§12.1).
- Later, on phones, folder access needs native code (Android Storage Access Framework, iOS security-scoped bookmarks), and the official Syncthing Android app is discontinued (Syncthing-Fork continues).

### 12.5 Access outside home

The same direct path to the API, but over Tailscale or WireGuard to the Eagle PC. **Do not forward port 41595 to the internet:** the token can be bypassed by spoofing Host (correction 5). If the address is in the Tailscale range (`100.64.0.0/10`) or is a `*.ts.net` name, the connect timeout grows from 3 to 8 s and the firewall hint is not shown.

### 12.6 What not to do

- Do not write `.library` files from the client in any way.
- Do not use Syncthing and OneDrive on the same library folder.
- Do not open one library in two running Eagles at the same time if the library is synced between PCs: one side's edits will be overwritten (§12.2, item 5).
- Do not read files from a mirror while the Eagle PC is reachable: the API is fresher and does not depend on the internal format.

---

## 13. Testing

| Level | What | Where |
|---|---|---|
| Rust unit, `library-core` | The core table in §10 of the spec, plus: `matches_locally` for snapshot updates; the lock filter with descendants; stable random sort; the smart folder evaluator against the cases of §16 | `cargo test`, CI |
| Rust, `library-native` | Create and open; a newer format version is refused; migrations on an older schema run after a backup; `library.lock` blocks a second writer, which opens read-only and writes nothing; after a killed writer the lock is free at once and the unclean close is detected; a library in a sync folder opens read-only and one on a network path is refused; a crash at any step of `add_file` leaves neither a row without a file nor a file without a row; locked folders: items hidden in every scope and count until unlocked, files, previews and fields age ciphertext that another folder's password does not open, recovery, removal, and every write into or out of a lock; a YubiKey wrap (`folder_piv_wrap`, retired slots 82–95) opens only its folder and the password still works without the card; duplicates by hash, the trash included; FTS5 translation of `OR`, parentheses, `-` and Cyrillic; case-sensitive tags; folder delete semantics; emptying the trash deletes rows and files | `cargo test` on temporary folders, CI |
| Rust, `media` | Each v1 format (small files in `fixtures/media/`, made for the tests) gives dimensions, a thumbnail and a palette; an undecodable file gives a clean error | `cargo test`, CI |
| Rust, app crate | `view_range` windows; views recomputed under the same id after a write; `thumb` paths outside the library are refused | `cargo test`, CI |
| Rust, Eagle import | A synthetic `.library` fixture in `fixtures/eagle-library/` (`metadata.json`, `mtime.json`, a few `.info` folders) → a native library with the expected rows, folder order and mapped ids, and no passwords; a truncated `metadata.json` is retried; an interrupted import resumes without duplicates | `cargo test`, CI |
| Rust unit, `eagle-api` | The planner never sends `isUnfiled: false`; a folder subtree → `ServerFilter::default()` without `folders`; `FolderUpdateBody` for a nested folder always contains `parent` | `cargo test`, CI |
| Rust HTTP, `eagle-api` + `wiremock` | Nested page `data.data`; 401; 500 with `message`; 500 with a string in `data`; `success` + `false` → `NotFound`; token in the POST body and in the GET query; body is pure ASCII; `page_all` when `total` changes; at most 2 requests in flight; export sends full folder bodies and batch `item/add` with paths | `cargo test`, CI |
| Rust, Eagle offline and mirror (later) | Writing and reading `index.cbor` (atomic replace, a separate cache per library); items from locked folders never reach the cache; parsing `mtime.json` and computing changed ids | `cargo test`, CI |
| UI | Vitest + Testing Library with `mockIPC` from `@tauri-apps/api/mocks`: the Welcome screen, Connect error copy per `SourceError.kind`, grid window loading, optimistic edit and rollback, dialogs, empty states | `pnpm test`, CI |
| Desktop end to end (optional) | `tauri-driver` (WebDriver) on Windows: create a library, add files, tag one | CI on Windows |
| Live contract | `cargo test --features live` against a real Eagle (`EAGLE_HOST`, `EAGLE_TOKEN` from the environment, off by default). Captures JSON fixtures for the tests above | manual |

**Fixtures** live in `fixtures/eagle-api/`:

| File | Contents |
|---|---|
| `item-add.single.json`, `item-add.batch.json` | `{id}` and `{ids}` responses. base64 is truncated in the request |
| `item-get.by-id.*.json`, `item-get.list.*.json` | A full item, a list with grid fields, a list without `fields` |
| `item-get.tags-cyrillic.json`, `item-get.is-unfiled.json` | Filters by a Cyrillic tag and by Uncategorized |
| `item-query.json`, `item-countAll*.json`, `item-getComments.json` | Search, count (before and after the trash), comments |
| `item-update.star.json`, `.not-found.json`, `.validation-error.json`, `.trash.json` | Success with the full item; `data: false`; HTTP 500; move to trash |
| `item-update.utf8-split.json` | The UTF-8 split check (correction 7) |
| `tag-get*.json`, `tag-getRecentTags.json`, `tag-getStarredTags.json`, `tagGroup-get.json` | Tag vocabulary (before and after the trash) and empty lists |
| `folder-get.json`, `smartFolder-get.json`, `library-info.json`, `app-info.json` | Empty lists and metadata of an empty library (includes the library path on the PC) |
| `folder-create.parent.json`, `folder-create.child.json` | Creating a root folder and a nested one |
| `folder-get.tree.json`, `folder-get.by-id.json`, `folder-get.with-item-in-child.json`, `folder-get.after-partial-update.json`, `folder-get.final.json` | The tree right after creation, one folder by id, the tree with counts, the tree after a partial update, the final tree with `extendTags` |
| `library-info.with-folders.json` | Folder nodes from disk (`password` and `passwordTips` redacted when not empty) |
| `folder-update.without-parent-and-color.json`, `.full-payload-move.json`, `.cycle.json`, `.not-found.json` | Move to root with the colour cleared; a correct move; a silently ignored cycle; an unknown id |
| `_folder-checks.json` | Summary of all folder checks |
| `item-add.path-batch.json`, `item-add.path-single.json`, `item-get.path-*.json`, `_path-import-checks.json` | `item/add` with `path`: batch keeps `star` and `modificationTime`, single does not (correction 35) |

**Still not verified live:**

- locked folders (`password`, `isUnLock`, v1 `folder/unlock`): the API cannot set a password;
- non-empty `tagGroup/get` and `smartFolder/get` objects;
- that raw base64 without `data:` really fails silently (correction 1 is confirmed in code);
- `item/add` data URLs for formats other than JPEG (phase 10);
- v1 `GET /api/item/thumbnail` paths for the same-PC thumbnails (phase 10).

---

## 14. Implementation order

The full plan with done checks is in §9 of the spec. Key technical points per phase:

- **Phase 0 (done).** Workspace with `library-core` (including the `LibrarySource` trait, §15.2), an `eagle-api` stub, `app-store` and the app crate; React app, `ts-rs` export, minimal capabilities and CSP, CI on Windows. The other crates of §3 appear with their phases.
- **Phase 1.** `library-native`: schema v1 from `docs/library-format.md` and migrations, the OS file lock on `library.lock`, recovery after an unclean close, backups, `add_file` for one file; `app-store`: settings, recent libraries, the location check; library commands; the Welcome screen; the frame of the library screen (menu bar from the command registry, both bars, status bar), the Settings, About and Keyboard Shortcuts dialogs, the desktop behaviour of §8.
- **Phase 2.** `media` (hash, probe, thumbnails, palettes, §15.4); `add_files` with the temp-copy protocol (§7.4); import tasks and progress events.
- **Phase 3.** `load_scope` in SQL; the pipeline, sorter and views (§6.4, §6.5); the `thumb` scheme (§11); the virtualized grid.
- **Phase 4.** Folders: `save_folder`, `delete_folder`, locks (age at rest, one Argon2id identity per folder, plaintext name and tag, the Locked folders sidebar), cycle guard (§7.3), `sort_key` reordering, manual order.
- **Phase 5.** Tags: vocabulary with counts, rename, merge, delete, tag groups; batch edits. Built 2026-10-28 (D-046).
- **Phase 6.** FTS5 search with the Eagle syntax; filter chips; the smart folder evaluator for the v1 subset (§16). Built 2026-10-29 (D-047, D-048).
- **Phase 7.** The remaining item edits, undo, trash, file actions, library maintenance. Built 2026-10-29 (D-049 to D-052).
- **Phase 8.** `eagle-folder` reader with retries (correction 37) and the import into `library-native` (§15.6).
- **Phase 9.** `EagleHttp`, `escape_non_ascii`, `EagleError`, `page_all` with `wiremock` tests; `SecretStore` on Credential Manager; `connect_eagle` and the Connect screen (404 → "update Eagle to 4.0", `{}` → "open a library on the PC"); `FolderDto` with the password dropped; `plan` + snapshot cache (§6.2, §6.3); search via `item/query`; smart folders via `smartFolder/getItems`.
- **Phase 10.** `ItemMutator` with a re-read before writing; `FolderUpdateBody` with the full set of fields; adding files with `path` on the same PC and data URLs from another PC; Eagle thumbnails (same PC, then the helper plugin).
- **Phase 11.** Export into Eagle through its API (§15.7).
- **Phase 12.** Hardening and the Windows release: NSIS per-user installer, WebView2 bootstrapper, optional signing and auto-update.
- **Later, outside the current scope.** More formats (§15.4); the Eagle offline cache (§12.3) and Syncthing mirror (§12.4); sync between devices, remote access to a native library, linked folders, `.eaglepack` (§15.8); macOS, Android and iOS (§4.6). AI features are not planned.

---

## 15. The native library and Eagle compatibility

### 15.1 Goals

- The app works **without Eagle**: its own library in an open, documented format is the main way to use it (D-013).
- The app stays **compatible with Eagle**: it imports Eagle libraries, works with a running Eagle (§4–§11), and exports into Eagle.
- The UI, the query engine (§6) and the edit rules (§7) do not care which kind of library is open.
- The app never writes Eagle's files (§12.2). Eagle stays the only writer of an Eagle library.

Order (spec §9): the native library first (phases 1–7), then the import from Eagle (phase 8), the live connection (phases 9–10) and the export (phase 11). The source abstraction (§15.2) has been part of `library-core` since phase 0, so the Eagle phases add sources, not screens.

### 15.2 Library sources

Everything above the sources (commands, views, the pipeline, the UI) talks to one trait:

```rust
pub trait LibrarySource: Send + Sync {
    fn capabilities(&self) -> Capabilities;

    async fn info(&self) -> Result<LibraryInfo, SourceError>;
    async fn folder_tree(&self) -> Result<Vec<Folder>, SourceError>;
    async fn load_scope(&self, scope: &Scope, hints: &ScopeHints, progress: Progress<'_>) -> Result<Vec<ItemLite>, SourceError>;
    async fn search_ids(&self, text: &str) -> Result<SearchMatch, SourceError>;
    async fn item(&self, id: &ItemId) -> Result<Option<Item>, SourceError>;
    async fn tags(&self) -> Result<Vec<Tag>, SourceError>;
    async fn tag_groups(&self) -> Result<Vec<TagGroup>, SourceError>;
    async fn smart_folders(&self) -> Result<Vec<SmartFolder>, SourceError>;
    async fn thumbnail(&self, id: &ItemId) -> Result<Option<Thumbnail>, SourceError>;
    async fn original(&self, id: &ItemId) -> Result<Option<Thumbnail>, SourceError>;   // the full-size preview
    async fn changes_since(&self, token: &ChangeToken) -> Result<Changes, SourceError>;

    // Writes. A read-only source returns SourceError::ReadOnly.
    async fn apply_item_edit(&self, id: &ItemId, edit: ItemEdit) -> Result<Item, SourceError>;
    async fn add_files(&self, files: Vec<NewFile>, target: AddTarget) -> Result<Vec<ImportTicket>, SourceError>;
    async fn save_folder(&self, change: FolderChange) -> Result<Vec<Folder>, SourceError>;
    async fn rename_tag(&self, from: &str, to: &str) -> Result<(), SourceError>;
    async fn merge_tags(&self, source: &str, target: &str) -> Result<(), SourceError>;
    async fn delete_tag(&self, name: &str) -> Result<(), SourceError>;
    async fn save_tag_group(&self, change: TagGroupChange) -> Result<Vec<TagGroup>, SourceError>;
    async fn delete_tag_group(&self, id: &str) -> Result<Vec<TagGroup>, SourceError>;
    async fn extensions(&self) -> Result<Vec<ExtensionCount>, SourceError>;
    async fn save_smart_folder(&self, change: SmartFolderChange) -> Result<Vec<SmartFolder>, SourceError>;
    async fn delete_smart_folder(&self, id: &str) -> Result<Vec<SmartFolder>, SourceError>;
}

pub struct Capabilities {
    pub writable: bool,
    pub change_feed: bool,       // native: yes; Eagle folder: through mtime.json; Eagle API: no
    pub delete_items: bool,      // emptying the trash: native only
    pub delete_folders: bool,    // native only; Eagle's API has no folder delete
    pub write_comments: bool,    // native: yes; Eagle API: only with a rect or a video time
    pub locked_folders: bool,
}
```

(The sketch uses `async fn` in a trait; the code uses the `async-trait` crate so that `dyn LibrarySource` works (D-008). Write methods default to `ReadOnly`.)

The trait grows with the phases: folder delete and locks (phase 4), tag delete and tag group writes (phase 5), smart folder writes (phase 6), restore, permanent delete and emptying the trash (phase 7). Every new write method gets a default that returns `ReadOnly` or `NotSupported`, so existing sources keep compiling, and a matching `Capabilities` flag tells the UI.

| Source | Crate | Used for | Writes |
|---|---|---|---|
| Native library | `library-native` | The main way to use the app (§15.3) | Directly; the app is the writer |
| Eagle library folder | `eagle-folder` | The import (§15.6); later read-only browsing and the Syncthing mirror (§12.4) | Never |
| Running Eagle | `eagle-api` | The live connection of §4–§11 | Through the Eagle API; Eagle is the writer |

- `ScopeHints` carries the chips of the query. The Eagle source uses them for safe server prefilters on large libraries (§6.2). The native source ignores them: one SQL query per scope is fast enough.
- `ItemId` is an opaque string: Eagle ids for the Eagle sources, ULIDs for the native library.
- `SourceError` (in `library-core`) is one flat set of kinds for every source: the Eagle kinds of §4.4 plus `ReadOnly`, `NotSupported` and `Storage`. Phase 1 adds `NotALibrary`, `NewerFormat`, `UnsupportedLocation` and `AlreadyExists` for creating and opening native libraries. `EagleError` maps onto it one to one. It reaches the UI as `{ kind, detail }` (decisions.md, D-004).
- The UI reads `capabilities` and hides what the open library cannot do (for example "Delete folder" exists only for the native library).
- One window has one open library (D-024). `LibraryInfo` carries `id`, `readOnly` and `readOnlyReason` (`otherInstance`, `syncFolder`, `readOnlyMedium`) so the UI can explain a read-only library. The Welcome screen (spec §7, D-017) creates or opens a native library, imports an Eagle library or connects to a running Eagle.

### 15.3 The native library format

Requirements: open and documented; the user's files stay ordinary files; a crash never corrupts the library; fast with 100 000+ items; versioned, with migrations.

```
My Library.eglib/               the folder suffix is a convention (D-016); the app recognises library.db
  library.db                    SQLite in WAL mode: the single source of truth for metadata
  files/7Q/01JB…7Q/IMG_1042.jpg originals: shard by the last two characters of the id, one folder per item, the original file name
  files/.tmp/                   files being copied in; emptied when the library opens (§7.4)
  thumbs/7Q/01JB…7Q.webp        thumbnails: regenerable, may be deleted at any time
  backups/library-2026-10-28.db copies (VACUUM INTO): daily, before a migration, by hand, before a restore; the last 10 kept
  library.lock                  held with an OS file lock while a writer has the library open; the holder is in meta.open_by
```

Decisions:

- **SQLite is the source of truth.** No per-item JSON files: they duplicate the data and drift apart (Eagle's `mtime.json` problems in §12.2 come from exactly that). Other tools can read the database while the app writes (WAL allows readers). The schema and the protocols are in [`library-format.md`](library-format.md).
- **Originals are copied into `files/`** (D-014), as Eagle does, so the library is self-contained and portable. Later options: hard links instead of copies for imports from the same NTFS volume (saves space; both names point at the same data, and the UI says so), and linked folders that index files where they are (§15.8).
- **Ids** are ULIDs: time-ordered and unique without coordination. Items that came from Eagle keep the Eagle id in `eagle_id`. Ids are stored as `TEXT`; `item` also has an explicit `seq INTEGER PRIMARY KEY` that keys `item_fts`, because implicit rowids change on `VACUUM` and in backups.
- **Tags are case-sensitive strings**, as in Eagle (`"Poster"` and `"poster"` are different), stored once in `tag` and linked through `item_tag`.
- **Folder order** among siblings is a `sort_key` string (fractional indexing): moving a folder changes one row.
- **Trash** is `deleted_at`. Emptying the trash deletes the rows and the files, the files after the rows under the `pending_file` journal. Unlike Eagle's API, the native library can delete items, folders and tags.
- **Renaming an item renames its original** under the journal: the new name is hard-linked (or copied) beside the old one before the row changes, and the old one goes after (D-050). The stored name follows the item name, made safe for Windows.
- **Locked folders** encrypt their content at rest (D-018, `docs/age-locked-folders.md`). Each locked folder has its own password, Argon2id salt, age X25519 identity, and recovery secret. Nothing is shared, and unlocking one folder loads only that identity into memory (`secrecy`) for the session. Argon2id wraps the identity; the password is not stored. A second wrap uses the recovery secret, which is shown once at creation (128-bit key, the same bits as a word phrase, or an optional file outside the library) and is not stored in the library. Originals under `files/`, thumbnails under `thumbs/`, and the item columns name, annotation, url, exif, and palette are age ciphertext. The age header, including nonces, stays inside the ciphertext. Each locked folder has its own user-editable name (`folder.name`) and tag — a label and a colour (`label`, `label_color`) — stored in plaintext on the folder row. They are not encrypted fields, and the tag is not an item tag. The left sidebar has an expandable Locked folders section that lists each locked folder with that name and tag. Choosing a row prompts for that folder only and does not unlock any other folder. Also plaintext on the row: id, lock metadata, salt, KDF parameters, and the wrapped identities. An item in two locked folders has its own item key, wrapped separately by each folder identity (`item_key_wrap`). A folder imported locked from Eagle has the locked flag and no identity until the user sets a password; its files stay plaintext until then (§15.5). An optional YubiKey 5C adds a third wrap of the same identity (`age-plugin-yubikey`, D-056): the card holds an ECDSA P-256 key in a retired PIV slot (82–95), and the age X25519 identity stays wrapped in `folder_piv_wrap`, one row per card. The user either touches the key or enters its PIN, not both. Several cards can wrap one folder. Absent the key, the password and the recovery secret still unwrap it. App login (spec §13) is a separate gate: exactly one of password, PIN, or WebAuthn. WebAuthn is a YubiKey 5C touch that asserts a stored credential. It does not unwrap folder identities. This is not encryption of the whole database, of settings, or of the Eagle token. The columns and tables of locked folders (`item.encrypted`, `item.sealed`, the lock columns of `folder`, `item_key_wrap`, `folder_piv_wrap`) exist from format version 1 and stay empty until phase 4. While an item is encrypted, its five sensitive fields are one age message in `item.sealed`, and the plaintext columns are empty (D-019).
- **Smart folders** keep Eagle's rule language (`conditions` with `property`, `method`, `value`) as JSON, so they survive import and export unchanged. The evaluator in `library-core` grows from the v1 subset (§5 of the spec).
- **Search** finds any part of a word without case, as Eagle does. An FTS5 table with the `trigram` tokenizer covers the item's own texts (name, annotation, url) for terms of three or more characters; a shorter term reads the rows, an encrypted item of an unlocked folder is matched on its opened fields, and tags and folder names are matched on their own tables, so renaming a tag or a folder rewrites no index. `library-core` parses the Eagle syntax (`OR`, parentheses, a leading `-`) and combines the terms (format version 3, D-047).
- **Changes**: every write appends a row to `change_log`. It drives `changes_since`, view updates and, later, the sync between devices.
- **One writer.** `library.lock` is held with an OS file lock (`File::try_lock`), which the OS releases when the process ends; a second app instance opens the library read-only and writes nothing (D-020). **Local disks only:** cloud sync tools (OneDrive, Dropbox, Syncthing and similar) copy a live database mid-write, and WAL does not work over a network share. Creating a library there is refused, a library found in a sync folder opens read-only, and a network path is refused (spec §3, D-021). Use the backups, or later the app's own sync (§15.8).
- **Versioning**: `PRAGMA application_id` (`EGLB`) marks the file, and `PRAGMA user_version` is the format version (D-019). Migrations run forward only, after an automatic backup.
- **Access** through `rusqlite` with the bundled SQLite (check that FTS5 is enabled in the build), on a blocking thread pool.

The full schema of format version 1, the connection settings, and the protocols for the lock, opening, recovery, backups and adding a file are in [`library-format.md`](library-format.md). The tables: `meta`, `item`, `folder`, `item_folder`, `tag`, `item_tag`, `folder_tag`, `tag_group`, `tag_group_tag`, `smart_folder`, `comment`, `change_log`, `pending_file` (the journal of adding a file), the FTS5 table `item_fts`, and the reserved `item_key_wrap` and `folder_piv_wrap`.

`load_scope` is one query per scope (for example `item JOIN item_folder` for a folder, `NOT EXISTS item_folder` for Uncategorized, `deleted_at IS NULL` everywhere), producing the same `ItemLite` list that the pipeline and the views (§6.4, §6.5) already use. Pushing filters and sorting into SQL is a later optimisation, not a requirement.

### 15.4 Media pipeline (native library only)

Eagle generates thumbnails, palettes and metadata itself; the native library needs its own pipeline, in the `media` crate.

**v1 formats** (D-015): JPEG, PNG, GIF (first frame), WebP, SVG, TIFF, BMP, and the composite image of PSD files. AVIF waits for a decoder (D-035). Every other file is accepted and stored, with an extension placeholder instead of a thumbnail; its `width`, `height` and palette stay empty.

| Step | Tools |
|---|---|
| Hash | `blake3`, streaming while copying; duplicates are reported on import |
| Probe | `image` for dimensions of the v1 formats, `nom-exif` (or `kamadak-exif`) for EXIF; later `ffprobe` for video and audio |
| Thumbnails | `image` (with `zune-jpeg` inside) + `fast_image_resize` (Lanczos3) for JPEG, PNG, GIF, WebP, BMP, TIFF, with the EXIF orientation applied; `resvg` for SVG, rendered at thumbnail size with system fonts; the `psd` crate for PSD composites; encoded as lossy WebP (quality 80) by `webp` (libwebp, D-036). Later: `ffmpeg` (a bundled LGPL build, run as a separate process) for video frames, RAW, PDF (`pdfium`) and fonts |
| Thumbnail size | 512 px on the long edge (sharp on high-DPI grids), WebP |
| Palette | k-means on a 64×64 copy, 5–8 colours with ratios, the same JSON shape as Eagle |
| Workers | CPU work on a `rayon` pool sized to the cores, IO on `tokio`; progress through the import channel |

The pipeline is idempotent: deleting `thumbs/` and running "Rebuild thumbnails" restores everything from the originals. If decoding fails, the item is still added, with an extension placeholder.

libvips is the alternative with the widest format coverage and the best speed, at the cost of shipping its DLLs. Start with the pure Rust set and add libvips only if real files demand it.

### 15.5 Compatibility with Eagle

| Level | What | How | Phase (spec §9) |
|---|---|---|---|
| 1. Import | Convert an Eagle library into the native format | `eagle-folder` → `library-native` (§15.6) | 8 |
| 2. Live | Browse and edit a running Eagle | `eagle-api` source (§4–§11) | 9–10 |
| 3. Export | Put native items, folders, tags and smart folders into Eagle | The Eagle API (§15.7) | 11 |
| Later | Open an Eagle `.library` folder read-only without importing; "follow Eagle" re-import; `.eaglepack` | `eagle-folder` source | Later |
| — | Edit Eagle's files directly | — | **Never** (§12.2) |

Field mapping, Eagle → native:

| Eagle | Native |
|---|---|
| item `id` | `item.eagle_id`; the native id is a new ULID |
| `name`, `ext`, `size`, `width`, `height`, `duration`, `annotation`, `url` | the same columns |
| `star` (absent means 0) | `star` |
| `modificationTime` | `added_at` |
| `btime`, `mtime` | `file_created_at`, `file_modified_at` |
| `palettes` | `palette` (the same JSON) |
| `tags` | `item_tag`, case preserved |
| `folders` | `item_folder`, ids mapped through `folder.eagle_id` |
| `isDeleted: true` | skipped by default; option: import into the trash |
| `comments` (rect or video time) | `comment` |
| `rawMetas` (EXIF strings), `bpm` | `exif` (JSON), `bpm` (correction 38) |
| `order` (manual position per folder) | `item_folder.position` (correction 39) |
| `_thumbnail.png` | copied as the first thumbnail when present (a faster import), otherwise generated |
| `lastModified` (on disk only) | kept in the import state for incremental re-import |
| folder tree, order of `children` | `folder.parent_id`, `sort_key` in the same order |
| folder `iconColor`, `description`, `tags`, `orderBy`, `sortIncrease`, `coverId` | the same fields |
| folder `password` | **not imported**: the folder gets `locked = 1` with no salt and no identity; files stay plaintext and the content stays hidden until the user sets a password or removes the lock |
| `extendTags` | not stored; computed from the tree |
| smart folder `conditions` | `smart_folder.conditions`, unchanged |
| `tagsGroups` | `tag_group` + `tag_group_tag` |
| `quickAccess`, `saved-filters.json`, `tags.json` | later, optional |

### 15.6 Import from an Eagle library folder

Read-only, and Eagle may be closed. If Eagle has this library open, reading is still safe, but a file can be caught mid-write: read with retries, and treat `mtime.json` as the manifest of what to read.

1. Check the folder: `metadata.json`, `mtime.json`, `images/`.
2. Read `metadata.json`: folders (password fields dropped, §5), smart folders, tag groups.
3. Read `mtime.json`: the ids and their `lastModified` values form the import manifest.
4. For each id, in a bounded parallel pool: read `images/<id>.info/metadata.json`, find the original by extension, hash it, copy (or hard-link) it into `files/`, take `_thumbnail.png` if present, insert the rows. Retry a JSON parse error, because Eagle writes these files in place (correction 37). Never use Eagle's cache in `%APPDATA%` (correction 36).
5. Commit in transactions of 500 items. Record each imported `eagle_id` with its `lastModified` in an import state table, so an interrupted import resumes where it stopped.
6. Report: imported, skipped (missing originals, unreadable metadata), duplicates by hash.

**Incremental re-import ("follow Eagle", later).** Compare `mtime.json` with the import state: new ids are added, larger `lastModified` values are re-read, missing ids go to the trash. While a native library follows an Eagle library, the items that came from Eagle are read-only in the app; "Unlink" makes the library independent. This avoids two-way conflicts, which Eagle's whole-item "newest wins" model (§12.2) cannot resolve.

**`.eaglepack` files** (later) are a second import source with the same mapping: `pack.json` replaces `metadata.json` and `mtime.json`, and the `.info` directories come from the archive (§17).

### 15.7 Export into Eagle

Only through the API of a running Eagle, so that Eagle stays the writer of its library:

1. Folders top-down with `folder/create`, then `folder/update` with the full set of fields where needed (§7.2); map ids; re-read the tree.
2. Items. On the same PC: batch `item/add { items: [{ path, name, tags, folders, annotation, website, modificationTime, star }] }`, up to 1000 items per call. Verified live: batch `path` items keep tags, folders, notes, the source link, `modificationTime` and `star`, and unknown folder ids are dropped (correction 35). From another computer: data URLs one by one, with the size limit (§7.4).
3. Wait for the items to appear (§7.4). Only for data URLs: then `item/update` for `star`, which that mode ignores (correction 11).
4. Comments with a rect or a video time through `item/addComment`; tag groups through `tagGroup/create`; smart folders through `smartFolder/create` with `conditions` unchanged.
5. Locked folders cannot be created through the API: they are exported unlocked, with a warning.
6. Each new Eagle id is stored in `eagle_id`, so a second export updates instead of duplicating.

Without a running Eagle, the app can write a `.eaglepack` instead (§17). Eagle imports it with new ids and a new date added, so it suits a one-time hand-over, not repeated exports that update earlier ones.

### 15.8 Later

- **Sync between devices** for native libraries, built on `change_log` with per-field "newest wins" (hybrid logical clocks). That is finer than Eagle's whole-item rule and does not depend on file sync tools.
- **Remote access to a native library**: a small HTTP server (`axum`) in the app that exposes the `LibrarySource` operations, so another computer (and later a phone) browses it the way the client browses Eagle today.
- **Linked folders**: index files where they are, without copying them into the library.
- `.eaglepack` import and export (the format is in §17).

### 15.9 Verification results (2026-10-28)

All four open questions were checked: live where the API allows it, otherwise in the code extracted from `app.asar` with REA.

| Question | Answer | How |
|---|---|---|
| Does batch `item/add` with `path` keep the fields? | Yes: tags, folders (unknown ids dropped), notes, source link, `modificationTime` and even `star`. Single mode loses `star` and `modificationTime`. Eagle copies the file; the source stays (correction 35) | Live: three test files, then moved to the trash |
| Where does Eagle cache libraries outside `.library`? | `%APPDATA%\Eagle\library-caches\<FNV-1a of the library path>.txt` and `eagle-backup\<same id>.json` (correction 36). Item and library `metadata.json` are written in place, so readers must retry on parse errors (correction 37) | Code (`background.js`) and the files on this PC |
| Smart folder rule semantics | Full reference with every quirk in §16 | Code (`existInSmartFilter`, `isMatch*Rule`, `smart-folder-rules.js`) |
| `.eaglepack` structure | An uncompressed ZIP with `pack.json` and the items' `.info` directories (§17) | Code (export and import in `background.js`) |

Still open:

- `.eaglepack` files were not exported live: Eagle creates them only from its UI. Export one pack by hand and compare it with §17 before writing packs.
- The smart folder reference comes from code only. Compare a few real smart folders through `smartFolder/getItems` with the local evaluator before relying on it.
- Locked folders remain unverified (the API cannot set a password).

---

## 16. Smart folder rules (reference)

Source: `existInSmartFilter` (`app.bundle.js:32145`), the `isMatch*Rule` functions (`app.bundle.js`, about lines 8340–9400), the schema in `js/plugin/handlers/smart-folder-rules.js` and the file type lists in `config.js`. Read from code, not run live. The quirks are listed on purpose: the local evaluator must reproduce them, or smart folders will show other items than the desktop.

### 16.1 Combination

- A smart folder has `conditions: [{ match: "AND" | "OR", boolean: "TRUE" | "FALSE", rules: [ … ] }]`.
- An item matches when **every** condition matches (AND across conditions).
- Inside a condition, `match: "AND"` needs all rules and `"OR"` any rule; a missing `match` behaves as AND.
- `boolean: "FALSE"` negates the result of its condition.
- A nested smart folder also requires the conditions of its **parent** smart folder, recursively.
- An exception during evaluation means "no match".
- Limits checked by the API: 1–30 conditions, 1–30 rules in each.

### 16.2 Strings: `name`, `url`, `annotation`, `folderName`, `camera`, `comments`

Methods: `equal`, `startWith`, `endWith`, `contain`, `uncontain`, `empty`, `not-empty`, `regex`.

- `name`, `url`, `annotation`, `folderName` and `camera`: the field and the value are both lowercased, so the comparison is case-insensitive.
- `comments`: the notes of all comments joined without a separator, **not** lowercased: `equal`, `contain`, `uncontain` and `regex` are case-sensitive, `startWith` and `endWith` are not (flag `i`).
- `startWith` and `endWith` build a regular expression from the value **without escaping** (`^value`, `value$`): `.`, `(`, `+` and similar characters act as regex syntax.
- `regex` uses `new RegExp(value, "g")`. For the lowercased properties the **pattern is lowercased too**, so `\D`, `\W`, `\S` and `\B` turn into `\d`, `\w`, `\s` and `\b`. An invalid pattern means "no match".
- An empty value with any method except `empty` and `not-empty` means "no match".
- A missing field counts as `""`. Exceptions: `folderName` and `camera` never match an item without folders or without `rawMetas.camera`, not even with `empty`.
- `folderName` matches when any of the item's folders has a matching name.

### 16.3 Numbers: `width`, `height`, `fileSize`, `duration`, `bpm`, `iso`, `aperture`, `focalLength`, `shutter`

Methods: `=`, `>=`, `<=`, `>`, `<`, `between` (inclusive on both ends). The value is `[n]` or `[min, max]`.

- A missing field never matches.
- `fileSize`: `unit: "kb"` divides bytes by 1024; any other unit (`"mb"`) by 1024².
- `duration` is in seconds; `unit: "m"` or `"h"` converts.
- `iso` is `parseInt(rawMetas.isoSpeed)`. `aperture` and `focalLength` take the first number in their `rawMetas` string. `shutter` takes the **second** number of `rawMetas.shutter` (250 for `"1/250"`).
- `=` compares floats exactly, so `fileSize = …` in KB or MB rarely matches.

### 16.4 Dates: `createTime`, `mtime`, `btime`, `timestamp`

Methods: `on`, `before`, `after`, `between`, `within`. Values are milliseconds.

- `createTime` is the **date added** (`modificationTime`). `mtime` and `btime` fall back to `modificationTime`. `timestamp` reads `rawMetas.timestamp` (capture time).
- `on`: the same calendar day in the **desktop's local time zone** (`toDateString()`).
- `before`: `t <= value + 1 day`. `after`: `t >= value`. `between`: `min <= t <= max + 1 day` (the end day counts). `within`: `t + value[0] days >= now`.

### 16.5 Sets: `tags`, `folders`

Methods: `union`, `intersection`, `equal`, `identity`, `empty`, `not-empty`.

- `intersection`: every value is present. For `tags` an empty value never matches; for `folders` an empty value matches every item that has a `folders` key.
- `equal`: intersection plus the same length. `union`: at least one value. `identity`: none of the values.
- An item without the `tags` (or `folders`) key never matches, not even `empty`.
- Strings are compared exactly, case-sensitive.

### 16.6 Special properties

- **`type`** (`equal`, `unequal`). The value is an extension (`jpg`, `psd`, …) or a category: `video` / `videos` (ts, 3gp, 360, afx, vap, eva, mp4, mov, m4v, webm, mkv, avi, wmv, mpg, mts, flv, m2ts, f4v), `audio` (mp3, wav, flac, ogg, aac, m4a), `font` (ttf, otf, ttc, woff, woff2), `powerpoint` (ppt, pptx, potx), `presentation` (ppt, key, pptx), `excel` (xls, xlsx), `word` (doc, docx), `url`, `youtube`, `vimeo`, `bilibili` (bookmarks: `ext: "url"` plus `medium`). The schema also offers `base`, `raw`, `riff` and `office`, but the matcher has no category for them: they only match an extension of that literal name.
- **`rating`** (`equal`, `unequal`, `contain`). A missing star and 0 both mean "no rating". `equal "none"` matches unrated items; `equal "3"` matches three stars. `contain` checks the star as a substring of the value string (`"3,4,5"`); if the value contains `none`, **every item matches**.
- **`shape`** (`equal`, `unequal`). `square` when width equals height; `landscape` or `portrait` otherwise; `panoramic-landscape` or `panoramic-portrait` when the long side is at least 2.5 times the short one. `custom` compares the item's `width / height` with the rule's own `width / height` exactly. Without dimensions there is no shape: `equal` fails and `unequal` matches.
- **`color`** needs `palettes`. `grayscale`: every palette entry with `ratio >= 0.02` has all channel differences below 8. `similar` and `accuracy`: the dominant colour must cover at least 33 % and lie within ±96 of the target in every channel. Then the item matches if palette entry 0 or 1 equals the target exactly, or if entry 0 or 1 (with a ratio above 33) is within ΔE2000 < 20 and ΔE76 < 70 (`accuracy`: < 10 and < 60).
- **`fontActivated`** depends on the fonts installed on the desktop PC. The client cannot evaluate it.

### 16.7 Consequences

- Until the local evaluator covers every property and quirk, open Eagle smart folders through `smartFolder/getItems`, which evaluates on the desktop. The plan already does this.
- The same rules power `item/get` filters (correction 40): `ext` accepts the categories above, and `rating: 0` matches unrated items.
- The native library stores `conditions` unchanged (§15.3) and evaluates them with `library_core::SmartRules`. The v1 subset and its differences from the desktop are in D-048: `on` uses the day start the editor computed in the user's time zone; `startWith` and `endWith` are literal, not regular expressions; `regex`, `color`, `folderName`, `camera`, `comments`, EXIF numbers, `duration`, `bpm`, custom shapes and `fontActivated` are not evaluated yet, never match, and are listed so the UI warns.

---

## 17. The `.eaglepack` format

Source: the export and import code in `js/background.js` (about lines 4430–4720 and 4880–5120). Not exported live, because Eagle creates packs only from its UI.

**Structure.**

- A ZIP archive **without compression** (`archiver('zip', { store: true })`) with the extension `.eaglepack`.
- `pack.json` at the root: `{ "images": [ … ], "folder": { … } }`.
  - `images`: full item objects as in `images/<id>.info/metadata.json`. Only runtime fields (`$$hashKey`, `thumbnailPath`, `rawPath`, `encodeName` and similar) are removed. Sorted by `modificationTime`, newest first.
  - `folder`, only in folder packs: the exported folder with its `children`, in the format of the library `metadata.json` (`id`, `name`, `description`, `children`, `modificationTime`, `tags`, `extendTags`, `icon`, `iconColor`, `pinyin`, `password`, `passwordTips`, `coverId`, `orderBy`, `sortIncrease`). **The folder password (base64) is included.** Folder packs skip trashed items.
  - Selection packs have no `folder`. Every item's `folders` is `[]`; trashed items are included with `isDeleted` removed.
- One `<id>.info/` directory per item, copied as is: the original, `_thumbnail.png` when present, `metadata.json`.
- Items whose names contain characters that are invalid on Windows are left out.

**Import into Eagle.**

- Eagle offers to skip duplicates. It finds them by a metadata fingerprint, not by content (correction 41).
- Every item and every folder gets a **new id**. `folders` and `coverId` are remapped, and the folder tree is created as a new tree.
- `modificationTime` (date added) is reset to the import time. The import order follows `order[<old folder id>]` (correction 39), otherwise the old `modificationTime`.
- The `.info` directories are moved into `images/`, and `metadata.json` is rewritten with the new id.

**Consequences.**

- The app can **write** `.eaglepack` files for an export without Eagle running (§15.7). Items keep tags, notes, ratings, comments, palettes and folders, but get new ids and a new date added, so repeated exports duplicate instead of updating.
- The app can **read** `.eaglepack` files as an import source, with the mapping of §15.5 (§15.6).
- A pack the app writes never contains real folder passwords: locked folders are exported without a password.
- Before writing packs, export one pack from Eagle by hand and compare it with this section.
