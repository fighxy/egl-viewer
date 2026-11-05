# Decisions

Decisions taken during development, newest last. Each one says what was decided, why, and when
to revisit it. [`spec.md`](spec.md) says what the app does, [`architecture.md`](architecture.md)
how it is built; this file records the choices made while building it.

## D-001 Working name and identifier (2026-10-28)

- Name: **egl-viewer** (working name). Tauri identifier `com.fighxy.eglviewer`.
- Repository folder on the development PC: `C:\Users\kirkw\dev\egl-viewer`.
- Renaming later touches `Cargo.toml` (`egl-viewer` package), `src-tauri/tauri.conf.json`
  (`productName`, `identifier`, window title), `ui/index.html`, `package.json` names and the
  footer text. Changing the identifier changes where the app keeps its data on disk.

## D-002 Latest stable versions, pinned exactly (2026-10-28)

Phase 0 started from the newest stable releases available that day, subject to D-003:

| Area | Versions |
|---|---|
| Rust | 1.99.1, pinned in `rust-toolchain.toml`, edition 2024 |
| Tauri | `tauri` 2.12.5, `tauri-build` 2.7.4, `@tauri-apps/cli` 2.12.4, `@tauri-apps/api` 2.12.4 |
| Rust crates | `serde` 1.0.229, `serde_json` 1.0.154, `thiserror` 2.0.21, `ts-rs` 12.0.1, `async-trait` 0.1.92, `tracing` 0.1.44, `tracing-subscriber` 0.3.23, `tokio` 1.54.1 |
| UI | React 19.3.1, TypeScript 7.0.2, Vite 8.3.5, `@vitejs/plugin-react` 6.1.2, Tailwind CSS 4.3.3, shadcn 4.22.1 |
| Tests | Vitest 5.0.5, jsdom 30.1.3, Testing Library (React 16.3.3, DOM 10.4.2, jest-dom 7.0.1, user-event 14.6.8) |
| Tooling | Node.js 24.22.1 (LTS), pnpm 12.15.0, Biome 2.6.3 |

- Cargo: exact versions in `[workspace.dependencies]`, `Cargo.lock` committed.
- npm: exact versions (no `^`) in `package.json`, `pnpm-lock.yaml` committed.
- **Tauri 2.12, not Tauri 3.** Tauri 3 is at 3.0.0-alpha.5 and breaks APIs between alphas
  (architecture.md §2). Revisit when 3.0 is stable.
- Updating a dependency is a deliberate change: update, run `pnpm check`, record it here.

## D-003 Supply-chain protection stays on (2026-10-28)

pnpm 12 refuses packages younger than a minimum release age (about a day by default). When exact
versions published the day before were requested, pnpm wrote `minimumReleaseAgeExclude` entries
into `pnpm-workspace.yaml`. Those entries were removed: the project takes **the newest version
that is older than that age**. On 2026-10-28 this meant shadcn 4.22.1 instead of 4.23.0 and
lucide-react 1.61.0 instead of 1.62.0. The same rule applies to crates when updating by hand
(all crates of D-002 were at least six days old).

## D-004 One flat error type for every source (2026-10-28)

`SourceError` in `library-core` is a single set of kinds (`unauthorized`, `rejected`, `network`,
`readOnly`, `storage`, …) shared by all library sources, instead of wrapping a per-source
`EagleError`. The UI maps `kind` to messages without knowing which kind of library is open, and
`eagle-api` (phase 9 since D-013) maps Eagle's errors onto it one to one. architecture.md §15.2 is updated
accordingly.

## D-005 Biome instead of ESLint (2026-10-28)

TypeScript 7.0.2 (the native compiler) is the current release, but `typescript-eslint` 8.72.1
supports only TypeScript below 6.1, and TypeScript 7 no longer ships the JavaScript compiler API
that it needs. The UI therefore uses **Biome** for linting and formatting (TypeScript, TSX, CSS,
JSON) and `tsc -b` from TypeScript 7 for type checking. Revisit when `typescript-eslint` supports
TypeScript 7, if a rule only ESLint has becomes necessary.

## D-006 UI kit: Tailwind CSS 4 and shadcn/ui on Radix (2026-10-28)

- shadcn/ui with the **Radix** primitives (shadcn 4 also offers Base UI and React Aria), the
  **Nova** preset, the neutral base colour, Lucide icons and the Geist variable font (bundled, no
  network). Components live in `ui/src/components/ui/` and belong to the project.
- shadcn uses the `cn` package for class merging. It is published by the shadcn author
  (`shadcn-ui/cn` on GitHub) and replaces `clsx` + `tailwind-merge`.
- Alternatives considered: Mantine 9 (more ready-made components), Fluent UI React 9 (native
  Windows 11 look), React Aria Components (accessibility first).
- More components are added with `pnpm dlx shadcn@<pinned> add <name> -c ui` when a screen needs
  them.

## D-007 UI text in English for now (2026-10-28)

~~All UI strings are English until a localisation phase adds Russian and English through a
message catalogue. New strings go through one place per screen, so extracting them later is
mechanical. Confirmed on 2026-10-28: the first version ships in English only; localisation is in
the "Later" list of spec §9.~~

**Superseded by D-069** (2026-10-31): English remains the default and the source catalog;
Russian and Chinese (Simplified) are selectable in Settings.

## D-008 `async-trait` for `LibrarySource` (2026-10-28)

The trait has to be usable as `dyn LibrarySource`, because the open library is chosen at run
time. Native `async fn` in traits is not dyn-compatible, so the trait uses `async-trait`.
Revisit when Rust supports dyn-compatible async traits.

## D-009 IPC types generated by ts-rs and committed (2026-10-28)

Types that cross IPC derive `ts_rs::TS` and are written to `ui/src/ipc/generated/` by
`cargo test` (`.cargo/config.toml` sets the directory; 64-bit integers become `number`, which
is safe for millisecond timestamps). The files are committed; CI fails when they differ from
what the Rust code generates. Do not edit them by hand.

## D-010 Libraries added when a phase needs them (2026-10-28)

Phase 0 has a single screen, so TanStack Router and TanStack Query (architecture.md §8) are not
installed yet. They arrive with the first phase that needs routing or server state (phase 1 or
3). The same holds for the store plugin (phase 1; dropped by D-022), and for HTTP (`reqwest`) and the secret store
(`keyring`), which come with the Eagle connection (phase 9 since D-013).

## D-011 Desktop-only crate type (2026-10-28)

`src-tauri` builds only an `rlib` (faster builds). Mobile targets will add `staticlib` and
`cdylib` when they come into scope.

## D-012 Fixtures live in `fixtures/eagle-api/` (2026-10-28)

The 49 captures from 2026-10-27/28 were copied unchanged. A test in `crates/eagle-api/tests`
checks that they stay valid JSON and contain no token and no unredacted folder password.
`library-info*.json` contain the library path of the development PC
(`C:\Users\kirkw\OneDrive\Pictures\ggwp.library`); scrub it before the repository becomes public.

## D-013 Standalone app first, Eagle compatibility second (2026-10-28)

egl-viewer is a **standalone asset manager** with its own library. Connecting to Eagle is a
secondary feature. Until now the plan was a client for a running Eagle, with an own format as a
late addition (phases 14–16). The spec and the architecture were re-prioritised the same day:

- Phases 1–7 build the native library: create and open, adding files and the media pipeline, the
  grid, folders, tags, search and smart folders, editing and trash.
- Then Eagle, in the order users need it: import an Eagle library (phase 8), connect to a running
  Eagle (phases 9–10), export into Eagle (phase 11). Hardening and the release are phase 12.
- The Eagle protocol content, the 41 corrections and the fixtures are unchanged; they moved to
  spec §8. The `LibrarySource` trait from phase 0 makes this a reordering, not a rewrite.
- The phase-0 code stays: the Connect screen becomes one of the Eagle screens, reached from
  Welcome (D-017).

## D-014 A managed library: files are copied in (2026-10-28)

Adding a file copies it into the library folder, as Eagle does. The library is self-contained:
it can be moved, backed up or copied to another PC as one folder, and the app never depends on
files elsewhere. Hard links for imports from the same NTFS volume and linked folders (indexing
files where they are) are later options (architecture.md §15.3, §15.8). Revisit if users with
very large collections on other drives need linked folders earlier.

## D-015 v1 formats: images, including PSD previews (2026-10-28)

The v1 media pipeline reads dimensions and makes thumbnails and palettes for JPEG, PNG, GIF,
WebP, AVIF, SVG, TIFF, BMP and the composite image of PSD files, with pure Rust crates
(architecture.md §15.4). Any other file can still be added; it is stored and shown with an
extension placeholder, so imports from Eagle never drop files. Video (bundled `ffmpeg`), audio,
PDF, fonts and RAW come later. (AVIF previews are postponed: D-035.)

## D-016 Library folders end in `.eglib` (2026-10-28)

A new library is a folder `Name.eglib`. The suffix tells people (and the folder picker) what the
folder is; the app itself recognises a library by `library.db` and the application id and
format version in its header (D-019), not by the name, so a renamed folder still opens. Revisit together with the product name (D-001).

## D-017 Welcome is the first screen (2026-10-28)

With no library open the app shows **Welcome**: Create library, Open library and Recent
libraries as the primary actions; Import an Eagle library and Connect to a running Eagle as
secondary actions under "Eagle". The phase-0 Connect screen is reached from there and has a Back
button. On start the app reopens the last library unless it was closed explicitly (phase 1).

## D-018 Locked folders encrypt their content with age (2026-10-28)

A locked folder is not only hidden in the UI. Its originals (`files/`), thumbnails (`thumbs/`),
and the sensitive item columns (name, annotation, url, exif, palette) are age ciphertext. Each
locked folder has its own password, Argon2id salt, age identity, and recovery secret. Argon2id
wraps the identity; the recovery secret wraps it a second time and is shown once at creation
(a 128-bit key, the same bits as a word phrase, or an optional plaintext file outside the
`.eglib` folder). The secret itself is not stored. Unlocking one folder does not unlock the
others. An item in two locked folders has its own item key, wrapped separately by each
folder identity, so folder A's password does not unlock folder B. Each locked folder has its own user-editable name and tag (a label and a colour),
stored in plaintext on the folder row. The left sidebar has an expandable "Locked folders"
section that lists them with that name and tag. Choosing a row prompts for that folder only.
The whole database, settings, and the Eagle token are not encrypted. The token stays in
Windows Credential Manager. An optional YubiKey (via `age-plugin-yubikey`) is a third
wrap of the same identity: the card holds an ECDSA P-256 key in a retired PIV slot
(82–95), not the X25519 identity itself. Touch and the slot PIN are alternatives, not
two steps (D-056). Either one unlocks that folder when the key is present; the password
and the recovery secret still work when it is absent. Several keys can wrap one folder.
Detail: `docs/age-locked-folders.md`. Phase 4. Revisit if a
full-database cipher is ever required; it is out of scope here.

## D-019 Schema v1 and how a library is recognised (2026-10-28)

- `PRAGMA application_id = 1162300482` (ASCII `EGLB`) marks the file as an egl-viewer library;
  `PRAGMA user_version` is the format version and the only version number. Before, the docs named
  both `user_version` and `meta.format_version`: two sources of truth.
- A library from a newer format version is refused. Considered and not taken: a second
  `min_reader_version` that lets older apps read additive changes, and a list of required
  features. Revisit if one library is often used by different app versions.
- The version goes up when the meaning of stored data changes, not only the schema: the first
  locked folder raises it although its columns already exist, because an older app would add
  plaintext files to a locked folder.
- The storage of locked folders is reserved in version 1 and stays empty until phase 4:
  `item.encrypted`, `item.sealed`, the lock columns of `folder`, and the tables `item_key_wrap`
  and `folder_piv_wrap`. `item.sealed` holds the five sensitive fields as one age message while
  the plaintext columns are empty. Ciphertext in the same columns was rejected: it breaks
  `NOT NULL`, `STRICT` types, sorting by name and the FTS index. `folder_piv_wrap` replaces the
  single `folder.piv_wrap` of `age-locked-folders.md`, because one folder can have several cards.
- What cannot change without rewriting the file is fixed at creation: `STRICT` tables,
  `auto_vacuum = INCREMENTAL`, and `item.seq INTEGER PRIMARY KEY` as the key of `item_fts`
  (implicit rowids change on `VACUUM` and `VACUUM INTO`). `CHECK` constraints only for invariants
  that never change.
- `synchronous = FULL`: a commit survives power loss, which adding a file relies on (D-023);
  metadata writes are small.
- `meta.name` is the display name; renaming the folder by hand does not rename the library.
- Detail: `library-format.md`.

## D-020 The library lock is an OS file lock (2026-10-28)

- A writer holds `library.lock` with `std::fs::File::try_lock` (stable since Rust 1.89, no
  `unsafe`). The OS releases it when the process ends, so a crash or a kill never leaves a stale
  lock. This replaces the JSON lock file with a heartbeat every minute and a five-minute stale
  rule; Windows reuses process ids quickly, which made "the process is gone" unreliable.
- On Windows the lock is mandatory and nobody else can read the file, so the holder (computer,
  process id, since) is in `meta.open_by`, set after taking the lock and removed on a clean close.
  A free lock with `open_by` set means an unclean close; the next writer runs `PRAGMA quick_check`.
- `library.lock` is never deleted.
- A read-only instance writes nothing (no `open_by`, no cleanup of `files/.tmp/`, no recovery,
  backups or migrations), reloads when `PRAGMA data_version` has changed on window focus, and
  offers "Open for editing", which retries the lock.
- Rejected: SQLite's `locking_mode = EXCLUSIVE`, which in WAL mode shuts other processes out
  completely, readers included.

## D-021 Libraries live on local disks outside sync folders (2026-10-28)

- Cloud sync tools copy a live SQLite database mid-write, and WAL does not work over a network
  file system. On the development PC, Pictures is already inside OneDrive (D-012): Windows'
  OneDrive folder backup moves Desktop, Documents and Pictures.
- New libraries default to `%USERPROFILE%\egl-library`. OneDrive does not move the profile root,
  and the app never puts a library into OneDrive on its own. The Create dialog can pick another
  local folder every time. The default itself is a preference in `app-store` that the user can
  change in Settings → General (phase 1, D-027), with the same location check. Changing it does
  not move existing libraries.
- Detection is best effort, in `app-store` behind a trait: the `OneDrive`, `OneDriveConsumer` and
  `OneDriveCommercial` variables; the sync roots registered with the Windows Cloud Files API
  (`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager\*\UserSyncRoots`:
  OneDrive, recent Dropbox, iCloud, Box); the markers `.dropbox.cache` and `.stfolder` in the
  folder or an ancestor; a network path when the canonical path is a UNC path. Google Drive and
  Yandex Disk are not guaranteed; add them when a reliable signal is known. The registry reader is
  a new dependency, chosen and recorded when the phase 1 code lands.
- Create is refused in a sync folder, on a network path, and inside another native library or an
  Eagle `.library` folder. Open: a library in a sync folder opens read-only with `immutable=1`
  (SQLite writes no `-wal` or `-shm` there) and a message to move the folder out while the app is
  closed; a network path is refused. No override in v1; "Move library…" can come with the library
  settings (phase 7).
- This replaces "warn and offer the backups" in spec §3.

## D-022 Settings and recent libraries are app-store files (2026-10-28)

- `app-store` writes small JSON files in the app's local data folder
  (`%LOCALAPPDATA%\com.fighxy.eglviewer`; not Roaming, because the paths belong to one computer):
  re-read, change, write a temp file, rename. Several app processes (D-024) merge instead of
  overwriting each other, and the code is tested without Tauri.
- `tauri-plugin-store` is not used (D-010 expected it in phase 1): it keeps a file in memory and
  saves it whole, so a second process overwrites the first one's changes, and it would add a
  JavaScript store API to the capabilities.
- Recent libraries are keyed by the canonical path, because a copied library has the same
  `library_id`. Each entry has the path, name, library id and last opened time; 20 entries, newest
  first. A library that is gone is shown as missing and kept, since a USB drive comes back.
- Start: `lastLibrary` and `closedExplicitly`; quitting is not an explicit close. The last library
  is not reopened when another instance has it open; Welcome says so. `Shift` is read in Rust at
  startup, because the WebView cannot see a key held before it loads; `--no-reopen` does the same
  for tests.

## D-023 Adding a file: a journal before the rename (2026-10-28)

- `pending_file` records the target path before the rename; the item insert and the journal
  delete are one transaction; the writer resolves leftover rows on open. A row never points at a
  missing file, and no file is left without a row, without walking `files/`. Before, such orphans
  were left to "Check library".
- One pass copies and hashes. The temp file is flushed (`sync_all`) before the rename, and the
  rename is retried for up to a second on sharing violations (antivirus scanners hold new files
  briefly). The source size is compared before and after the copy.
- An item in the trash counts as a duplicate and is reported as such.
- The stored file keeps the original name with the part before the extension cut to 100 UTF-16
  units: paths over 260 characters break Explorer and other apps. `name` keeps the full name.
- `library-native` gets `add_file` for one file in phase 1, with tests and without a command or
  UI, so the schema is exercised before the grid. Batches, the media pipeline and the UI stay in
  phase 2. Later: when no item has the same size, the file cannot be a duplicate, which saves the
  lookup for large imports.

## D-024 Several processes, one window and one library each (2026-10-28)

- No single-instance plugin: a second app process may run, and on the same library it is
  read-only (the phase 1 done check). Two libraries at the same time means two processes until
  multi-window support comes.
- Switching libraries in a window opens and checks the new one first and closes the current one
  only on success.
- `AppState` holds `Option<OpenLibrary>` behind one accessor, so a map per window is a local
  change later.
- `LibraryInfo` gains `id`, `readOnly` and `readOnlyReason` (`otherInstance`, `syncFolder`,
  `readOnlyMedium`). `Capabilities.locked_folders` is false for the native library until phase 4.
  App login (spec §13.2) stays outside phase 1; Welcome remains the entry route, so a gate can be
  put in front of it later.

## D-025 Native window frame, the app's own menu bar, one command registry (2026-10-28)

- Phase 1 keeps the native Windows frame and puts the app's own menu bar (shadcn `Menubar`) under
  it. Snap Layouts, the system shadow and the dark title bar come from Windows.
- Rejected: the native Tauri menu, whose classic Win32 menu bar does not match the shadcn look and
  the dark theme. Postponed to phase 12: a custom title bar that merges the title and menu rows.
  Its HTML caption buttons lose the Snap Layouts of Windows 11 unless a plugin such as
  `tauri-plugin-decorum` restores them (a new dependency, to be checked then). Moving the menu bar
  into a custom title bar moves the component, not the menu.
- One command registry (`ui/src/commands/`) feeds the menu bar, context menus, shortcuts, the
  Keyboard Shortcuts dialog and a later command palette. Commands of later phases are absent, not
  disabled; a menu without items is hidden; write commands in a read-only library are disabled
  with the reason as a tooltip.
- The menu (spec §7.1): File, Edit, View, Library, Help. Closing the window quits the app: one
  window, no tray icon (quit confirmation: D-072). Close Library returns to Welcome.

## D-026 Library screen: left bar, grid, right bar with metadata (2026-10-28)

- Layout (spec §7.3): menu bar; left bar (200–360 px); toolbar and filter bar over the grid;
  right bar with metadata (280–480 px); status bar. Both bars resize, toggle (`Ctrl+B`, `Ctrl+I`)
  and remember their state. Below about 1200 px the right bar slides over the grid.
- Left bar: fixed rows, folders, locked folders, smart folders, tag groups and recent tags. The
  full tag vocabulary is in the Tags filter, not in the left bar, because it grows to thousands of
  tags. Choosing a tag opens All with that tag as a filter. The Include subfolders switch sits in
  the Folders header (and the View menu), because it changes the tree's counts.
- Filters: a filter bar under the toolbar, shown by the Filter button and on its own while a
  filter is active. Rejected: an always visible bar (it costs a row most of the time) and filters
  in the right bar (that belongs to the selection).
- Grid: justified rows from item aspect ratios (D-067; phase 3 shipped squares first). List
  unchanged. Masonry can still come later. Double-click and `Space` open the full-size preview
  (`Space` hold-vs-toggle and letterbox click-out: D-073); `Enter` moves focus to the right bar
  (spec §7 said before that `Enter` opens the detail).
- Right bar: fields save on `Enter` or blur, `Esc` reverts, no Save button. With several items it
  shows batch edits; with nothing selected, the open folder's properties (from phase 4).
- Phase 1 builds the whole frame with empty zones, so later phases fill zones instead of
  restructuring the screen.

## D-027 Settings and About are dialogs (2026-10-28)

- Settings is a dialog (`Ctrl+,`) with sections on the left, not a screen and not a second window
  (one window, D-024). Phase 1 has General: theme, reopening the last library on start, and the
  default folder for new libraries (D-021). Library settings stay a separate dialog in the
  Library menu.
- About shows the version, build date and commit, the WebView2 version, links and the
  third-party licences. The licence file is generated by the release build (phase 12), because
  the MIT and Apache licences of the dependencies require showing their texts. Open Log Folder in
  the Help menu waits for logs written to a file (phase 12).

## D-028 Look and motion (2026-10-28)

- The theme follows Windows by default, with a System / Light / Dark override in the View menu and
  in Settings. The grid background is a neutral grey. Desktop density: rows about 28–32 px, body
  text 13–14 px.
- Not now: Mica or Acrylic backgrounds of Windows 11. They need a transparent window and WebView,
  a performance risk with a large grid; revisit during phase 12.
- Motion: `opacity` and `transform` only, 120–200 ms, for menus, dialogs, panels, thumbnails
  fading in, and the preview growing out of its cell (View Transitions API). No motion on scroll,
  sort, filter, selection or hover. Everything off under `prefers-reduced-motion`.
- No animation library: `tw-animate-css` (already a dependency), CSS transitions and View
  Transitions are enough. Revisit if gestures in the full-size preview need more.

## D-029 Desktop behaviour in the WebView (2026-10-28)

- The app must not feel like a web page: the WebView2 context menu is replaced by the app's own;
  reload, printing, the browser's find, back navigation and page zoom are off (`Ctrl` + wheel
  changes the thumbnail size); chrome text is not selectable; buttons show the arrow cursor;
  focus rings only for keyboard focus.
- The window starts hidden and appears after the first paint (no white flash in the dark theme);
  its size, position and maximized state are saved by `app-store` (D-022).
- Which of these are Tauri or WebView2 settings and which are UI code is settled when phase 1 is
  built; arch §8 lists them.

## D-030 Dependencies of phase 1 (2026-10-28)

The newest versions older than the minimum release age (D-002, D-003):

| Where | Package | Version | Why |
|---|---|---|---|
| Rust | `rusqlite` with `bundled` | 0.41.0 | SQLite with WAL and FTS5 for `library.db` |
| Rust | `ulid` | 3.0.0 | Library and item ids (`Ulid::generate`) |
| Rust | `blake3` | 1.8.7 | Content hashes of added files |
| Rust, Windows only | `winreg` | 0.56.0 | Reading the sync roots of the Windows Cloud Files API (D-021) |
| Rust | `tauri-plugin-dialog` | 2.8.3 | The folder picker for Create and Open. 2.8.4 was published on 2026-10-28, too new |
| Rust, tests | `tempfile` | 3.27.0 | Temporary folders in tests |
| npm | `@tauri-apps/plugin-dialog` | 2.8.3 | The plugin's JavaScript side, same version as the crate |

The `std::fs::File::try_lock` of the library lock needs no crate (Rust 1.89+).

## D-031 shadcn components written by hand when the registry is unreachable (2026-10-28)

The shadcn CLI downloads components from `ui.shadcn.com`, which the build environment of phase 1
could not reach. `menubar`, `dialog` and `switch` in `ui/src/components/ui/` were written by hand
on the `radix-ui` primitives already installed, in the style of the existing Nova components. They
belong to the project like CLI output (D-006); `pnpm dlx shadcn@<pinned> add <name> -c ui` can
replace them when the registry is reachable. No new npm package was needed.

## D-032 Phase 1 as built (2026-10-28)

- `library-native` exposes `Library` directly; the commands use it through `src-tauri/src/session.rs`,
  a Tauri-free layer that the tests drive. The native `LibrarySource` implementation arrives in
  phase 3 with `load_scope`, when there is something to load.
- Holding `Shift` on start (D-017, D-022) is not implemented yet: reading the key state needs a
  Windows call, and `unsafe` is forbidden in our crates, so it needs a crate with a safe wrapper,
  chosen separately. `--no-reopen` and the Settings switch "Reopen the last library on start" work.
- Daily backups are named by the UTC date. The library's display name is `meta.name`.
- The left bar shows its four rows without counts; counts come with the queries of phase 3. The
  screens switch without TanStack Router (D-010): two screens do not need it yet.
- A read-only open of a library that needs a migration is refused with `notSupported`; with only
  format version 1 this cannot happen yet.
- Checked on Linux under Xvfb with the real app: create, quit, restart (reopened), a second
  instance (Welcome notice, then read-only with Open for editing), `kill -9` and restart (lock free,
  unclean close detected). The Windows checks of the done check are still open.

## D-033 Dependencies of phase 2 (2026-10-28)

The newest versions older than the minimum release age (D-002, D-003):

| Where | Package | Version | Why |
|---|---|---|---|
| `media` | `image`, default features off: `bmp`, `gif`, `jpeg`, `png`, `tiff`, `webp` | 0.25.10 | Decoding raster images and the EXIF orientation; no AVIF encoder (it would pull `rav1e`) |
| `media` | `fast_image_resize` | 6.1.0 | Lanczos3 downscaling with alpha handled |
| `media` | `resvg` | 0.49.0 | SVG rendering, system fonts for text |
| `media` | `psd` | 0.3.5 | The composite image of PSD files |
| `media` | `libheif-rs`, default features off: `v1_17` | 2.6 | HEIC/HEIF through the system libheif (D-063) |
| `media` | `kamadak-exif` | 0.6.1 | EXIF from JPEG, TIFF, PNG and WebP |
| `media` | `webp`, default features off | 0.3.1 | Lossy WebP thumbnails through libwebp (D-036) |
| `library-native` | `rayon` | 1.12.0 | The thread pool for previews (at most four threads) |
| app | `arboard` | 3.7.0 | Reading copied files and images from the clipboard in Rust, so no bytes cross the WebView |

## D-034 Media test files are made in code (2026-10-28)

Architecture §13 planned small binary files in `fixtures/media/`. Instead, `media::fixtures` (behind
the `fixtures` feature, used by the tests of `media`, `library-native` and the app) makes every v1
format on the fly: JPEG with an EXIF block, PNG, GIF, WebP, TIFF, BMP, SVG, and a PSD written byte by
byte. Every seed gives different pixels, so 1 000 files never collide as duplicates. The repository
holds no binary fixtures, and a test can make as many files as it needs.

## D-035 AVIF previews are postponed (2026-10-28)

There is no safe, pure Rust AVIF decoder to use yet: `image` decodes AVIF only through the C
library `dav1d` installed on the system, and `rav1d` offers dav1d's C interface, which would need
`unsafe`. AVIF files are added like any other file, with an extension placeholder, and the add dialog
still lists them under "Images". Revisit when a safe decoder exists, or decide to build `dav1d`
in CI. This narrows D-015.

## D-036 Thumbnails are lossy WebP through libwebp (2026-10-28)

The WebP encoder of `image` writes lossless files only: a 512 px photo becomes several hundred
kilobytes, about ten times a lossy one, which would make `thumbs/` of a large library many
gigabytes. Thumbnails are encoded at quality 80 by the `webp` crate, which builds libwebp from C
(like the bundled SQLite). Opaque images are encoded without an alpha channel.

## D-037 Phase 2 as built (2026-10-28)

- **The duplicate question:** the first duplicate of a batch pauses the import and asks "Skip
  duplicates", "Add anyway" or "Cancel import"; the answer holds for the rest of the batch. "Add the
  existing item to the target folder" joins with folders (phase 4).
- **Waiting for later phases:** mirroring dropped folders (phase 4), tags and rating for a batch
  (phases 5 and 7). The add dialog is the system file picker without extra options.
- **Paste** is Edit → Paste (`Ctrl+V`), new in the menu table of spec §7.1. Text fields keep their
  own `Ctrl+V`. A pasted image is named `Pasted YYYY-MM-DD HH-MM-SS.png` in UTC; the time zone
  needs a dependency that phase 2 does not otherwise need.
- **Previews** run on a pool of at most four threads. `thumb_version = -1` marks a file that cannot
  be decoded, so it is not tried again; previews that a crash or a cancel left undone are made the
  next time the library opens, as a "Previews" task that does not hold back quitting.
- **Quitting or closing while adding:** the window's close is held back and the UI asks
  (`close-requested`); closing or switching the library asks the same in the UI. D-072 extends
  that confirmation to every quit, not only while imports or exports are running.
- **Counts** of the left bar and the toolbar arrive in phase 2 (D-032 expected them in phase 3):
  with files going in, the screen needs to show that they did.
- **Checks:** 1 020 files (1 000 previews, 20 PDF placeholders) in 2.6 s in a release build on Linux
  (the ignored test `a_thousand_mixed_files`). In the real app under Xvfb: pasting an image from
  the clipboard, adding files through the GTK file dialog, the Imports panel and the duplicate
  question. The Windows checks are still open.

## D-038 Dependencies of phase 3 (2026-10-28)

The newest versions older than the minimum release age (D-002, D-003):

| Where | Package | Version | Why |
|---|---|---|---|
| `library-core` | `icu_collator` (compiled data) | 2.4.0 | Natural, case-insensitive order for titles, extensions and tags, through sort keys (arch §6.4) |
| UI | `@tanstack/react-virtual` | 3.14.23 | The virtualized grid and list |
| UI | `@tanstack/react-query` | 5.105.0 | Windows of cells from Rust, invalidated by `views-invalidated` |

TanStack Router is still not needed: the library screen and Welcome switch by state, and the
preview is a layer, not a route (D-010, D-032). Biome formats `pnpm-lock.yaml`, so after
`pnpm add` the lockfile is formatted with Biome to keep its diff to the new entries.

## D-039 Views and selections (2026-10-28)

- **Every `open_view` is a new view.** Reusing a view for an equal query let one screen close
  another's view: React runs effects twice in development, so the first effect's late close
  removed the view the second effect was showing. Four views are kept; the UI closes the previous
  one once the new one is open, so the grid never flashes empty on a new sort.
- **Snapshots** (six per library) are keyed by scope. After an import writes, every snapshot is
  dropped and the open views are recomputed under their ids, at most every 500 ms and once at the
  end, before the final progress goes out. Patching snapshots per item comes with edits (phase 7).
- **The selection is ids**, not positions: positions move when an import adds items above. Shift
  ranges and the rubber band ask Rust for the ids of a range (`view_ids`); Select All is a flag
  ("everything except these ids"), never a list of every id. A plain click selects on mouse
  down, as desktop apps do. A new scope clears the selection; a new sort keeps the selected
  items and drops the anchor.
- **Keys inside the grid** (arrows, Home, End, Page Up and Down, Enter, Ctrl + wheel) and inside
  the preview are handled by those widgets, as in the ARIA grid pattern; the Keyboard Shortcuts
  dialog lists them from `WIDGET_KEYS` in the registry. Everything with a menu item, including
  Select All and Preview (`Space`), is a registry command. Space and Enter on a focused button,
  link or field belong to that control.
- **Shortcuts read the latest state.** The keyboard listener reads the state and actions from a
  ref updated in a layout effect, so a key pressed right after a change never acts on the state
  before it (found as a flaky test: Space opened the item selected before the click).

## D-040 View Transitions only in WebView2 (2026-10-28)

The preview grows out of its cell with the View Transitions API (D-028). On Linux, WebKitGTK
crashed the app process (SIGSEGV) at `document.startViewTransition` under Xvfb with compositing
turned off. The transition now runs only in Chromium (WebView2), the target of v1; elsewhere the
preview simply appears. Revisit for macOS (WKWebView) when that target starts.

## D-041 Phase 3 as built (2026-10-28)

- **`library-core`** has the pipeline of arch §6.4 in full (lock filter, subfolder union, tag and
  folder chips, type, rating, shape, search ids, sort), although folders, tags and search arrive
  in phases 4–6: the core tests of spec §10 already run against it. An empty tag or folder
  selection filters nothing, whatever the mode. Shapes follow Eagle (arch §16.6), including the
  panoramic ones.
- **The native `LibrarySource`** reads scopes, the item, tags, thumbnails and originals. Folders,
  tag groups and smart folders answer empty lists until their phases; search answers
  `notSupported` until FTS5 (phase 6). The app reads the async trait on a blocking thread with a
  single poll (the native source never waits); a waiting source (Eagle, phase 9) needs a runtime
  there.
- **The sort menu** shows ten keys. Duration (no video in v1) and Manual (folders, phase 4) are
  supported by the sorter but hidden. The global sort, the layout (grid or list), the thumbnail
  size (80–320 px) and Show Names are preferences (`BrowsePreferences`); the last query per
  library comes with phase 12.
- **Thumbnail size** is two View menu items, Larger and Smaller (`Ctrl+=`, `Ctrl+-`), plus the
  slider and `Ctrl` + wheel; the item at the top stays in place when the size changes. Images
  smaller than their cell are not enlarged.
- **The details panel** is read-only: preview, palette, name, rating, tags, note, link, file info
  and EXIF. Several selected items show their count; the stack of thumbnails and the total size
  come with batch edits (phase 7).
- **The preview** shows originals the WebView can display (JPEG, PNG, GIF, WebP, AVIF, BMP, SVG)
  and the 512 px thumbnail for TIFF and PSD. It zooms to 800 % with the wheel or `+` / `−`, pans
  by dragging, and moves with the arrow keys.
- **Checks:** a library of 10 000 items (made with the ignored test `demo_library`) in the real
  app under Xvfb: scrolling with every thumbnail size, End, selection, list, preview, sort by
  title and at random, Trash. 100 000 titles sort in 0.2 s, at random in 0.05 s (the ignored test
  `sorting_100k_items`, release build). The Windows checks are still open.

## D-042 Dragging inside the window with mouse events (2026-10-28)

- **Folders in the tree, items onto folders and items into a manual order** are dragged with
  mouse events, not HTML5 drag and drop. With the window's file drop on, WebView2 on Windows
  hands HTML5 drags to Tauri's file drop handler and the page never gets `drop`. One
  `DragProvider` starts a drag after the pointer moved 5 px, reads the target under the pointer
  from `data-drop` attributes (`elementFromPoint`), highlights only targets that accept the drag,
  draws a label at the pointer, and `Esc` cancels. Files dragged in from Explorer still come
  through Tauri's file drop.
- **A press on a selected item keeps the selection**, so all of it can be dragged; a click
  without a drag then selects that item alone, as in Explorer.
- **The browser's context menu is blocked after the page saw the event** (the bubbling phase on
  the window), not before it (capture): Radix menus ignore an event that is already prevented,
  so the folder tree's menu never opened. A UI test found it.

## D-043 Phase 4a as built (2026-10-28)

Phase 4 is built in two parts: **4a** (folders, membership, Include subfolders, the folder sort,
manual order) and **4b** (locked folders: age, Argon2id, the recovery secret, item keys, the
Locked folders section). 4a's done check is phase 4's without the locked-folder part.

- **Folder order** among siblings is a fractional key (base 62) between the neighbours
  (`order_key_between`); siblings with equal keys (an import) are numbered again first. A move
  carries the full position (parent and index); a folder update always carries every property,
  as Eagle needs (corrections 3, 31).
- **Deleting** a folder is one transaction: the subtree, its memberships, and with the option
  the items found only in the subtree go to the trash.
- **Dropped folders are mirrored:** a dropped folder becomes a library folder of the same name
  inside the target (at the top level when no folder is open); a folder of that name already
  there is reused. Its subfolders go below it and each file into the folder it came from, with
  the folder tags of that folder and its ancestors.
- **A duplicate going into a folder** can join that folder instead ("Add to <folder>" in the
  batch question). An item in the trash joins it and stays in the trash; restoring comes with
  phase 7.
- **Manual order** is `item_folder.position`. A reorder numbers the folder's items 1..n again;
  the moved items keep their order among themselves, and a drop before one of them lands before
  the next item that stays. **An item that joins a folder gets no position:** it sorts after the
  positioned items, by date added, as Eagle falls back (correction 39). Looking up the last
  position on every join made adding 10 000 items to a folder take a minute in a debug build;
  now 0.3 s.
- **Reordering in the grid** works while the folder's sort is Manual ascending. With Include
  subfolders on it works only in a folder without subfolders (their items have no position
  there); the folder's properties say so.
- **Sort picks inside a folder:** Manual in a folder without its own sort becomes that folder's
  sort (ascending), since Manual means nothing elsewhere; any other pick changes the global
  sort. In a folder with its own sort, a pick holds for that screen only; the folder's sort
  changes in its properties (spec §5).
- **Include subfolders** is a preference (`BrowsePreferences.includeSubfolders`), off by default.
  **Which folders are expanded** is a convenience of the window, kept per library in the
  WebView's storage; `library.db` holds none of it.
- **Membership edits** are `ItemEdit` intents (`addToFolders`, `removeFromFolders`,
  `setFolders`) on one item or on a batch. A batch target is ids, or Select All as "every item
  of this view except these ids" (`ItemsTarget`), resolved in Rust. The other intents answer
  "not available yet" until phases 5 and 7.
- **New Folder** (`Ctrl+Shift+N`, the Library menu) creates a folder at the top level and opens
  it; New Subfolder, Rename (`F2`, double click), Colour, Move To and Delete are in the tree's
  context menu. A folder deleted while it is open answers an empty view, and the screen goes
  back to All.
- **Not yet:** `item_fts.folders` is written with search (phase 6); covers come with the folder
  covers of phase 12; removing a batch from a folder waits for the batch panel (phase 7).
- **Checks:** the 10 000-item demo library in the real app under Xvfb: creating, nesting by
  drag, colour, dragging 12 items onto a folder, Select All into a folder, Include subfolders,
  Manual with a drag to the front, the context menu, deleting a folder. The Windows checks are
  still open.

## D-044 Dependencies of phase 4b (2026-10-28)

The newest versions older than the minimum release age (D-002, D-003), with one exception:

| Where | Package | Version | Why |
|---|---|---|---|
| Rust | `age` (no default features) | 0.12.1 | Files, previews, sealed fields and item keys of locked folders (D-018) |
| Rust | `argon2` | 0.6.0 | Argon2id of a folder password |
| Rust | `chacha20poly1305` | 0.10.1 | XChaCha20-Poly1305 for the two wraps of a folder identity. 0.11.0 is newer, but `age` 0.12 uses 0.10.1: one copy instead of two |
| Rust | `secrecy` | 0.10.3 | Passwords as `SecretString` (the copy `age` re-exports) |
| Rust | `zeroize` | 1.9.1 | Wrapping keys, the recovery secret and decrypted key text are wiped when dropped |
| Rust | `bip39` (std only) | 3.0.0 | The recovery secret as 12 English words |
| Rust | `getrandom` | 0.4.3 | Salts, nonces and recovery secrets; already in the tree through `uuid` |

Dev builds and tests optimise the crypto crates (`[profile.dev.package]` in `Cargo.toml`):
unoptimised, one Argon2id of 64 MiB and the X25519 of every item took seconds.

## D-045 Phase 4b as built: locked folders (2026-10-28)

- **A lock covers its folder and every folder under it.** An item in any covered folder is
  encrypted; an item covered by no lock is plaintext. A locked folder that is not unlocked hides
  its items, and every item in one of its folders, in every scope, in the counts, the tag counts
  and the duplicate check (which would otherwise tell that a locked folder holds a file). In the
  left bar it appears only under Locked folders (D-059), not in the Folders tree.
- **Keys.** Each lock: a random age X25519 identity, wrapped twice with XChaCha20-Poly1305 (a
  version byte, a 24-byte nonce, the ciphertext). The password wrap's key is Argon2id of the
  password (64 MiB, 3 passes, 1 lane, a 16-byte salt, both stored per folder); the recovery
  wrap's key is BLAKE3 `derive_key` of the 128-bit recovery secret. Each wrap is bound to its
  folder id and its role, so it cannot be moved to another folder or used in the other role.
  Each item: its own X25519 identity, age-encrypted to every covering lock (`item_key_wrap`);
  the original, the preview and the sealed fields are age messages to the item.
- **The recovery key** is written as 26 Crockford base32 characters plus 2 check characters in
  groups of four (`XXXX-…-XXXX`); case, spaces, dashes and O/I/L are forgiven, a typo is caught
  by the check characters. The same bits are 12 BIP39 words. It is shown once with Copy and
  Save to a file (refused inside the library folder); the dialog closes only after "I saved the
  recovery key".
- **Every write that changes what a lock covers needs that lock unlocked:** adding items or
  files to a covered folder, removing them, moving a folder in or out, deleting a folder with a
  lock in or above it, and locking a folder whose items another lock already encrypts. The write
  is first simulated, and refused before anything changes (`Locked { folder }` names the folder
  to unlock); then it is written and the items are encrypted, decrypted or rewrapped. A crash
  in between leaves the items hidden; the next unlock of that folder finishes the work.
- **Encrypting an item** writes the ciphertext through `files/.tmp/`, journals and renames it to
  `files/<shard>/<id>/<id>.age` (no file name: it is sealed), commits the row (plaintext columns
  emptied, the search row deleted, wraps inserted) with the old plaintext paths in the journal,
  then deletes them. Decrypting is the reverse, back to the original file name. Previews go to
  `thumbs/<shard>/<id>.age`; the journal also takes `thumbs/` paths. Files added to an unlocked
  locked folder are encrypted while they are copied: no plaintext lies in `files/.tmp/`.
- **Locking** puts the lock on at once, encrypts with progress, and ends with the folder locked
  (its key forgotten). Removing a lock decrypts first and drops the identity last, so an
  interrupted removal loses nothing.
- **Format version 2** comes with the first lock, after a `-v1` backup (D-019); its schema is
  version 1's. Libraries without locks stay at 1. `FORMAT_VERSION` is the newest version this
  build reads; the schema version is separate.
- **In memory:** unlocked identities, item keys and opened fields live in the library's keyring
  for the session and are wiped on Lock Now, Lock All Folders (`Ctrl+L`) and closing. A
  read-only second instance may unlock and read, and changes no lock. Decrypted images go to
  the WebView with `Cache-Control: no-store`; views are recomputed on every lock change, so no
  snapshot keeps a decrypted name.
- **The lock's tag** (a caption of up to 60 characters and a colour) is plaintext on the folder
  row and editable while the folder is locked; the Locked folders section of the left bar lists
  every locked folder with it.
- **Not in 4b:** the YubiKey PIV wrap waited for a card. D-056 adds it. The table
  `folder_piv_wrap` was empty until then. A folder imported locked from Eagle (no identity) comes with
  phase 8.
- **Checks:** the demo library in the real app under Xvfb: lock a folder of 12 items (also in
  another folder), the recovery key dialog, the counts and the Locked folders section, a wrong
  password, unlocking (the folder opens with decrypted previews and originals), Lock All
  Folders, removing the lock (the original file names come back). The Windows checks are still
  open.

## D-046 Phase 5 as built: tags (2026-10-28)

- **A tag is one `tag` row**, named by its exact string; items, folders and tag groups link to
  it. Adding a tag makes the row; a write that leaves a row without links (removing a tag from
  items, editing or deleting a group) deletes it, so the table holds no dead names.
- **Rename and merge are one operation.** Renaming onto a free name updates the row. Renaming
  onto a name that is already a tag merges the two, as Merge Into does: the links move to the
  other row and the old one goes. The Rename dialog says "already exists: the two tags will be
  merged" before it happens. Only an exact match merges: `poster` → `Poster` is a rename unless
  `Poster` exists too. Merge Into accepts only existing tags.
- **Everywhere means every item**, the trash and the items of locked folders included: a rename
  or a delete changes every link in one transaction and gives those items a new modification
  time. Item tags are not sealed (D-045 seals name, note, link, EXIF and palette), so this
  needs no lock open and writes no files. Tagging, by contrast, only reaches items the user can
  see: an item hidden by a lock is left alone, like a missing one, because a selection can
  outlive a Lock Now.
- **The vocabulary** is every tag on a visible item (the trash included, with a count of 0 when
  only trashed items carry it) and every tag kept in a group. A tag that only hidden items carry
  stays out, so a locked folder does not show through its tags. Counts leave out the trash and
  hidden items.
- **Tag order** everywhere (vocabulary, groups, the tags of a selection) is the name collation
  with a byte-order tie-break (`compare_tags`): `Poster` before `poster`, and lists never depend
  on where they came from. The names collation alone treated the two as equal.
- **The tags of a selection** (`selection_tags`) are counted in Rust from the open view, so
  Select All needs no id list over IPC. The right bar dims a tag on only some items ("On 2 of 5
  items"); a click on it puts it on every selected item.
- **Edits show at once.** One item: the chip appears or goes before the answer and the stored
  item replaces it; a failed write refetches the item and shows the error. A selection: the
  counts change at once and are read again after the write.
- **Suggestions and the sheet's search ignore case** (`post` finds `Poster` and `poster`), but
  what is picked or typed is kept exactly; the field never changes a name's case. Spaces at
  either end of a typed name are dropped.
- **Filters stay when the place changes** and show in the filter bar, which opens on its own
  while a filter is on; the Filter button counts them. Another library starts without filters.
  A view with no results says so and offers Clear filters. The filter bar has only Tags now;
  phase 6 adds type, rating, shape and folders.
- **The Tags sheet** draws the first 300 matching tags of the vocabulary and says how many more
  there are; typing narrows the list. A virtualized list can come if libraries need it. The
  chosen tags show under the lists, not above them: picking a tag never moves the rows under
  the pointer (the first real-app run caught a misclick from that).
- **Recent tags** are the last 12 tags the user added, per library, kept in the WebView's
  storage like the expanded folders (they may be lost; `library.db` holds none of it). Rename,
  merge and delete update the list. Lists show only recent tags that are in the vocabulary.
- **Left bar:** Tag groups (with ⊕, Edit Group… and Delete Group… in the context menu) and
  Recent tags; choosing a tag opens All filtered by it. Tag rows here and in the sheet have
  Rename…, Merge Into… and Delete… in their context menu (none in a read-only library). Left bar
  sections and groups stay collapsed once collapsed.
- **The search row's tags column** stays empty until phase 6 fills `item_fts` from the tables.
- **Checks:** the 10 000-item demo library in the real app under Xvfb: tagging one item
  (`Poster`, `Swiss` and `poster` as three tags), tagging a selection of three with dimmed tags
  and "add to every item", the four modes on two tags (Any 3, All 3, Exact 2, None 9 997),
  renaming `Poster` onto `poster` (the merge notice, then one tag on every item), a coloured tag
  group, a group tag opening All filtered, removing the filtered tag (the empty result with
  Clear filters). The Windows checks are still open.

## D-047 Phase 6 search and format version 3 (2026-10-29)

- **A search word is a substring**, without case, of an item's name, note, link, tag names or
  folder names, as in Eagle: `post` finds `Poster`, `плак` finds `ПЛАКАТ` (Unicode lowercase on
  both sides; tags keep their case everywhere else). The token search of the old index
  (`unicode61`) found whole words and prefixes only, so it could not behave like Eagle.
- **Format version 3** replaces `item_fts` with `fts5(name, annotation, url, tokenize = 'trigram')`,
  filled again from the items that are not encrypted, after the usual backup. Measured on 100 000
  rows: 5 ms per term through the index against 60 ms for a scan of the rows (release build).
  Every library a writer opens is migrated; a read-only open of an older library is refused as
  before. A new library starts at version 1 and runs the same steps, so the schema has one
  source.
- **What the index cannot answer** is scanned: terms of one or two characters (trigrams start at
  three), and encrypted items of unlocked folders, matched on their opened fields; hidden items
  match nothing. Tag and folder names are matched on the `tag` and `folder` tables and joined
  to items, so renaming a tag or a folder rewrites no index row.
- **The syntax** is parsed in `library-core` (`parse_search`, `SearchExpr`): words AND-ed, `OR`
  in capitals, parentheses, a leading `-` before a word, a phrase or a group, `"…"` for spaces.
  Broken input is forgiven (a missing `)`, a stray `OR`). A search returns a `SearchMatch`: the
  ids it keeps, or for a search that only excludes, the ids it drops, so no list of every item is
  needed. `LibrarySource::search_ids` returns it.
- **The lock step of version 2 is gone.** Version 3 keeps older apps out of every library, so
  the first lock no longer changes the version (D-019, D-045 still explain version 2).
- **A native item always has a folder list**, perhaps empty (`ItemLite.folders = Some`); only an
  Eagle item can lack the key, which matters for Eagle's smart folder rules.
- **The search field** sits in the toolbar: `Ctrl+F` focuses it, typing applies after 300 ms or
  at once on `Enter`, `Esc` clears it. The search stays when the place changes and resets with
  another library; an empty result says so and offers to clear.

## D-048 Phase 6 filters and smart folders (2026-10-29)

- **The filter bar** has Tags, Folders (the four modes over folder ids), Type (Images, then the
  library's extensions with counts from the new `extensions` call), Rating (unrated or 1–5) and
  Shape (Eagle's five shapes, panoramic ones included), Clear all and Save as smart folder.
  Filters stay when the place changes; the Filter button counts them.
- **Smart folders** have their own tree in the left bar, nested, a new one after its siblings
  (moving and reordering wait for a later phase, and so do counts). A smart folder shows the
  items outside the trash that its rules and the rules of every smart folder above it keep,
  hidden items never. Editing or deleting one recomputes its open view; deleting takes the ones
  under it.
- **The evaluator** (`library_core::SmartRules`) covers the v1 subset of spec §5 with Eagle's
  quirks from architecture §16: `before` and `between` count the last day, `within` counts days
  back from now, a missing value never matches, an empty text value matches nothing, an empty
  `intersection` never matches tags and matches every item with folders, `rating contain` with
  `none` matches everything. Where it differs: `on` takes the day start the editor computed in
  the user's time zone (Eagle compares `toDateString()` on the desktop); `startWith` and
  `endWith` are literal, where Eagle builds an unescaped regular expression; `regex`, `color`,
  `folderName`, `camera`, `comments`, EXIF numbers, `duration`, `bpm`, a custom shape and
  `fontActivated` are not evaluated: they never match (Eagle's "an exception means no match"),
  are listed on the smart folder, and the left bar shows a warning. Rules that cannot be read at
  all match nothing. A limit of 30 conditions of 30 rules is checked before saving, as Eagle's
  API does.
- **Save as smart folder** (`rules_from_query`) maps the scope and the chips to one AND
  condition (spec §5). Inside a smart folder the new one goes under it. Search text and the
  trash are refused with the reason, and so is All without a filter. The new smart folder opens
  with the filters cleared, since its rules are now the place. A test checks, for a range of
  screens, that the saved rules keep exactly the items the chips showed.
- **The editor** shows each condition as "Items that match all / any of these rules" with "Leave
  these items out" (`boolean: FALSE`), and each rule as property, method and a value field that
  fits it (tags with suggestions, folders from the tree, text, the type, rating and shape lists,
  dates in the user's time zone or a number of days, numbers with KB or MB for file size). A rule
  it cannot edit stays as it is, marked as not supported, unless it is removed. Unknown keys of a
  rule are kept.
- **A narrow toolbar** (the right bar open on a 1280 px window) drops the thumbnail size slider,
  through a container query; `Ctrl` + wheel and `Ctrl+=` / `Ctrl+-` still change the size. The
  title and the item count never shrink away, and the filter bar wraps instead of cutting
  buttons off. A smart folder saved from filters is named after them (tags, folders, type,
  rating, shape); the name can be changed before saving.
- **Checks:** the 10 000-item demo library in the real app under Xvfb: opening it migrated it
  from version 2 to 3 (a `-v2` backup, 10 000 index rows); `file-999` found 10 items,
  `file-9999 OR file-9990` 2, `file-99 -file-999` 90; a search inside a folder kept to it; the
  Type filter listed the library's extensions with counts; Type GIF (1 250 items) saved as a
  smart folder opened with the same 1 250; adding "Name contains 99" in the editor recomputed it
  to 21. The Windows checks are still open.

## D-049 Dependencies of phase 7 (2026-10-29)

The newest versions older than the minimum release age (D-002, D-003):

| Where | Package | Version | Why |
|---|---|---|---|
| Rust | `tauri-plugin-opener` | 2.7.2 | Open with the default app, Show in Explorer (`SHOpenFolderAndSelectItems` on Windows) and links in the browser. Used as a library: its free functions only. The plugin is not registered, so the WebView gets no opener commands and the UI cannot open a path itself |
| Rust | `drag` | 2.1.1 | Dragging originals out of the window (OLE `DoDragDrop` on Windows, GTK on Linux). It brings `windows` 0.52 next to Tauri's newer `windows`; only a second copy at build time |

`rusqlite` turns on its `backup` feature (no new crate) for restoring a backup into the open
connection (D-051). Copy File uses `arboard`, already there for Paste.

## D-050 Phase 7 item edits, renames and undo (2026-10-29)

- **Name, note, link and rating** are written by `apply_batch_edit` like the other intents, in
  one transaction for a selection, with the search row of a plaintext item. An encrypted item
  gets its sealed fields sealed again; its original keeps its sealed name `<id>.age`.
- **A rename renames the stored original** (spec §6) without a moment where the row points at a
  missing file: the new name is journaled and hard-linked next to the old one, the row commits
  (it also clears the new name from `pending_file` and journals the old one), then the old name
  is removed and leaves the journal. A crash at any step leaves both names or one, and recovery
  on open deletes the name no row points at. A drive without hard links (FAT32, exFAT) gets a
  flushed copy through `files/.tmp` instead. A rename that only changes the case on a file system
  that ignores case renames in place without the journal, because recovery would delete the one
  file under its other spelling. An original that is missing keeps its path.
- **Stored names come from the item name** (`file_name_for`): characters Windows refuses become
  `_`, the stem is cut to 100 UTF-16 units without trailing dots or spaces, a device name such as
  `CON` gets a leading `_`, and the extension is the item's. Decrypting an item uses the same rule,
  so a name typed with `/` can no longer produce a bad path.
- **Undo** keeps the inverse of the last 50 item edits of the session in memory (`UndoStack` in
  `library-core`): before an edit, the session reads the items' state (`item_states`), and the
  inverse is grouped into one batch per distinct edit (each item back to its own rating, name,
  note, link, folder set; tags and folders added are removed again, and only those). `Ctrl+Z`
  applies one entry as new edits; the Edit menu names it ("Undo Rating"). Folder, tag-vocabulary
  and lock operations are not undone, nor permanent deletes; the stack empties when the library
  closes, another one opens, or a backup is restored. Undoing Restore puts items back in the trash
  with a new date.
- **Optimistic fields**: the right bar shows a change at once and rolls back to the stored item
  when Rust refuses it. A field shows its draft only while it is being edited; otherwise it always
  shows the stored value, so a refusal that arrives before the next render is not lost.

## D-051 Phase 7 trash, backups and maintenance (2026-10-29)

- **Delete Permanently and Empty Trash** take only visible items of the trash. One transaction
  deletes the rows (links, tags, comments and key wraps cascade, tags left unused go) and
  journals the original and both preview names; the files are deleted after it commits, each
  leaving the journal once gone. A file that cannot be deleted now (open in another app) is
  deleted by recovery on the next open. Items of a locked folder stay in the trash until it is
  unlocked.
- **Backups** are `library-YYYY-MM-DD.db` (daily), `-vN` (before a migration),
  `-HHMMSS.db` (Back Up Now) and `-HHMMSS-restore.db` (before a restore), all in UTC; the last 10
  of all kinds are kept, by name order.
- **Restoring a backup** keeps the library open and its lock held: the backup is checked
  (egl-viewer's application id, a format this build knows, the same `library_id`,
  `quick_check`), the current state is backed up first (not pruned yet, so the chosen backup
  cannot go), then SQLite's backup API copies the backup's pages into the open connection. The
  writer settings and `meta.open_by` are written again, an older format is migrated after its own
  backup, unlocked folders lock again, views and undo start over. The backup's add journal is
  dropped without deleting files: files added after the backup have no row now, and rows removed
  for good since then point at nothing. Check Library reports both and deletes nothing unless
  asked, because the backup made before the restore still needs those files.
- **Check Library** runs `PRAGMA quick_check`, lists items whose original is missing (names of
  hidden items stay empty) and files under `files/` (outside `.tmp`) and thumbnails that no item
  or journal entry points at. It holds the connection while it walks the folders: a new file is
  journaled before it appears, which needs the connection, so no file being added counts as
  stray. **Repair** deletes stray files and thumbnails, or removes visible items without a file
  for good, only what the user switches on.
- **Rebuild Thumbnails** asks every visible item of a previewable type for its preview again,
  files that could not be read before included, and starts the preview task. A rebuild is marked
  by a negative `thumb_version` (`-1 - version`): the old thumbnail keeps showing under that URL
  until the new one is written, and the new thumbnail continues the old number, so its URL is new
  and the WebView's year-long cache cannot serve the old image. A rebuild that is cut short goes
  on when the library opens again.
- **Library Settings** shows the name (editable: `meta.name` only, the folder keeps its name),
  the location with Show in Explorer, the size on disk (originals, thumbnails, database,
  backups), the backups with Back up now and Restore (a second click confirms), Rebuild
  thumbnails and Check library.

## D-052 Phase 7 file actions and the right bar (2026-10-29)

- **Files leave through Rust only.** Open with default app, Show in Explorer, Copy File and
  dragging out take item ids; Rust finds the original inside the library folder and refuses an
  item of a locked folder, whose original is ciphertext (a decrypted copy would put the file on
  disk in plaintext). Links open in the browser only if they are `http` or `https`.
- **Dragging out** starts from the right bar: the preview of one item, or the stack of thumbnails
  of a selection, after the pointer moves a few pixels. Rust starts an OS drag of the originals
  on the main thread with a 96 px PNG of the thumbnail. The grid keeps its own drag onto folders
  (D-042). Files of the open library dragged back over the window get no drop overlay and are not
  added (the import refuses them anyway).
- **Copy File** keeps one `arboard` clipboard for the whole run: on X11 and Wayland the files stay
  on the clipboard only while it lives; Windows keeps them on its own.
- **The right bar** edits name, rating, note and link in place (Enter or leaving the field saves,
  `Ctrl+Enter` in the note, `Esc` reverts; a link must be a web link). A selection shows a fanned
  stack of its first three thumbnails, its total size (both from the view, in the selection
  summary), a rating for all, tags, Add to folder and the buttons. In the Trash the buttons are
  Restore and Delete permanently, and an item says since when it is there; the list's date column
  shows the date of deletion.
- **Menus and keys**: Edit gets Undo (`Ctrl+Z`), Copy File (`Ctrl+C`) and Move to Trash
  (`Delete`; in the Trash, Delete Permanently…), each confirmed except Undo and Copy. `Delete` in
  a text field or on the folder tree stays theirs. Library gets Library Settings…, Back Up Now,
  Check Library, Rebuild Thumbnails and Show in Explorer. The grid and the list have a right-click
  menu (Preview, Open with Default App, Show in Explorer, Copy File, Move to Trash; in the Trash,
  Restore and Delete Permanently); a right click on an item outside the selection selects it.
  The Trash's toolbar has Empty Trash, only its icon on a narrow toolbar (as the size slider,
  D-048).
- **Fixed on the way:** switching from the list to the grid after starting in the list gave
  zero-width cells. The scroll container moved in the tree between the two modes, so it was
  mounted again while the width observer still watched the old, detached element. It now stays in
  one place for both modes.
- **Checks:** the 10 000-item demo library in the real app under Xvfb: a rename to
  `Poster: final?` stored `Poster_ final_.psd` with the thumbnail still shown and the journal
  empty; Undo Rating and `Ctrl+Z` brought back the rating and the old file name; three items went
  to the trash with Delete and a confirmation (counts of All, folders and tags followed), the Trash
  list showed the date of deletion, Restore brought back folders and tags, Empty Trash removed the
  item folders and thumbnails from disk; Back Up Now, a rename, then Restore brought back the
  earlier name, Check Library reported the one missing original and the one stray file, and
  restoring the "before a restore" backup made the check clean again; Copy File put a
  `text/uri-list` on the X clipboard; the right-click menu selected its item; dragging the preview
  started an OS drag with the thumbnail; Rebuild Thumbnails moved all 9 998 thumbnails to version
  2 and went on after a restart. The Windows checks (Explorer, the default app, dropping into other
  apps) are still open.

## D-053 Release 0.1.1 installers (2026-10-29)

- The app version is **0.1.1**, in the workspace package, `Cargo.lock`, `tauri.conf.json` and
  `ui/package.json`. `tauri.conf.json` still bundles only the Windows NSIS target, so `pnpm build`
  on Windows is unchanged.
- A version tag (`v*`) also builds a macOS DMG (`pnpm tauri build --bundles dmg` on
  `macos-latest`, Apple Silicon) and a Linux AppImage (`--bundles appimage` on `ubuntu-22.04`).
  The config names no Linux bundle; the release attaches one AppImage. Ubuntu 22.04 is the oldest
  hosted image that still has WebKitGTK 4.1, which keeps the AppImage's glibc baseline where
  Tauri's AppImage guidance puts it. The image is deprecated and remains runnable until
  2027-04-17.
- The three installers are unsigned. Signing, auto-update and the Windows done check stay in
  phase 12. Eagle import, the live connection and export are not in 0.1.1.

## D-054 Scrollbars follow the app theme (2026-10-29)

- The grid's scrollbar stayed the light system bar (a white track and thumb) in the dark theme,
  because the page never set `color-scheme`. It is `light` on `:root` and `dark` on `.dark`.
  The thumb is the `ring` token and the track is transparent, so the bar shows the surface under
  it (the grid's `--grid`, the sidebar's own background) and the light theme stays a readable
  grey, not a hardcoded colour.

## D-055 Search field in the toolbar (2026-10-29)

- The toolbar search was a fixed 9rem box packed against Filter, with empty space beside it.
  It is now 24rem (`w-96`), centered in the free space between the place title and the Filter
  button, and it shrinks to that space (`max-w-full`) so Filter, sort and Add files stay where
  they are. It does not grow across the whole row.

## D-056 YubiKey folder unlock and WebAuthn app login (2026-10-29)

Two locks, and they do not share keys (spec §13).

- **A locked folder** keeps its password wrap and its recovery wrap. A YubiKey 5C is an
  optional third wrap of that same age X25519 identity. The ciphertext is an age file whose
  recipient stanza is `piv-p256`, the stanza `age-plugin-yubikey` 0.5 writes for an ECDSA
  P-256 key in a retired PIV slot (0x82–0x95). The card holds the P-256 key. The age identity
  stays in `folder_piv_wrap`. Unlocking one folder loads only that identity. Without the card,
  the password and the recovery secret still open the folder, and a missing card does not
  block the others.
- **Touch and PIN are alternatives.** A wrap is stored as `touch` or `pin`. A prompt of the
  other kind is refused before any ECDH. The YubiKey PIN is 6 to 8 digits. It is not the app
  login PIN.
- **Format version 4** rebuilds the empty `folder_piv_wrap` table with `slot` and `policy`,
  after a backup, the same way version 3 rebuilt the search index. Changing the password or
  the recovery secret does not drop the YubiKey rows. Removing the lock does.
- **Tests have no card.** A software P-256 key performs the same ECDH the card would, so the
  wrap, the slot range, and the touch-or-PIN rule are checked in `cargo test`.
- **Desktop PIV bridge** (`src-tauri/src/yubikey.rs`) talks to a real YubiKey 5C over PC/SC
  through the `yubikey` 0.8.0 crate (`untested` for ECDH). `present` opens the card;
  `enroll` picks the first empty retired slot 0x82–0x95, generates an ECC P-256 key with
  touch-or-PIN as alternatives (`PinPolicy::Never`+`TouchPolicy::Always`, or the reverse),
  and stores the `age1yubikey` recipient. `agree` verifies the PIN when the wrap is PIN, then
  does P-256 ECDH with `decrypt_data`. Without a card, the bridge still reports that no
  YubiKey is connected and leaves the password path in place. Factory management key
  (or PIN-protected mgm after the factory PIN) is required to enroll.
- **App login** is exactly one of password, PIN, or WebAuthn, stored in `login.json` in the
  app data folder, not in `library.db`. Password and PIN are Argon2id hashes, not age keys.
  WebAuthn stores the credential id and the P-256 public key. Login checks the assertion
  (authenticator data, the challenge, a user-present flag, the signature) and returns no key
  material. A YubiKey 5C touch is the authenticator. It does not use the PIV slot and does not
  unwrap folder identities. With no method set, the app opens on Welcome. Switching methods
  replaces the previous login and does not re-encrypt libraries.
- **No physical key in tests.** A software authenticator signs the WebAuthn assertion. The
  system bridge, the stand-in for `tauri-plugin-webauthn`, refuses when no authenticator is
  connected and does not ask for a PIN.
- **New direct dependencies**, pinned to versions already in the tree (D-002, D-003):
  `age-core` 0.12.0, `base64` 0.22.1, `bech32` 0.11.1, `p256` 0.13.2, `rand` 0.8.8,
  `sha2` 0.10.9. `argon2` 0.6.0 and `secrecy` 0.10.3 were already direct dependencies of
  the locked-folder crates; login uses them too.

## D-057 Dropping items on a tag (2026-10-30)

- **A tag row is a drop target** in the same mouse-event drag as folders (D-042). Each tag in
  Tag groups and in Recent tags carries `data-drop="tag"` and the exact tag name. The payload
  is the items payload already used for a folder (`ItemsTarget`). A folder dragged onto a tag
  is refused. A read-only library refuses every drop, as it refuses a drop on a folder.
- **The edit is `addTags` with that one name.** The full tag list is not sent, so the other
  tags stay. The name is kept exactly: `Poster` and `poster` are different targets.
- **Nothing to add is a no-op.** The UI skips the edit when the open item, or the selection's
  tag counts, already show that tag on every dropped item. Rust does the same when it writes:
  an item that already has every requested tag is left alone (no new modification time, no
  change log row), and a batch that changes no item is not an undo step. Items that lack the
  tag still gain it. A tag becomes recent only when the edit reports that items changed.
- **The highlight** is the folder inside-zone highlight, on the row under the pointer. A tag
  that also sits in Recent tags does not light both rows.

## D-059 Locked folders only in the Locked folders section (2026-10-30)

- **Folders tree omits every locked folder.** A folder with `is_locked` (locked or unlocked for
  the session) is filtered out of the Folders section, so it is not listed twice next to normal
  folders. Lock… stays on an unlocked folder's menu in Folders; Unlock…, Lock Now, Change
  Password…, New Recovery Key…, Add/Remove YubiKey… and Remove Lock… live on the Locked
  folders section's context menu.
- **Nested children of a locked ancestor stay out of Folders too.** Hiding the locked ancestor
  also hides its descendants in that tree (even unlocked children of an unlocked locked
  folder). Opening the locked folder from Locked folders still shows its items; Include
  subfolders covers nested items. Locked folders continues to list only the locked ancestors
  themselves, in tree order. Revisit if nested folder navigation under an unlocked lock needs
  its own tree.

## D-060 Opt out of the trash confirmation (2026-10-31)

- **"Don't ask again" on the trash confirmation.** `Delete` asks before moving items to the
  trash (spec §7.3). The confirmation dialog has a "Don't ask again" checkbox with a hint that
  the choice lives in Settings; ticking it and confirming saves `confirmBeforeTrash: false`
  (`Preferences.confirm_before_trash` in `app-store`, default `true`), and from then on
  `Delete` moves the selection to the trash directly. Settings → General has a
  "Confirm before moving items to the trash" switch to turn the confirmation back on.
- **Only the trash.** Delete Permanently and Empty Trash always ask: they remove files with no
  undo, so they never get an opt-out. The shared `ConfirmDialog` carries the checkbox as an
  optional part of the confirmation; other callers are unchanged.

## D-061 Reviewed Import Folder… (2026-10-31)

- **One folder, not a drive.** File → Import Folder… picks a single folder (EGL-31: no
  full-disk scan in v1). Dropped folders stay unreviewed and take every non-junk file; only
  this command shows the review dialog.
- **Preview before copy.** `preview_import` walks the folder and returns counts, bytes and a
  tree (cap 10 000 listed entries; `unlisted` covers the rest). Nothing is copied. Images of
  the v1 preview formats are preselected; other files are listed off. Unchecking a folder
  excludes its children (`exclude`); checking a non-image adds it (`include`). Confirm calls
  `add_files` with `reviewed: true` and those sets; cancel leaves the library unchanged.
- **Same pipeline.** After confirm, the usual add-file path runs (tmp → hash → rename → row,
  arch §15.4). Source files stay on disk. Duplicates, a locked target folder, cancel during
  copy and a read-only library behave as for Add Files…. Relative paths in `exclude` /
  `include` are from the picked folder's children (`/` separated), matching the walk.

Note: D-058 (preview rotate, session-only) lives on `feat/viewer-rotate` / was merged in code
without its decision entry on `main`; this number skips ahead so a later cherry-pick of that
entry does not collide.

## D-062 Desktop auto-update through GitHub Releases (2026-10-31)

- **Plugin.** `tauri-plugin-updater` / `@tauri-apps/plugin-updater` **2.14.1** and
  `tauri-plugin-process` / `@tauri-apps/plugin-process` **2.4.0** (relaunch after install),
  pinned exactly (D-002, D-003). Not the 3.x alphas. Registered in `src-tauri`; capabilities
  add `updater:default` and `process:allow-restart`.
- **Endpoint.** Static JSON on the latest GitHub Release:
  `https://github.com/fighxy/egl-viewer/releases/latest/download/latest.json`.
  `bundle.createUpdaterArtifacts: true`. Windows `installMode: "passive"` matches the
  per-user NSIS install (D-053).
- **Signing.** Minisign keypair from `pnpm tauri signer generate`. The **public** key is the
  string in `plugins.updater.pubkey` in `tauri.conf.json`. The **private** key never enters
  git: local file (for example `%USERPROFILE%\.tauri\egl-viewer.key`) and GitHub Actions
  secrets `TAURI_SIGNING_PRIVATE_KEY` / optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Release
  jobs fail if the secret is missing. Losing the private key bricks in-app updates until a
  new pubkey ships in a manually installed build.
- **UX.** Help → Check for Updates…; a silent check after the login gate opens in production
  builds (errors stay quiet). Install and Restart only after the user confirms. English
  strings follow the active locale (D-069).
- **Platforms.** Windows NSIS `.exe` + `.sig`; macOS `.app.tar.gz` + `.sig` (DMG still for
  first install); Linux AppImage + `.sig`. `scripts/write-latest-json.mjs` builds the
  manifest to attach to the release.
- **Migration.** Builds before this change (including 0.1.2) cannot self-update: one last
  manual install of the first updater-enabled release, then in-app updates work. Rollback is
  a manual reinstall of an older GitHub Release; `allowDowngrades` stays off.
- **Still phase 12.** Authenticode / Apple notarization are separate; SmartScreen may still
  warn on unsigned first installs.

## D-063 HEIC/HEIF previews through the system libheif (2026-10-31)

- **Decoder.** `libheif-rs` **2.6** (MIT), default features off, `v1_17`: works with any system
  libheif >= 1.17 and never requires the newest one. The pure Rust `heic` crate was rejected:
  every version is AGPL-3.0-only or commercial, which would relicense the distributed app.
  libheif itself is LGPL: it is linked dynamically and must ship next to the app with a notice
  (Windows packaging: D-070; libwebp stays built from source per D-036).
- **Pipeline.** `heic`/`heif`/`hif` map to `Kind::Heic` in `media`. libheif applies the file's
  geometric transforms (irot/imir), so no EXIF-orientation pass is needed. EXIF comes from the
  HEIF `Exif` metadata item: a four-byte big-endian offset to the TIFF header (ISO/IEC
  23008-12 §6.2), parsed by the existing kamadak-exif code. Files above 1 GiB keep the
  placeholder, like PSD. Decoding is from bytes (`read_from_bytes`), so locked folders never
  write plaintext to disk.
- **Preview.** The WebView cannot show HEIC, so the full-size preview falls back to the 512 px
  thumbnail like TIFF and PSD (`Preview.tsx`); a converted full-size PNG endpoint is a later
  option.
- **CI.** Windows: `vcpkg install libheif[core,libde265]:x64-windows` (on `PATH` for tests;
  decode-only — see D-070). macOS: `brew install libheif`. Linux AppImage (Ubuntu 22.04):
  libheif from the `strukturag/libheif` PPA because Jammy ships 1.12 < 1.17.
- **Fixture exception to D-034.** `crates/media/fixtures/sample.heic.b64` (96x64, with EXIF)
  is a committed base64 file, decoded by `fixtures::heic()`: pure Rust has no HEIC encoder to
  make one in code. It does not vary with the fixture seed, so it stays out of
  `every_format()` and the duplicate-detection tests; the media tests use it directly.

## D-064 Export and Backup of originals (2026-10-31)

- **What it is.** Library → Backup… and the Export… item of the folder and tag context menus
  copy the originals of a scope (whole library, chosen folders with or without subfolders,
  chosen tags, any mix) to a user-picked folder, as a plain folder tree or one ZIP archive.
  The existing database backup (Back Up Now, restore) is unchanged and unrelated.
- **Selection SQL.** One `WITH RECURSIVE` over all chosen folders plus an `EXISTS` per tag
  name, `deleted_at IS NULL` and the `VISIBLE` lock rule; an item is counted and exported once
  (`DISTINCT`). The preview (item count, byte sum) is the same query with `COUNT + SUM(size)`,
  so the dialog's numbers cannot disagree with what is written. `hidden` counts what the
  selection covers without `VISIBLE` — items of locked folders that stay home.
- **Layout.** Selected folders mirror their subtree (`<export root>/<folder>/<sub>/file`);
  the whole library mirrors every top-level folder and puts unfiled items at the root; a tag
  export (or anything a folder tree does not cover) is flat. File names reuse
  `file_name_for` (Windows-safe, D-050); a path collision becomes `name (2).ext`,
  case-insensitively. The export root is named after the single selected folder or tag, else
  the library; a second export into the same destination gets ` (2)`.
- **Manifest.** `egl-export.json` at the export root carries the library name, the time and,
  per item, id, name, tags, folders, rating, note, link and the written relative path — a bare
  file copy keeps its metadata. There is no restore from an export in v1.
- **Protection.** The archive option encrypts the ZIP with an **age passphrase**
  (`Encryptor::with_user_passphrase`, scrypt) into `<name>.zip.age`; the password goes over
  IPC like a lock password and is never stored. Decrypting is age CLI territory
  (`age -d -p file.zip.age`). An export refuses a destination inside the library folder.
- **Reads only.** A read-only library exports fine. Items of a locked folder are hidden like
  everywhere else (`VISIBLE`); items of a folder unlocked in this session export decrypted,
  streaming through the vault — plaintext never sits on disk inside the library.

## D-065 The `zip` crate for export archives (2026-10-31)

- **Choice.** `zip` **9.2** (MIT), default features off, `deflate-flate2` only: flate2 with the
  pure Rust zlib-rs backend, no C, no zopfli/bzip2/lzma/zstd. `zip` is the same library Eagle
  uses for `.eaglepack` (architecture §17), so the archives read everywhere.
- **Compression levels.** None (Stored), Fast (deflate 1), Balanced (6, the default), Smallest
  (9). Media is usually pre-compressed, so None is a first-class choice; v1 has no zstd.
- **Writing.** `ZipWriter` over a `.zip.part` file in the destination (auto-large-file for
  entries above 4 GiB), renamed on success — a crash or a cancel never leaves a half-written
  archive under its final name. `set_auto_large_file` keeps zip64 entry sizes honest.
- **Rejected.** `tar` + external gzip: no entry index without reading to the end, worse for
  partial restores later. Per-entry AES of the zip crate: nonstandard readers; age over the
  whole archive (D-064) is one well-audited construction instead.

## D-066 Export progress and tasks (2026-10-31)

- **Background task, event progress.** `start_export` returns at once; the work runs on a
  named thread holding `Arc<Library>` and reports `export-progress` events through the session
  emitter, the same shape as imports (D-022). The dialog subscribes by task id; `cancel_export`
  sets a flag the engine checks between files and removes the partial output. Closing the
  library or the window stops exports like imports.
- **Why not a `Channel`.** A channel dies with the dialog; the event registry lets the dialog
  reopen onto a running export (`exports` lists it) and keeps one mechanism for long file
  work. Channels stay for genuinely modal waits (open view, locking).
- **Final states.** `done` carries the summary (items, files, bytes, missing names, path);
  `failed` carries the message; `cancelled` means the partial output was removed. The engine
  removes partial output on any non-success, so a failed export never masquerades as a backup.

## D-067 Justified library grid (2026-10-31)

- **Layout.** The grid mode is justified rows (Flickr / Google Photos): one height per row, each
  cell width from `ItemCell.width` / `height` (aspect), rows fill the container width, `8px`
  gutters, rounded thumbs. The thumbnail size slider is the **target row height** (still
  80–320 px). Show Names keeps the existing name-only caption under the thumb (no WxH line).
  List mode is unchanged.
- **Packing data.** Stable row breaks need sizes for every position, not only the visible
  windows. `view_dimensions(viewId) → Vec<[u32; 2]>` returns stored pixels (zeros = unknown →
  square). That is numbers only — still not a whole `ItemCell` list over IPC. Invalidated with
  the view on `views-invalidated`.
- **Virtualization.** TanStack Virtual still virtualizes **rows**; each row now has a variable
  height and cell count. Rubber-band hit-testing and arrow keys use the packed geometry
  (down/up = nearest cell by X in the next/previous row). Selection, DnD onto folders/tags, and
  manual reorder drop targets stay index/id based.
- **Tradeoff vs fixed squares.** Variable items-per-row and row height; scroll anchoring remaps
  by item index when the size slider or window width changes. Extreme panoramas are clamped to
  0.5×–1.5× the target height so one row cannot dominate. Last incomplete rows stay near the
  target height unless they already fill most of the width.

## D-068 No native WebView context menu anywhere (2026-10-31)

- D-029 already said the WebView2 context menu is replaced by the app's own. Phase 1 still
  left it on for `input` / `textarea` / contenteditable so cut, copy and paste stayed in the
  browser chrome. That menu also offers emoji and password-import entries (localized by the
  OS), which made the app feel like a browser page.
- **Always** `preventDefault` on `contextmenu` in `installDesktopBehaviour`, including over
  text fields. Clipboard shortcuts (`Ctrl+X` / `C` / `V`) still work in fields; there is no
  app context menu for fields yet.
- Keep the listener on the **bubbling** phase of the window (D-042): Radix `ContextMenu`
  handlers on folder / item / tag triggers run first and open the app menu; a capture-phase
  prevent would leave those menus dead.

## D-069 UI languages: English default, Russian and Chinese selectable (2026-10-31)

Supersedes D-007. The UI ships with **English as the default and the source catalog**, plus
**Russian (ru)** and **Chinese Simplified (zh-CN)**. The user picks the language in
Settings → General; the choice is stored in `preferences.locale` through `app-store`
(D-022), same as theme.

- **Stack.** `i18next` **26.5.11** and `react-i18next` **17.1.9** in the UI package (newest
  older than pnpm's minimum release age, D-002 / D-003). Catalogs live under `ui/src/i18n/`
  (`en.ts`, `ru.ts`, `zh-CN.ts`); English is the typed source. `setAppLocale` applies
  the preference at startup and when Settings saves.
- **Coverage.** App UI strings are translated (menus, Settings, Welcome, library chrome,
  filters, imports, locks, details, smart folders, maintenance, updates, errors, status,
  sort menus, and Backup / Export). Native OS file dialogs stay in the OS language. New UI
  strings go into the English catalog first.
- **Sort menu.** The toolbar `SortButton`, View → Sort By submenu, and the folder-properties
  sort select all use `sort.*` (field names, ascending/descending, aria/title, composed
  folder options). `translateSortLabel` / `translateSubmenuItem` in `ui/src/i18n/labels.ts`
  keep the English `SORT_KEYS` registry as the key list only.
- **Backup / Export.** `ExportDialog` (Library → Backup… and folder/tag Export) uses
  `exportUi.*`, plus shared keys such as `common.cancel`, `common.browse`, and
  `foldersUi.includeSubfolders`.
- **IPC.** `Locale` is a `ts-rs` enum (`"en" | "ru" | "zh-CN"`) on `Preferences`; missing
  field keeps English.
- **Tests.** Catalog key coverage for `ru` / `zh-CN` vs English; language switch; Settings
  saves `locale`; sort and Backup / Export labels resolve under `ru` / `zh-CN`.

## D-070 Bundle libheif DLLs with the Windows NSIS installer (2026-11-01)

Fixes the Windows startup failure where `egl-viewer.exe` could not find `heif.dll` (D-063
linked libheif dynamically, but the installer did not ship it).

- **What ships.** The NSIS package installs `heif.dll`, `libde265.dll` (and any further
  DLLs those two import from the same vcpkg `bin` directory) **next to** `egl-viewer.exe`,
  plus `NOTICE-libheif.txt` and `NOTICE-libde265.txt` (LGPL). Windows `LoadLibrary` only
  searches the exe directory; a `resources/` subfolder would not load.
- **How.** CI (`check` and `nsis`) installs the same vcpkg package, then runs
  `scripts/bundle-heif-dlls.ps1` **before** any cargo/tauri step that compiles
  `src-tauri` — `tauri-build` reads `tauri.windows.conf.json` during clippy and fails if
  the listed paths are missing. The script stages files into `src-tauri/heif-runtime/`
  (gitignored artefacts; README kept). `tauri.windows.conf.json` maps each file to the
  install root (`heif.dll`, `libde265.dll`, notices) — not under `resources/`.
  `node scripts/check-heif-windows-bundle.mjs` guards those mappings in `pnpm check` / CI.
- **Decode-only features.** vcpkg's default `hevc` feature pulls **GPL** x265. CI removes any
  prior `libheif` / `x265` install, then installs `[core,libde265]` so iPhone HEVC HEIC
  decodes without shipping x265, and asserts `libx265.lib` / `libx265.dll` are absent.
  Rust cache uses `prefix-key: heif-core-libde265-v1` so a previous default-feature install
  cannot leave `cargo:rustc-link-lib=x265` in a restored `libheif-sys` build script output
  (that caused `LNK1181` on `libx265.lib`). The staging script refuses `libx265.dll` if it
  appears, and dumpbin fails the job if heif/libde265 import any further DLL that lives in
  the vcpkg bin (so the fixed resource map cannot silently omit a sibling).
- **Other platforms.** macOS and Linux keep system / AppImage libheif from D-063; this
  decision is Windows packaging only. libwebp remains built from source (D-036), so there
  is still no separate libwebp DLL to bundle.
- **Verify.** After a Windows NSIS install (or unzip of the install dir), `heif.dll` and
  `libde265.dll` sit beside `egl-viewer.exe`; the app starts without a missing-DLL dialog;
  importing a `.heic` produces a thumbnail. On CI, the nsis job prints `bundled-dlls.txt`
  before `pnpm build`.

## D-071 Hide Add files in the trash toolbar (2026-11-01)

When the open place is the trash (`place.kind === "trash"`), the toolbar does not show
**Add files**. Empty Trash stays (via `placeActions`). Importing into the trash would be
misleading: new files never land there. File → Add Files… / Import Folder… remain available
from the menu and still add into the library as usual. Covered by `LibraryScreen` and Phase 7
UI tests.

## D-072 Confirm before quitting (2026-11-01)

Closing the only window (title-bar X) or choosing File → Exit always asks before the app
exits. Cancel dismisses the dialog and keeps the process running.

- **Rust.** Every `CloseRequested` calls `prevent_close` and emits `close-requested` with
  `{ busy }` (`true` when imports or exports are still running). The window geometry is no
  longer saved on a cancelled close; `quit` still remembers it, then stops imports and exits
  (arch §8, §9).
- **UI.** The existing `ConfirmDialog` shows either the stop-and-quit wording (`busy`) or a
  simple quit prompt. Confirm calls `quit`; Cancel clears the dialog. File → Exit goes through
  `window.close()`, so it shares the same path as the title-bar X.
- **i18n.** `confirm.quitTitle` / `quitBody` / `quitConfirm` in `en`, `ru`, and `zh-CN`
  (D-069); Cancel reuses `common.cancel`.

## D-073 Dropping items on Trash (2026-11-01)

- **The Trash row is a drop target** in the same mouse-event drag as folders and tags
  (D-042, D-057). It carries `data-drop="trash"`. The payload is the items payload already
  used for a folder or a tag (`ItemsTarget`). A folder dragged onto Trash is refused. A
  read-only library refuses every drop. Items cannot be dragged out of the Trash view
  already, so there is no drop-onto-self case.
- **The edit is the same `trash` intent as Delete / Move to Trash.** Multi-select keeps the
  selection when the drag starts from a selected cell (same as a folder drop). The
  confirmation from D-060 applies: when `confirmBeforeTrash` is on, the drop opens the same
  dialog (with "Don't ask again"); when it is off, the items go straight to the trash.
- **The highlight** is the folder inside-zone / tag-row highlight on the Trash button under
  the pointer. A refused target is not highlighted.

## D-074 Progress while emptying the trash (2026-11-01)

- **Modal Channel, like locking.** Empty Trash stays a modal wait (`empty_trash` with a
  `Channel` of `{ done, total }`), not a background export-style task (D-066). A large trash
  (~2000 items) was otherwise a silent hang after the confirmation. The dialog shows a bar and
  a counted status; Cancel calls `cancel_empty_trash`, which stops between items.
- **Two halves.** `done` / `total` cover removing rows, then deleting files (`total` is twice
  the item count). After the delete transaction commits, views refresh immediately so the grid
  and counts can update while files are still going; leftover journal rows still clean up on
  the next open (library-format §6).
- **Counts and trash info refresh without the 300 ms debounce.** On progress (throttled) and
  when the command finishes, the UI re-fetches `library_counts` and `trash_info` and
  invalidates the folder tree so the sidebar badge and the Trash context menu stop showing
  stale numbers. Relying only on `views-invalidated` left the badge wrong or late.

## D-075 Phase 9 libraries: reqwest, url, keyring, wiremock (2026-11-01)

Phase 9 (connect to a running Eagle, docs/spec.md §8.3) adds four pinned
dependencies, the newest stable releases of 2026-10-28 where available:

- `reqwest 0.12.28` with `default-features = false, features = ["json",
  "rustls-tls"]`: the Eagle transport (docs/architecture.md §4). rustls, not
  native-tls: no Schannel dependency to stage in the installer, and Eagle's
  local API is plain HTTP anyway.
- `url 2.5.4`: the endpoint base is parsed and joined, never concatenated by
  hand.
- `keyring 4.2.0`: the Eagle API token lives in Windows Credential Manager
  (docs/spec.md §8.3 "Auth"); `app-store` gains `SecretStore` with a memory
  implementation for tests.
- `wiremock 0.6.3` (dev, `eagle-api` only): the integration tests replay the
  captures from `fixtures/eagle-api/`, per docs/architecture.md §13.

## D-076 Phase 10: eagle-api writes, adds, same-PC thumbnails (2026-11-01)

Phase 10 (docs/spec.md §9) turns a live Eagle session writable and serves
same-PC thumbnails. `eagle-api` takes two already-pinned workspace crates:

- `base64 0.22.1`: data-URL bodies for `item/add` from another computer
  (correction 1).
- `media`: when Eagle reports `noThumbnail: true`, the original under
  `images/{id}.info/` is downscaled for the grid (docs/spec.md §8.3
  "Thumbnails"); the library path sandbox still refuses any thumbnail path
  outside the open `.library` folder.

Remote thumbnails stay deferred to a helper plugin on the Eagle PC. Undo of
item edits stays native-only: Eagle's API does not return previous states.
Unsupported native-only session commands (`delete_folder`, empty trash, tag
groups, smart-folder writes, …) return `NotSupported` while an Eagle session
is open, not "Open a library first".

## D-077 Phase 11: export into a running Eagle (2026-11-01)

Phase 11 (docs/spec.md §8.4 / §9, docs/architecture.md §15.7) exports a native
library into a running Eagle through its API only, so Eagle stays the writer of
its library.

- **Destination.** `ExportRequest.eagle: Option<EagleExportTarget>` (host, port,
  token). When set, `format`, `destination` and `encrypt` are ignored. The
  Export dialog adds “Into a running Eagle” beside folder / ZIP; the token never
  reaches the WebView beyond the form field for that run (same Connect rules).
- **Plan.** `library-native::load_eagle_export` lists folders top-down, items
  (locked-folder items stay hidden like every export), comments, tag groups and
  smart folders. Locked folders themselves are listed and exported unlocked
  after a UI warning.
- **API.** `eagle-api` gains `tag_group_create`, `smart_folder_create` and
  `item_add_comment` (folder create/update and item add already existed). Same-PC
  uses batch `item/add` with `path`; another computer uses data URLs then
  `item/update {star}`.
- **Repeat exports.** New Eagle ids are written back with `store_eagle_ids` into
  the existing `eagle_id` columns (format version 1), so a second export updates
  instead of duplicating. No format bump.
- **Engine.** `src-tauri/export_eagle.rs` runs on the export thread via
  `start_export`; progress and cancel reuse D-066. Live done check against a
  real Eagle stays manual.

## D-078 Persist place, filters and selection per library (2026-11-01)

Phase 12 (docs/spec.md §9): after reopen (D-022), restore where the user was
inside that library.

- **File.** `queries.json` next to `preferences.json` / `recent.json` (same
  atomic JSON write as D-022). One entry per library path (newest first, cap 40).
- **Payload.** Place (All / Uncategorized / Untagged / Trash / folder / smart),
  filters (tags, folders, type, rating, shape and their modes), search text,
  selection (`all` flag plus up to 500 ids — Select All stays a flag), and an
  optional per-screen sort when a folder overrides the global browse sort.
- **Not in this file.** Global browse preferences (grid/list, thumbnail size,
  global sort, include-subfolders) stay in `preferences.json` (D-041). Window
  geometry stays in `window.json`.
- **Lifecycle.** The UI debounces writes while browsing; forgetting a recent
  library also drops its entry. Missing or unknown folder/smart ids fall back
  to All once the trees load (existing place guards).

## D-079 File logs and Help → Open Log Folder (2026-11-01)

- **Folder.** `{app_local_data_dir}/logs/egl-viewer.log`, created on start.
- **Subscriber.** `tracing` writes to stderr (dev / CI) and appends to that file.
  Same rules as before: never log the Eagle token or raw bodies of
  `library/info` / `folder/get` (§10).
- **Menu.** Help → Open Log Folder runs `open_log_folder`, which creates the
  folder if needed and opens it with the system file manager.
- **No new crate.** The file writer is a small `Mutex<File>` MakeWriter; no
  `tracing-appender` dependency.

## D-080 Clear the Eagle token from the WebView after send (2026-11-01)

Tightens phase 12’s “token never reaches the WebView longer than needed”.

- **Connect.** On submit, the Connect form copies the token into the IPC payload
  and clears the password field immediately. Rust wraps it in `SecretString`
  before storing in the keyring (D-010 / D-075).
- **Export into Eagle.** The Export dialog clears its token field once
  `start_export` accepts the request.
- **Still true.** `EagleSessionInfo` has no token field; POST bodies keep the
  token in JSON only; there is still no crash-report pipeline to scrub. Logs
  follow D-079.

## D-081 Phase 12 accessibility polish; custom title bar still postponed (2026-11-01)

- **Landed.** Folder tree Home / End; filter-bar chips expose `aria-label` /
  `aria-pressed`; existing tree / tag / grid roles and keyboard remain.
- **Still postponed (D-025).** A custom title bar that merges the native caption
  with the menu (and Alt to reach the menu) needs `decorations: false`, drag
  regions, and Snap Layouts restoration (e.g. `tauri-plugin-decorum`). That is a
  large redesign and stays out of this hardening pass; the native frame plus the
  app Menubar keep working.

## D-082 Experimental iOS SwiftUI shell over native `.eglib` (2026-11-02)

Draft iPhone client for **native libraries only** (no Eagle API, import or export;
no Canvas). Desktop Tauri stays the default product and the Windows CI path is
unchanged.

- **FFI.** New crate `library-ffi` (`libegl_library`, `rlib` + `staticlib`) exposes
  a small **C ABI** over `library-native`: create/open/close, `LibraryInfo`, a
  windowed scope (`ItemCell` JSON, capped at `MAX_WINDOW`), item detail, folder
  tree, tags, thumbnail path/bytes, and `ItemEdit` for tags/folders. Plain C was
  chosen over UniFFI / swift-bridge: no new dependencies (D-002 / D-003), host
  `cargo test -p library-ffi` works on Linux with libheif available, and the Mac
  / Xcode link step is documented in `ios/README.md`. `unsafe` stays at the FFI
  boundary (`#![allow(unsafe_code)]`, same idea as the WebAuthn bridge).
- **iOS package.** `ios/EglViewer.xcodeproj` + SwiftUI screens (Welcome, Browse,
  Preview, folders/tags edit) matching the desktop-aligned concept. Without the
  static library linked, the UI runs on **mock data**; define `EGL_HAS_FFI` and
  link `libegl_library.a` to call Rust (see `ios/README.md`).
- **Out of scope for this draft.** App Store, signing, document-picker / security-
  scoped bookmarks end-to-end, on-device media pipeline, Eagle, production polish.
- **D-011.** Mobile `staticlib` lives in `library-ffi`, not in `src-tauri` (desktop
  remains `rlib`-only).

## D-083 Experimental Android Compose shell over the same C ABI (2026-11-02)

Companion draft to D-082: a **Jetpack Compose** phone client for native `.eglib`
only (no Eagle, no Canvas). Desktop Tauri stays the default.

- **Size check.** The iOS SwiftUI shell is ~1.1k LOC of product Swift. The Android
  mirror is ~1.2k Kotlin (screens + bridge) plus ~200 C++ JNI forwarders — well
  under a multi‑kLOC balloon, so the scaffold was worth landing. Gradle /
  manifests are scaffolding, not product logic.
- **Same FFI.** Reuses `library-ffi` / `egl_*`. `crate-type` now also includes
  `cdylib` so `cargo ndk` can emit `libegl_library.so` for `arm64-v8a`. A tiny
  JNI shim (`android/app/src/main/cpp/egl_jni.cpp`) forwards to the C ABI; without
  the `.so`, Compose runs on **mock data** (same idea as iOS `EGL_HAS_FFI`).
- **UI.** Welcome → Browse (places strip, filters, dense grid, tab bar) → Preview
  → Details / Tags / Folders sheet, aligned with `ios/` and the iOS UI concept
  (desktop DNA, D-028 tokens). Kotlin packages live under `app.eglviewer.screens`
  / `app.eglviewer.theme` — **not** `app.eglviewer.ui`, which shadows
  `androidx.compose.ui` and makes `import androidx.compose.ui.Modifier` fail
  (and `…Modifier.Modifier` resolve the nested `modifier` package instead of
  the class).
- **Not Tauri Android.** This is a native Compose shell, not the WebView UI. Play
  Store, signing, SAF end-to-end, and on-device media pipeline stay out of scope.
- **Docs.** `android/README.md` covers NDK / `cargo-ndk` build and CMake wiring.

## D-085 Mobile UI parity (Compose ↔ SwiftUI) (2026-11-02)

Bring the experimental Android Compose shell and iOS SwiftUI shell to the same
visual/UX language as the iPhone MVP concept (desktop DNA, D-028 tokens):

- **Shared chrome.** Welcome (brand hero, Create / Open, recent rows with tile +
  location hint), Browse (places strip with counts, search + filter/sort
  toolbar, applied filter pills, dense grid on `--grid`, Grid/Places/Tags/More
  tab bar), Preview (near-black stage, ghost close/info + tool buttons, swipe-up
  / info → edit sheet), Edit sheet (filename + meta, Details / Tags sheet /
  Folders segmented control at the bottom, tag chips, folder check rows).
- **UI-only.** No FFI / JNI / bridge contract changes — stays compatible with
  mock data and a future live `.eglib` wire-up.
- **Not production polish.** SAF / document picker, real thumbs, and justified
  row packing remain stubs; density and tokens match the concept board.

## D-084 CI for experimental mobile shells (2026-11-02)

Separate GitHub Actions workflow [`.github/workflows/mobile.yml`](../.github/workflows/mobile.yml)
runs next to the Windows desktop `CI` workflow and does not change it.

- **Triggers.** Path-filtered on `android/`, `ios/`, `crates/library-ffi` (+ core
  native/media deps), and the workflow itself — so desktop-only PRs skip mobile
  jobs. `workflow_dispatch` stays available.
- **`library-ffi` (ubuntu).** `scripts/check-mobile-shell.sh` (layout + header
  sync), `cargo fmt` / clippy / `cargo test -p library-ffi` with `libheif-dev`.
  The Windows workspace job still covers the crate via `cargo test --workspace`.
- **Android (ubuntu).** Temurin 17, Gradle wrapper cache, the runner’s preinstalled
  Android SDK (not `android-actions/setup-android`, whose default `tools` package
  is gone from `sdkmanager`), platform 35, `./gradlew :app:assembleDebug`.
  **No NDK / no `.so`** — CMake `externalNativeBuild` is gated on
  `jniLibs/.../libegl_library.so` (or `-Pegl.ffi=true`) so the mock-FFI scaffold
  builds on CI without secrets (D-085).
- **iOS (`macos-latest`).** Shared scheme `EglViewer`, unsigned
  `xcodebuild … -sdk iphonesimulator` with `CODE_SIGNING_ALLOWED=NO` (mock FFI,
  no `libegl_library.a`). No Apple Developer certs or secrets.
- **Not required.** Linking Rust into either app, Play/App Store signing, or
  building `libegl_library` for device ABIs in CI.

## D-085 Mobile live `.eglib` open and thumbnails (2026-11-02)

Turns the D-082 / D-083 shells from mock-only UI into **live native library**
clients when the Rust artifact is present. Desktop Tauri is unchanged; still no
Eagle, App Store, or Play polish.

- **Open.** iOS: `fileImporter` for the `.eglib` package / folder, with
  **security-scoped bookmarks** persisted in `UserDefaults` (`LibraryAccess`)
  and resumed on recent open. Android: `OpenDocumentTree` SAF picker, persistable
  URI permissions, and a filesystem-path resolver for primary-storage trees so
  `egl_open` can see `library.db`. Create on Android defaults to app-specific
  external files when FFI is linked; iOS create picks a parent folder then calls
  `egl_create_library`.
- **Thumbs.** `egl_thumbnail_bytes` (and path) feed the browse grid and preview
  stage on both shells; WebP decoded by `UIImage` / `BitmapFactory`. Missing
  thumbs keep the extension placeholder. Host tests cover path/bytes after a
  seeded preview (`cargo test -p library-ffi`).
- **Linking.** iOS: `Config/WithFfi.xcconfig` sets `EGL_HAS_FFI` +
  `-legl_library` search paths (CI / default builds stay mock). Android: Gradle
  enables CMake/NDK only when `libegl_library.so` exists under `jniLibs` (or
  `-Pegl.ffi=true`); JNI gains `createLibrary` and `thumbnailBytes`.
- **SAF limit.** Android still needs a resolvable local path for SQLite; cloud
  provider URIs without a path fall back to mock with an explanatory error.
- **Out of scope.** On-device media pipeline, full-size original preview beyond
  thumb bytes, iCloud File Provider packaging polish, Play/App Store signing.

## D-087 Ids from an export into Eagle get their own table, format version 6 (2026-11-03)

`eagle_id` on `item`, `folder`, `tag_group` and `smart_folder` meant two things: where an
imported row came from (phase 8) and what an export into a running Eagle created (phase 10).
`store_eagle_ids` overwrote it, so exporting an imported item into another Eagle library
replaced its origin, and a later re-import of the first library no longer recognised it.

- **Format version 6** adds `eagle_export (kind, native_id, eagle_id)`, primary key
  `(kind, native_id)`, `STRICT, WITHOUT ROWID`. `eagle_id` now only names the origin of an import.
- **Migration.** Item ids that are not in `eagle_import` move into `eagle_export`. Folder, tag
  group and smart folder ids move only when `eagle_import` is empty: an import never records its
  folders there, so in a library that imported, the column may hold either and stays as it was.
- **No foreign key.** Native ids are ULIDs and never reused; a row of a deleted object is
  harmless and an export simply creates that object again.
- **First fixture.** `fixtures/library/v5/Fixture.eglib` is the first released-format fixture
  (docs/library-format.md §8); `every_released_format_migrates_from_its_fixture` opens a copy of
  each one. New fixtures are written by the ignored `make_format_fixture` test.

## D-088 Data-safety fixes from the post-phase-12 review (2026-11-03)

A review of the work merged after phase 7 found these defects in the core crates; each fix has
a test that failed before it.

- **Eagle import thumbnails.** The import set `thumb_version = 1` without writing the file. The
  converted Eagle thumbnail is now journaled in `pending_file` and written atomically to
  `thumbs/<shard>/<id>.webp` before the row is inserted. The preview pass reuses the import's
  batch loop (`preview_pass`) with its stuck-batch guard. A `Storage` error aborts the import
  instead of becoming a report line.
- **Eagle originals.** `eagle_folder::find_original` is the one finder for both the folder
  reader and the live API: it never takes `<stem>_thumbnail.<ext>` when `<stem>.<ext>` exists and
  prefers the file named after the item.
- **Locks.** `lock_folder` always returns the recovery key once the folder row is committed; a
  stopped re-encryption is finished by the next unlock. Decrypting journals the new plaintext
  preview before writing it; removing old copies after the commit is best effort. Encrypting an
  item that wrote nothing keeps its `file_path`.
- **Imported locked folders** (docs/spec.md §8.2). A folder imported locked has no password.
  `Folder.has_password` tells the UI; Lock takes a first password for it, and Remove Lock needs
  none. The Locked folders section offers Set Password… and Remove Lock… for such a folder.
- **Export.** A failed or cancelled export removes the partial tree, `.zip.part` and `.age`. A
  destination inside an Eagle or egl-viewer library is refused. The manifest name is reserved,
  and the `(2)` counter goes before the extension of names without one or with a leading dot.
- **Export into Eagle.** Folders go out in tree and `sort_key` order, from the selected folders
  whose parent is not selected; hidden subfolders of a locked folder stay home.
- **Migrations.** The writer reads `user_version` again under the lock.
- **App login fails closed.** `login.json` that cannot be read or parsed is an error, never "no
  login": `login_status` returns it, the UI keeps the login screen, and unlocking shows why.
- **Eagle API errors.** A network error drops its URL before it becomes text, so the token in a
  query string never reaches a log or the UI.

## D-089 Session fixes for a connected Eagle (2026-11-03)

The review after phase 12 found these defects in the app layer (`src-tauri`); each has a
test, the first Session tests against a fake Eagle (`wiremock` over `fixtures/eagle-api/`,
`session_eagle_tests.rs`).

- **Deadlock.** Every item edit, batch edit and folder unlock hung while Eagle was connected:
  `if let Some(source) = lock(&self.eagle)…` keeps the guard through the then-block in
  edition 2024, and `refresh_views` locked `eagle` again. `Session::eagle()` clones the
  `Arc` out from under the lock; no call site binds the guard itself any more.
- **One source at a time.** Opening or creating a native library ends a live Eagle
  connection, as connecting already closed the library. A library that fails to close cleanly
  when Eagle connects is logged, not an error: it is already out of the session.
- **The token is saved only after it connected**, so a typo never replaces a working token.
- **Disconnect stops a running add to Eagle** and joins its thread first. A failed add ends as
  Done with its failures listed, not as Cancelled.
- **Export into Eagle.**
  - The ids Eagle created are stored on every exit, a failure or a cancel included, so a retry
    updates instead of duplicating.
  - Decrypted originals for the same-PC batch go into a private temp folder that is removed
    when the run ends.
  - The token is the one typed in the dialog or else the one Connect saved for that host and
    port; it is a `SecretString` in Rust and is dropped from the request before the thread starts.
- **Two requests per Eagle per process** (correction 10): the request permits are shared by
  every client of the same `host:port`, so the connection and an export together still send
  at most two.
- **Folder updates** read the folder from a fresh tree and keep its own sort (`orderBy`,
  `sortIncrease`) when a move or rename does not set one: the body is always its full state.
- **Background tasks** that cannot start a thread are an error for the user's import or
  export, and a log line for pending previews.
- **Login commands** run off the main thread: Argon2id and a YubiKey touch no longer freeze
  the window.
- **Show in Explorer** after an export reveals only a path an export of this session wrote.
- **Undo** puts an entry back when taking it back fails; Delete Permanently drops the deleted
  ids from the stack.
- **Views with pending cells.** A window is capped at `MAX_WINDOW` before the "Importing…"
  cells are appended, so they keep their positions; the totals of a refresh include them.
- **Adding files to Eagle** walks dropped folders with the import's own collector
  (`library_native::files_to_add`): hidden and system files and linked folders stay out.
- **File actions on a selection** (Show in Explorer, Copy File, drag out) take an
  `ItemsTarget`: Select All is resolved in Rust instead of the UI fetching every id.

## D-090 UI fixes from the post-phase-12 review (2026-11-03)

Each fix has a UI test; the ones marked † fail on the code before the fix.

- **Export dialog** †. The form is keyed on each opening (`useOpenings`), so saving the
  destination as the export starts no longer resets the dialog and drops its progress. It
  listens for progress from the moment it opens and keeps the latest event of every export, so
  an event that arrives before `start_export` answers is not lost. Reopened while an export
  runs, it follows that export. Cancel and Show in Explorer report their errors.
- **Settings** keys its draft the same way: a preference or login change made while it is open
  does not reset what the user is editing.
- **Preferences** †. Every change starts from the latest preferences, not from a render's
  copy, and writes go to Rust one after another; Rust's answer is shown only when no newer
  change came. A slow first save no longer undoes a newer one.
- **Saved screen (D-078).** The saved session is read before the library is shown, so the
  library and its restored place, filters and selection land in one render; the selection is
  also set directly, and the skip flags hold for that one commit only. A saved screen equal to
  the default one restores its selection and does not swallow a later reset.
- **Quit while exporting** gets its own question ("An export is still running"): the payload
  of `close-requested` carries `exporting`.
- **Strings** †. A test checks every literal key passed to `t()` against the English catalog;
  it found `common.close` in the Eagle import dialog. Full key typing through i18next's
  `CustomTypeOptions` was tried and left out: its types reject key unions and break type
  predicates in a dozen places for no further finding. Russian plurals have all four forms,
  checked by a test (`status.makingThumbnails` lacked `_few` and `_many`).
- **Imports panel** ignores a late `imports` reply for the previous library.
- **Rubber-band selection** removes its window listeners when the grid unmounts mid-drag.

## D-091 `app-service`: the open native library as an app uses it (2026-11-03)

The desktop `Session` (3 400 lines in `src-tauri/src/session.rs`) held everything an app does
with an open native library: its views, the undo stack, item, folder, tag and lock edits, the
import and export threads. `library-ffi` re-implemented part of it for the mobile shells, and an
iOS app with the desktop's features (the next series of PRs) needs all of it.

- **New crate `crates/app-service`**, without Tauri, C, Eagle or the app's settings files.
  `LibraryService` holds the library, the views, the undo stack, the Empty Trash cancel flag
  and the task registries; `ImportTasks` and `ExportTasks` run one thread per task with a
  cancel flag, the latest progress and (imports) the duplicate question. The app hears
  progress and recomputed views through `Emit`, a closure: Tauri emits to the WebView, the C
  ABI will call back into Swift. `ItemsTarget`, `ViewOpened`, `ViewsInvalidated`,
  `ScopeProgress` and `SmartDraft` move with it; their TypeScript bindings are unchanged.
- **Where a library may live is the caller's question.** The service opens a path, read-only
  when told it is synced; the desktop keeps `app_store::check_location` (cloud-synced and network
  folders), a phone will check its own containers. So `app-service` does not depend on
  `app-store` and its OS secret store.
- **The desktop `Session` stays the Tauri-free layer the commands call**, over a
  `LibraryService`: a running Eagle as the other source (it shares the service's views and
  import registry, so ids stay unique), the "Importing…" cells, recent libraries,
  preferences, login, YubiKey and WebAuthn, the clipboard, and the export into Eagle (a task in
  the service's export registry). `session.rs` shrinks to about 1 300 lines plus its tests.
- **Behaviour does not change.** The Session tests run unchanged against the new layer; the
  service has its own tests for opening, views, edits with undo, imports, the trash, folders and
  tags, exports and the read-only open.
- `library-ffi` moves onto the service in the next PR.

## D-092 `library-ffi` over `app-service`; Swift `EglCore` (2026-11-03)

The C ABI re-implemented part of the desktop session (its own import registry, undo stack and
stateless query windows) and had no locked folders, backups, checks or exports. The iOS app
with the desktop's features needs all of them, the same way the desktop does them.

- **Every export goes through `LibraryService`** (D-091): one service per `EglLibrary` handle.
  Edits refresh the open views and push undo exactly as on the desktop; imports and exports
  are the service's tasks. The crate's own registry and undo stack are gone.
- **The existing functions keep their signatures**, so the frozen Android shell and the current
  iOS screens need no change. `egl_view_window` and `egl_query_window` stay stateless (they
  open a view for the call), and `Target` with `query` still names a Select All.
- **New: views by id** (`egl_open_view`, `egl_view_range`, `egl_view_ids`,
  `egl_view_dimensions`, `egl_close_view`, `egl_edit_items`, `egl_delete_items`,
  `egl_view_selection_tags`, `egl_refresh_views`): a query evaluated once and followed under
  its id, as the desktop grid does.
- **New: events through a callback** (`egl_set_event_callback`): import and export progress and
  `viewsInvalidated`, as JSON, on the thread that produced them. Polling (`egl_imports`,
  `egl_exports`) still works.
- **New: locked folders, settings and maintenance** (lock, unlock, relock one or all, recover,
  change password, renew the recovery key, remove the lock, the lock label; rename, stats,
  backups and restore, check and repair, export preview, start, list and cancel, preview of a
  reviewed import; counts, smart rules, decrypted original bytes). YubiKey and app login stay
  out: there is no PC/SC on iOS, and the app login is the app's (Face ID, a later PR).
- **New status codes** `EGL_ERR_WRONG_PASSWORD` (8) and `EGL_ERR_LOCKED` (9); they were
  `EGL_ERR_OTHER` before.
- **Safety at the boundary.** A panic never unwinds into C: every export runs under
  `catch_unwind` and answers `EGL_ERR_OTHER` with the message. The pointer helpers are
  `unsafe fn` with SAFETY notes; `unsafe` stays confined to this crate.
- **The header is checked.** A host test compares the functions declared in
  `include/egl_library.h` with the exported symbols, and the status codes with the enum;
  `scripts/check-mobile-shell.sh` keeps its copies identical (now four, with `ios/EglCore`).
- **Swift package `ios/EglCore`**: a system-library module over the header, a `Library` class
  (typed calls, `EglError` with the code and the core's message, an event handler) and
  `Codable` models of the core's JSON. Its XCTests run on macOS in CI (`ios-core` job of the
  Mobile workflow) against the core built for the host, so Swift and Rust are tested together
  without a simulator. The app adopts the package in the next PR.
- An export that has not reported yet is listed as preparing, so a dialog opened right after
  the start, or a caller waiting for the list to empty, sees it.
- Not changed: Android's JNI still converts strings with `GetStringUTFChars` (modified UTF-8);
  the Android shell is frozen while the iOS app is built.


## D-093 iOS app foundation: XcodeGen, real core always, libraries in On My iPhone (2026-11-03)

The first PR of the iOS app on `EglCore` (D-092) replaces the draft shell of D-082 and D-085.

- **The Xcode project is generated** from `ios/project.yml` by XcodeGen 2.42.0 (a build tool,
  pinned in CI by its release download, not a library dependency). The generated
  `EglViewer.xcodeproj` is not committed. A hand-written `project.pbxproj` with a local Swift
  package and two test targets is fragile to edit without Xcode, and this repository's work
  happens largely without one.
- **No mock mode.** The app always links the real core through `EglCore`; the draft's
  `EGL_HAS_FFI` switch, `WithFfi.xcconfig`, the bridging header and the mock library are gone.
- **The core is linked as a static library** built with `cargo rustc --crate-type staticlib`
  for the destination (`aarch64-apple-ios`, `aarch64-apple-ios-sim`). Next to a `cdylib` the
  linker takes the dylib, which an app does not embed; the CI jobs remove it.
- **Where libraries live.** New libraries go to On My iPhone (`Documents/Libraries`; the Info.plist
  already shares Documents with the Files app). A library picked in Files is kept with a
  security-scoped bookmark, and its scope is held while it is open. One in iCloud Drive (or any
  path a File Provider manages) opens read-only, as a synced folder does on the desktop (D-021).
- **State.** `AppModel` (`@Observable`, main actor) holds Welcome or one `OpenLibrary`, the
  recent libraries (the last 10, in `UserDefaults`) and the last error; every core call runs on
  a detached task. `OpenLibrary` listens to the core's events and refreshes its counts.
- **Strings** are a String Catalog in English, Russian and Simplified Chinese, the desktop's
  three languages (D-069). `scripts/check-ios-strings.py` checks it on Linux.
- **Colours** follow light and dark mode through system colours, with the desktop's neutral
  accent.
- **Tests**: unit tests (locations, recents, `AppModel` creating, opening and closing a library
  with the real core) and UI tests on a simulator (create, open, close, reopen from Recent; the
  Welcome screen in Russian), in the `ios-app` job of the Mobile workflow, which replaces the
  mock simulator build.
- The library screen shows its places with counts for now; the grid arrives in the next PR.

## D-094 iOS browsing: places, a lazy grid by view id, saved view per library (2026-11-03)

The second app PR of the iOS series (D-093) adds browsing.

- **Places.** The library screen lists All, Uncategorized, Untagged and Trash with counts, the
  folder tree (a folder shows its subfolders' items, as on the desktop) and the smart folders. A
  locked folder shows its lock label and does not open; unlocking comes with the locks PR. Each
  place opens `BrowseScreen` in a `NavigationStack`.
- **Windows by view id.** `BrowseModel` opens one core view per query (`egl_open_view`) and asks
  for cells in windows of 100 around what the user sees (`egl_view_range`; the core serves at
  most 200). The item list never crosses the C ABI. `viewsInvalidated` events reach the open
  models through `OpenLibrary`; a model takes the new total and reloads the windows near the
  last cell seen, dropping the others. Answers for an older query or an older refresh are
  dropped by generation counters.
- **Thumbnails** are the core's WebP bytes (`egl_thumbnail_bytes`), decoded by ImageIO off the
  main thread at the cell's pixel size and kept in an `NSCache` by cost (64 MB). The key has
  the thumbnail version, so a rebuilt thumbnail shows at once. Until it arrives a cell shows its
  main colour (`palette0`), or the file type when it has no thumbnail.
- **Controls.** Grid or list, three thumbnail sizes, the sort (the desktop's keys except manual
  order, tag and duration) with ascending, search in the navigation bar (asked 250 ms after the
  last keystroke), and a filter sheet: tags with any / all / none, file type with counts, rating
  (unrated or exactly 1–5 stars), shape. Each change opens a new view.
- **Selection.** A Select button or a long press starts it; Select All is the core's target
  `{viewId, all: true, ids: exclusions}` (as on the desktop, D-089), so it never lists the view's
  ids. The actions on a selection arrive with the details PR.
- **Saved view** (as D-078 on the desktop), per library path in `UserDefaults`: the sort, layout
  and size for the whole library, and the last place with its filters and search. The library
  opens in that place again; going back to the list of places forgets it, and filters of one
  place do not carry over to another.
- **Tests.** Unit tests of `BrowseModel` against the real core (windows, sort, search, filters,
  Select All with a trash and the refresh, the saved view) and a UI test on a generated library
  (`-sampleLibrary N`, a debug-only launch argument: N PNGs of their own colour, the first four
  tagged `Red`). `scripts/check-ios-strings.py` also skips SF Symbol names after `systemName:`.

## D-095 iOS preview, details and actions with undo (2026-11-04)

The third app PR of the iOS series (D-093, D-094).

- **Preview.** A tap on an item opens it full screen on the dark stage, over the same view of
  the core as the grid: a lazy horizontal pager (`LazyHStack` with paging and
  `scrollPosition`), so a 10,000-item view costs no more than a short one. The original is
  decoded by ImageIO at 1.5× the screen's pixel size, by path for a stored original and through
  the core's decrypted bytes for an item of an unlocked folder; a few stay in memory. Until it
  arrives, and for what ImageIO cannot draw, the page shows the thumbnail and the file type.
  A UIScrollView zooms (pinch, double tap); paging stops while zoomed; a single tap hides the
  bars. The browse screen keeps its view open while the preview covers it.
- **Details** (the desktop's right bar): name, rating (the same rating again clears it), tags
  with suggestions from the library (matched ignoring case, stored exactly as typed), folders
  (the tree as a picker; locked folders only while open), notes, link, and the file's facts.
  Each change is one undoable edit through `egl_apply_item_edit`; text fields save on submit
  and when the sheet closes.
- **Actions.** On a selection (a bar under the grid), from the preview, and from the long-press
  menu of a cell: rate, tags (the selection's tags with counts from `egl_view_selection_tags`,
  remove from all or add), add to folder, share, move to trash; in the Trash: restore, delete
  permanently (confirmed) and Empty Trash. Select All acts through the view target (D-094).
- **Undo.** After an action a note shows what happened, with Undo when the core can take it
  back (`egl_undo`); permanent deletes have none, as on the desktop.
- **Sharing.** Originals go to the system share sheet under their item names (`name.ext`, with
  ` (2)` for repeats), at most 100 at once: a stored original is hard-linked (copied across
  volumes) into a temporary folder, an unlocked folder's item is written there decrypted with
  complete file protection. The folder is removed when the sheet closes. The library's own
  files are never moved or changed.
- **EglCore:** `selectionTags(_:)`; public initialisers for `Tag` and `ExtensionCount`.
- **Strings check:** a key's first segment may be camelCase now.

## D-096 iOS adding: Photos, Files, paste, drops, duplicates, HEIC through ImageIO (2026-11-04)

The fourth app PR of the iOS series (D-093–D-095).

- **Sources.** The + menu (library screen and every place but the Trash; files added in a
  folder go into it) offers Photos (`PhotosPicker`, images and videos, the original encoding
  kept: HEIC stays HEIC), Files (`fileImporter`, files and folders, several at once) and Paste.
  Files can also be dropped onto a place. Photos, paste and drops hand over temporary files:
  each is copied into a folder of its own (`tmp/picked-…`) and removed when its import ends;
  copies left by a killed run are removed at launch. Files from the Files picker are not
  copied: the app holds their security scopes until the import ends. Paste reads the clipboard
  only when the user picks Paste, so the system asks once, at that moment.
- **Import.** One `egl_add_files` per batch; progress shows over the place with a way to stop;
  when it ends a note says what was added, skipped and failed. Closing the library runs the
  pending clean-ups.
- **Duplicates.** At the first duplicate of a batch the core waits (`waitingForDecision`); the
  app asks: skip, add anyway, add the existing item to the folder (when the batch goes into
  one), or stop. The answer holds for the rest of the batch (`egl_answer_import`).
- **HEIC on iOS.** The iOS core is built without libheif (D-086). The core gains a
  process-wide platform decoder (`media::set_platform_decoder`), offered through the C ABI as
  `egl_set_platform_decoder(callback, ctx)` with `egl_decode_sink_put` for the result. The app
  installs ImageIO at launch: it decodes with the orientation applied, reports the displayed
  size and hands RGBA pixels of at most 2048 on the long side (the thumbnail is far smaller).
  The core checks the sizes before it uses them. HEIC gets dimensions, a thumbnail and a
  palette on the phone; EXIF of HEIC stays a desktop feature. On the desktop the decoder is not
  set and libheif keeps working as before; when one is set it goes first.
- **EglCore:** `PlatformDecoder.install()`, `answerImport(_:_:)`, `DuplicateNotice`,
  `DuplicateAnswer`, `ImportProgress.duplicate`.
- **Tests.** Rust: the platform image is checked (sizes, pixels) and used for HEIC; the C ABI
  callback round trip. EglCore on macOS: ImageIO applies the orientation; HEIC imported through
  the decoder; a duplicate waits for an answer. The app on the simulator (where the core really
  has no libheif): picked copies are added and removed, the duplicate question and its answer
  for the batch, HEIC with dimensions and a thumbnail, clipboard file names; a UI test of the
  menu.

## D-097 iOS organizing: folders, tags, tag groups, the smart folder editor (2026-11-04)

The fifth app PR of the iOS series (D-093–D-096).

- **Folders.** The library screen always lists its folder section (when the library is
  writable), with New Folder; a folder's long-press menu offers New Subfolder, Edit (name,
  description, one of the eight label colours or none), Move To (the top level or another
  folder, never into itself; it lands after the new parent's last child) and Delete (keep the
  items, or move them to the trash too). A folder's tags and its own sort stay as they are:
  they belong to the desktop's properties panel. A locked folder offers nothing until it is
  open (the locks PR). A deleted folder is not the place the library reopens in.
- **Tags.** A Tags row opens the tag list: tags by tag group, then the others, with counts and
  search. A tag opens its items (a `tag` place: All filtered by the tag; the filter sheet hides
  its tag section there). Rename (onto an existing name the two merge), Merge Into, Add to
  Group, Remove from Group and Delete; tag groups are made, edited (name, colour, tags) and
  deleted. Names are compared exactly: tags stay case-sensitive.
- **Smart folders.** New Smart Folder, New Smart Subfolder, Edit and Delete; the browse
  screen's options offer Save as Smart Folder (the core's `egl_smart_rules` for the view's
  query). The editor follows the desktop's (`ui/src/features/smart/rules.ts`): conditions that
  all must hold, each matching all or any of its rules or negated; the v1 properties (tags,
  folders, name, notes, link, type, rating, shape, the three dates, width, height, file size)
  with their methods and value editors. The rules stay Eagle's JSON: `JSONValue` in EglCore
  keeps every key, so keys and rules this editor does not know survive an edit; such a rule
  shows as one it cannot edit and can only be removed.
- **EglCore:** `JSONValue`; `FolderChange`, `saveFolder`, `deleteFolder`; `renameTag`,
  `mergeTag`, `deleteTag`; `TagGroup`, `TagGroupChange` and their calls; `SmartFolderChange`,
  `saveSmartFolder`, `deleteSmartFolder`, `smartRules(of:)`; `Folder.orderBy`,
  `Folder.sortAscending`, `SmartFolder.conditions`; `LabelColor`. Stored colours stay strings, so
  a library imported with another colour still opens.
- **Tests.** Rust: the smart rules of a query, unknown rules kept and listed as unsupported, an
  update in place. EglCore on macOS: every edit above against the core. The app on the
  simulator: the rule model (round trip with unknown keys, methods keeping their values),
  folders made, moved (not into themselves) and deleted with their items, tags renamed, merged,
  grouped, browsed and deleted, a view saved as a smart folder; UI tests for a new folder and a
  tag's items.

## D-098 iOS locked folders, locking on leaving, and the app's login with Face ID (2026-11-04)

The sixth app PR of the iOS series (D-093–D-097). YubiKey stays a desktop feature.

- **Locked folders.** A closed lock shows its label (and colour) instead of the folder; a tap
  opens the unlock sheet. Its long-press menu offers Unlock and Use Recovery Key (the key or
  its twelve words open the folder and set a new password); a lock imported without a password
  (from Eagle) also offers Set Password and Remove Lock. An open folder offers Lock (password
  twice, an optional label and colour); an open lock offers Lock Now, Change Password, New
  Recovery Key, Lock Label and Remove Lock. Lock All sits in the library's toolbar while any
  folder is open. The recovery key is shown once after locking or renewing, with copy buttons
  (the clipboard keeps it for this device only, for two minutes) and an "I have saved it"
  switch before Done. Every lock call runs off the main thread (Argon2id). A wrong password is
  said in the sheet; other failures go to the alert. The library's views and counts follow.
- **Leaving the app locks again.** When the app goes to the background every open folder is
  locked again (`egl_relock_folder(NULL)`), and the decoded originals and thumbnails in memory
  are dropped.
- **The app's login.** App Lock (on the Welcome screen for now; Settings arrive with the next
  PR) sets a password or PIN, changes it and turns it off. It is kept as a PBKDF2-HMAC-SHA256
  hash (310,000 rounds, a salt of its own) in the Keychain, on this device only, readable while
  the phone is unlocked. Face ID or Touch ID, when the device has it, is a Keychain item that
  only biometry can read (`.biometryCurrentSet`): a new face or finger needs the password
  again. The app locks when it goes to the background and asks at launch; from the fifth wrong
  try each one makes the next wait longer (30 s, 60 s, …). The login and, while a login is
  set, a privacy cover for the app switcher live in a window above every other one, so sheets
  and the preview never show behind them. UI tests keep the login in memory.
- **Strings.** `NSFaceIDUsageDescription` is localized through `InfoPlist.xcstrings`; the
  strings check skips lines marked `// not a key`.
- **EglCore:** `lockFolder(_:password:label:labelColor:)`, `recoverFolder`,
  `changeFolderPassword`, `renewRecoveryKey`, `removeFolderLock`, `setLockLabel`; a public
  `EglError(code:message:)`.
- **Tests.** EglCore on macOS: a lock's whole life (label, recovery, change, renew, label,
  remove). The app on the simulator: hashes, the app lock (set, change, turn off, waiting after
  wrong tries), the Keychain store, a folder locked, a wrong password kept out of the alert,
  unlocked and locked again by Lock All; a UI test of the login after the app comes back.

## D-099 iOS Settings: library upkeep, backups, check and repair, export (2026-11-04)

The seventh app PR of the iOS series (D-093–D-098).

- **Settings** opens from the library screen (gear) and from Welcome. The library part:
  - **Library**: the name (renaming changes the library's own name, not its folder), where it
    lives, what it takes on disk (originals, thumbnails, database, backups) and Rebuild
    Thumbnails (the core's preview task in the background, its progress in the import banner);
  - **Backups**: Back Up Now and the list (daily, by hand, before an update, before a restore)
    with Restore, confirmed. A restore is another database: views and undo start over, so every
    open browse screen asks for its view again and the tree is read anew;
  - **Check and Repair**: the core's check (database, missing originals, files and thumbnails
    without an item) and Repair with the desktop's two choices; the core backs up first;
  - **Export**: the whole library, or folders (with or without their subfolders) and tags; a
    ZIP archive (four compression levels, optionally encrypted with an age passphrase, never
    stored) or a folder tree; the preview counts items, bytes and items a closed locked folder
    leaves out. The export is written into the app's temporary folder, then saved to Files
    (`fileMover`) or shared; the temporary copy goes when the screen closes or a new export
    starts. Eagle is not a destination on the phone.
  The app part: App Lock (moved here from Welcome), the version, the language with a link to the
  system settings, where iOS keeps the app's language.
- **The preview's position** (D-095) follows the scroll offset on iOS 18 and later (offset over
  page width): `scrollPosition` and the visible-target callback did not always follow a quick
  swipe on a busy device, which made the preview UI test fail now and then on `main`.
- **EglCore:** `stats`, `backups`, `backUpNow`, `restoreBackup`, `check`, `repair`,
  `generatePreviews(rebuild:)`, `exportPreview`, `startExport`, `exports`, `cancelExport`, and
  their models (`LibraryStats`, `Backup.kind`, `LibraryCheck`, `RepairOptions`,
  `PreviewsTicket`, `ExportSelection`, `ExportFormat`, `ExportRequest`, `ExportPreview`).
- **Tests.** EglCore on macOS: stats, a backup, a clean check and repair, a rebuild, an
  encrypted ZIP export that is not a plain ZIP. The app on the simulator: rename, back up, trash,
  restore (the view comes back, the state before the restore is kept), a clean check, a ZIP and
  a folder export; a UI test of Check Library and an export from Settings.


## D-100 iOS release: icon, privacy, accessibility, 10,000 items, tagged releases (2026-11-05)

The last PR of the iOS series (D-093–D-099).

- **Icon.** One 1024 px PNG in `Assets.xcassets/AppIcon` (Xcode makes the other sizes), drawn
  from the desktop icon as a full square without transparency: iOS rounds the corners itself.
  `scripts/check-mobile-shell.sh` checks its size and that it has no alpha.
- **Privacy.** `PrivacyInfo.xcprivacy`: no tracking, no collected data; the APIs that need a
  reason are `UserDefaults` (CA92.1, the app's own settings) and file timestamps (C617.1 for
  the app's container, 3B52.1 for libraries the user picked in Files: the Rust core reads file
  metadata). `NSPhotoLibraryAddUsageDescription` joins `NSFaceIDUsageDescription`: the share
  sheet's Save Image writes to Photos and iOS ends an app without that string. Photos are read
  through the system picker, which needs no permission. `check-ios-strings.py` checks that
  every privacy string of Info.plist is in `InfoPlist.xcstrings` in the three languages.
- **Accessibility.** Ratings read as "3 stars", not the symbols (a shared `StarsText`); grid
  cells say their rating and "Loading" until they arrive; the folder pickers mark the chosen
  folder as selected. A UI test runs Xcode's accessibility audit (sufficient element
  descriptions) over the library, the grid, the preview and the details. Its first run on
  `main` found the item image at the top of the details without a description; it is named
  after the item now.
- **10,000 items.** A unit test fills a library with 10,000 generated images (D-096's sample
  generator, now walking all windows to tag its first four) and times the paths that must not
  grow with the library: opening the view, a window at the end, the name sort, a search, a tag
  filter, Select All (a rule over the view, no id list). The budgets (3–5 s) are loose for
  shared CI simulators; the timings go to the log as `perf:` lines.
- **Releases.** A tag `ios-vX.Y.Z` runs the Mobile workflow (path filters do not apply to
  tags): the device build takes `X.Y.Z` as its version and the run number as its build number,
  and checks that the app says so; once `ios-core`, `ios-app` and `ios-ipa` pass, `ios-release`
  publishes a GitHub Release `ios-vX.Y.Z` with `EglViewer-X.Y.Z.ipa` and `SHA256SUMS`, and the
  changes since the previous `ios-v` tag. It is created with `--latest=false`: the desktop
  updater reads `releases/latest/download/latest.json` (D-062), and an iOS release marked
  latest would hide the desktop's. `scripts/ios-release-version.sh` turns the tag into the
  version and refuses anything else. The rolling `ios-latest` pre-release from `main` stays
  (D-086). The IPA is unsigned: it is installed by signing it with one's own Apple ID.
- **Version 0.2.3 everywhere:** the workspace crates (and `Cargo.lock`), the desktop
  (`tauri.conf.json`, `ui/package.json`), Android (`versionName`, the version the shell gives
  the core) and iOS (`MARKETING_VERSION`). The first iOS release tag is `ios-v0.2.3`.
- **Desktop releases from the tag too.** A tag `vX.Y.Z` already built the three installers
  (D-062); a new `release` job in `ci.yml` now publishes them once all three pass: the NSIS
  installer, the DMG, the macOS updater archive and the AppImage under the names of earlier
  releases (`egl-viewer_X.Y.Z_x64-setup.exe`, `_aarch64.dmg`, `_aarch64.app.tar.gz`,
  `_amd64.AppImage`; Tauri names the macOS archive without the version), their `.sig` files and
  `latest.json` written by `scripts/write-latest-json.mjs`, marked as the latest release. It
  refuses a tag that differs from `tauri.conf.json` and needs the notes in
  `docs/releases/X.Y.Z.md`, which the iOS release of the same version shows too. Android is
  not released: its shell is a draft over a mock core (D-083).


## D-101 Android A1 foundation: unfreeze, Kotlin eglcore, real core, app storage + SAF (2026-11-05)

Android was frozen while the iOS app landed on `EglCore` (D-092). iOS is done (D-093–D-100).
This is the first PR of the Android series — the mirror of D-093. Later stages ship as
separate PRs after merge:

| Stage | Theme (≈ iOS) |
| --- | --- |
| **A1** (this PR) | Foundation: Kotlin core, real `.so`, Welcome, places/counts |
| **A2** | Browsing (D-094): places, lazy grid by view id, thumbs, filters, selection |
| **A3** | Preview / details / undo (D-095) |
| **A4** | Adding from gallery / folders, duplicates, HEIC platform decoder (D-096) |
| **A5** | Organizing: folders, tags, smart folders (D-097) |
| **A6** | Locked folders + app login / biometrics (D-098) |
| **A7** | Settings: backups, check/repair, export (D-099) |
| **A8** | Release: icon, privacy, a11y, 10k, `android-vX.Y.Z` (D-100) |

- **Unfreeze.** The draft Compose shell (D-083 / D-085) is no longer the product path. The
  mock library fallback is gone: create/open always go through the real core, or fail with
  `EglError`.
- **Kotlin `eglcore` module** (`android/eglcore`, package `app.eglviewer.core`): typed
  `Library`, `EglError` (status codes 1–9), JSON models (`LibraryInfo`, `LibraryCounts`,
  event stub), JNI forwarder for create / open / close / info / counts / folder tree / event
  callback. Host JVM unit tests run without a device.
- **Real `.so` in CI.** Mobile `android` installs the NDK, builds
  `libegl_library.so` with cargo-ndk (`arm64-v8a`) into `eglcore/src/main/jniLibs`, then
  `testDebugUnitTest` + `assembleDebug`. CMake stays gated on the `.so` (or `-Pegl.ffi=true`)
  so a local Kotlin-only assemble still needs no NDK; without the `.so` the APK starts but
  refuses to open a library.
- **No libheif on Android targets** (same idea as iOS D-086): `media` skips `libheif-rs` for
  `target_os = "android"` so the NDK link does not need a system HEIF. A4 will add a platform
  decoder for gallery HEIC.
- **Where libraries live.** New libraries go to app-specific storage (`files/libraries`). A
  library picked through SAF (`OpenDocumentTree`) keeps a persistable URI grant and a recent
  entry (last 10). A path or provider URI that looks cloud-synced opens read-only (`syncFolder`,
  D-021).
- **`AppModel`.** Welcome or one `OpenLibrary`; every core call off the main thread. The open
  library screen shows places with counts (and a folders count); the item grid is A2.
- **Strings** in English, Russian and Simplified Chinese (the desktop's three languages,
  D-069), under `res/values*`.
- **Docs.** `android/README.md`, the README status table, and this decision. Architecture notes
  the unfrozen Android app on `eglcore`.
- **Next PR after merge: A2 browsing.**



## D-102 Android A2 browsing: places, lazy grid by view id, saved view per library (2026-11-05)

The second PR of the Android series (D-101), mirroring iOS D-094. Stacked on
`feat/android-app-foundation` (A1).

- **Places.** The library screen lists All, Uncategorized, Untagged and Trash with counts, the
  folder tree (a folder shows its subfolders' items) and the smart folders. A locked folder
  shows its lock label and does not open; unlocking comes with A6. Each place opens
  `BrowseScreen`.
- **Windows by view id.** `BrowseModel` opens one core view per query (`egl_open_view`) and
  asks for cells in windows of 100 around what the user sees (`egl_view_range`; the core serves
  at most 200). The item list never crosses JNI. `viewsInvalidated` events reach open models
  through `OpenLibrary`; a model takes the new total and reloads windows near the last cell
  seen. Answers for an older query or refresh are dropped by generation counters.
- **Thumbnails** are the core's WebP bytes (`egl_thumbnail_bytes`), decoded by `BitmapFactory`
  off the main thread at the cell's pixel size and kept in an `LruCache` by cost (64 MB). The
  key has the thumbnail version. Until it arrives a cell shows its main colour (`palette0`), or
  the file type when it has no thumbnail.
- **Controls.** Grid or list, three thumbnail sizes, the sort (desktop keys except manual, tag
  and duration) with ascending, search debounced 250 ms, and a filter sheet: tags with any /
  all / none, file type with counts, rating (unrated or exactly 1–5 stars), shape. Each change
  opens a new view.
- **Selection.** Select or a long press starts it; Select All is the core's target
  `{viewId, all: true, ids: exclusions}` (desktop D-089), so it never lists the view's ids.
  Actions on a selection arrive with A3.
- **Saved view** (as D-078 / D-094), per library path in `SharedPreferences`: the sort, layout
  and size for the whole library, and the last place with its filters and search. The library
  opens in that place again; going back to the list of places forgets it, and filters of one
  place do not carry over to another.
- **eglcore.** JNI and `Library` gain `openView` / `viewRange` / `closeView`, tags, extensions,
  smart folders, thumbnail bytes and `editItems`; `LibraryEvent.ViewsInvalidated` carries
  typed `ViewOpened` rows.
- **Tests.** JVM unit tests of `BrowseModel` against an in-memory `FakeViewSession` (windows,
  sort, search, filters, Select All with a trash refresh, the saved view), in
  `:app:testDebugUnitTest` of the Mobile workflow.
- **Docs.** `android/README.md`, the README status table, and this decision.
- **CI.** Mobile `android` pins `cargo-ndk` **4.1.2** (`--locked`): 3.5.6 / 3.5.7 were yanked
  on crates.io and broke the A2 PR install step; 4.1.2 is the newest stable older than the
  supply-chain release age (D-002 / D-003).
- **Out of scope.** Preview/details/undo (A3), gallery add (A4), organize / locks / settings /
  release.

## D-103 Android A3 preview, details and actions with undo (2026-11-05)

The third PR of the Android series (D-101, D-102), mirroring iOS D-095. Stacked on
`feat/android-a2-browsing` (A2).

- **Preview.** A tap opens a full-screen dark stage over the same core view as the grid: a
  `HorizontalPager` of the view's positions, so a large library costs no more than a short
  one. The original is decoded by `BitmapFactory` at about 1.5× the screen's long edge (path
  via `egl_original_path`, else decrypted bytes via `egl_original_bytes`); a few stay in
  `OriginalCache`. Until it arrives, and for formats the decoder cannot draw, the page shows
  a spinner or the file type. Pinch / double-tap zooms; paging stops while zoomed; a single
  tap hides the bars.
- **Details** (the desktop's right bar): name, rating (the same rating again clears it), tags
  with suggestions from the library (matched ignoring case, stored exactly as typed), folders
  (the tree as a picker; locked folders only while open), notes, link, and the file's facts.
  Each change is one undoable edit through `egl_apply_item_edit`; text fields save on Done and
  when the sheet closes.
- **Actions.** On a selection (a bar under the grid), from the preview, and from a long-press
  start of selection: rate, tags (counts from `egl_view_selection_tags`), add to folder, share,
  move to trash; in the Trash: restore, delete permanently (confirmed) and Empty Trash. Select
  All still acts through the view target (D-102).
- **Undo.** After an action a snackbar shows what happened, with Undo when the core can take
  it back (`egl_undo`); permanent deletes have none, as on the desktop.
- **Sharing.** Originals go to the system share sheet under their item names (`name.ext`, with
  ` (2)` for repeats), at most 100 at once: copied into a temporary cache folder (FileProvider),
  never hard-linking or mutating the library's own files. The folder is removed when the share
  returns.
- **eglcore.** JNI and `Library` gain `item`, `applyItemEdit`, typed `ItemEdit` for
  `editItems`, `selectionTags`, `undo` / `undoStatus`, `deleteItems`, `emptyTrash`,
  `trashInfo`, `viewIds`, `originalPath`, `originalBytes`; models `Item`, `SelectionTags`,
  `UndoStatus`, `TrashInfo`.
- **Tests.** JVM unit tests of `ItemEdit` JSON, details edits with undo, selection
  trash/restore/delete, share naming, and preview open state against `FakeViewSession`, in
  `:eglcore:testDebugUnitTest` / `:app:testDebugUnitTest` of the Mobile workflow.
- **Docs.** `android/README.md`, the README status table, and this decision.
- **Out of scope.** Gallery add (A4), organize / locks / settings / release.

## D-104 Android A4 adding: gallery, Files/folders, paste, drops, duplicates, HEIC via ImageDecoder (2026-11-05)

The fourth PR of the Android series (D-101…D-103), mirroring iOS D-096. Stacked on
`feat/android-a3-preview` (A3).

- **Sources.** The + menu (library screen and every place but the Trash; files added in a
  folder go into it) offers Gallery (`PickMultipleVisualMedia`, images and videos, original
  encoding kept), Files (`OpenMultipleDocuments`), Folder (`OpenDocumentTree`) and Paste.
  Files can also be dropped onto a place. Gallery, paste and drops hand over temporary files:
  each is copied into a folder of its own (`cache/picked-…`) and removed when its import ends;
  copies left by a killed run are removed at launch. Files/folders from SAF: when a document
  resolves to a primary-storage path the app keeps the URI grant until the import ends;
  otherwise the bytes are copied into `picked-*` while the grant is held (Android content URIs
  have no durable path the core can read). Paste reads the clipboard only when the user picks
  Paste.
- **Import.** One `egl_add_files` per batch; progress shows over the place with Stop; when it
  ends a toast says what was added, skipped and failed. Closing the library runs the pending
  clean-ups.
- **Duplicates.** At the first duplicate of a batch the core waits (`waitingForDecision`); the
  app asks: skip, add anyway, add the existing item to the folder (when the batch goes into
  one), or stop. The answer holds for the rest of the batch (`egl_answer_import`).
- **HEIC on Android.** The Android core is built without libheif (D-101). The app installs
  `PlatformDecoder` at launch: JNI registers `egl_set_platform_decoder`, and ImageDecoder
  (API 28+) decodes with orientation applied, reports the displayed size and hands RGBA pixels
  of at most 2048 on the long side. The core checks the sizes before it uses them. HEIC gets
  dimensions, a thumbnail and a palette on the phone when the device can decode HEIC; EXIF of
  HEIC stays a desktop feature.
- **eglcore.** JNI and `Library` gain `addFiles`, `answerImport`, `cancelImport`,
  `importStatus`, `setPlatformDecoder` / `decodeSinkPut`; models `ImportTicket`,
  `ImportProgress`, `DuplicateNotice`, `DuplicateAnswer`; typed `LibraryEvent.ImportProgress`.
- **Tests.** JVM unit tests of import JSON, picked-copy lifecycle, clipboard file names, the
  import summary, and platform-decoder constants / rejection of junk, in
  `:eglcore:testDebugUnitTest` / `:app:testDebugUnitTest` of the Mobile workflow.
- **Docs.** `android/README.md`, the README status table, and this decision.
- **Out of scope.** Organizing (A5), locks / login (A6), settings (A7), release (A8).

## D-105 Android A5 organizing: folders, tags, tag groups, the smart folder editor (2026-11-05)

The fifth PR of the Android series (D-101…D-104), mirroring iOS D-097. Stacked on
`feat/android-a4-adding` (A4).

- **Folders.** The library screen always lists its folder section (when the library is
  writable), with New Folder; a folder's long-press menu offers New Subfolder, Edit (name,
  description, one of the eight label colours or none), Move To (the top level or another
  folder, never into itself; it lands after the new parent's last child) and Delete (keep the
  items, or move them to the trash too). A folder's tags and its own sort stay as they are:
  they belong to the desktop's properties panel. A locked folder offers nothing until it is
  open (the locks PR). A deleted folder is not the place the library reopens in.
- **Tags.** A Tags row opens the tag list: tags by tag group, then the others, with counts and
  search. A tag opens its items (a `tag` place: All filtered by the tag). Rename (onto an
  existing name the two merge), Merge Into, Add to Group, Remove from Group and Delete; tag
  groups are made, edited (name, colour, tags) and deleted. Names are compared exactly: tags
  stay case-sensitive.
- **Smart folders.** New Smart Folder, New Smart Subfolder, Edit and Delete; the browse
  screen offers Save as Smart Folder (the core's `egl_smart_rules` for the view's query). The
  editor follows the desktop's (`ui/src/features/smart/rules.ts`): conditions that all must
  hold, each matching all or any of its rules or negated; the v1 properties (tags, folders,
  name, notes, link, type, rating, shape, the three dates, width, height, file size) with
  their methods and value editors. The rules stay Eagle's JSON: `JSONValue` in eglcore keeps
  every key, so keys and rules this editor does not know survive an edit; such a rule shows as
  one it cannot edit and can only be removed.
- **eglcore.** `JSONValue`; `FolderChange`, `saveFolder`, `deleteFolder`; `renameTag`,
  `mergeTags`, `deleteTag`; `TagGroup`, `TagGroupChange` and their calls; `SmartFolderChange`,
  `saveSmartFolder`, `deleteSmartFolder`, `smartRules`; `Folder` description / colour / tags /
  sort; `SmartFolder.conditions` / `unsupported`; `LabelColor`. Stored colours stay strings, so
  a library imported with another colour still opens.
- **Tests.** JVM unit tests of organize JSON, `JSONValue` unknown keys, and the smart rule
  model (round trip with unknown keys, methods keeping their values), in
  `:eglcore:testDebugUnitTest` / `:app:testDebugUnitTest` of the Mobile workflow.
- **Docs.** `android/README.md`, the README status table, and this decision.
- **Out of scope.** Locks / login (A6), settings (A7), release (A8).

## D-106 Android A6 locks + app login: locked folders, lock on background, PIN / biometrics (2026-11-05)

The sixth PR of the Android series (D-101…D-105), mirroring iOS D-098. Stacked on
`feat/android-a5-organizing` (A5 / D-105). YubiKey stays a desktop feature.

- **Locked folders.** A closed lock shows its label (and colour) instead of the folder; a tap
  opens the unlock sheet. Its long-press menu offers Unlock and Use Recovery Key (the key or
  its twelve words open the folder and set a new password); a lock imported without a password
  (from Eagle) also offers Set Password and Remove Lock. An open folder offers Lock (password
  twice, an optional label and colour); an open lock offers Lock Now, Change Password, New
  Recovery Key, Lock Label and Remove Lock. Lock All sits in the library's toolbar while any
  folder is open. The recovery key is shown once after locking or renewing, with copy buttons
  and an "I have saved it" switch before Done. Every lock call runs off the main thread
  (Argon2id). A wrong password is said in the sheet; other failures go to the toast. The
  library's views and counts follow.
- **Leaving the app locks again.** When the activity stops, every open folder is locked again
  (`egl_relock_folder(NULL)`), and the decoded originals and thumbnails in memory are dropped.
- **The app's login.** App Lock (on the Welcome screen for now; Settings arrive with A7) sets a
  password or PIN, changes it and turns it off. It is kept as a PBKDF2-HMAC-SHA256 hash
  (310,000 rounds, a salt of its own) in app-private SharedPreferences. Biometrics, when the
  device has them, use `BiometricPrompt` with a Keystore AES key
  (`setInvalidatedByBiometricEnrollment(true)`, strong biometrics only): a new enrollment needs
  the password again. The app locks when it goes to the background and asks at launch; from the
  fifth wrong try each one makes the next wait longer (30 s, 60 s, …). While a login is set,
  `FLAG_SECURE` and a privacy cover hide the library in Recents. JVM tests keep the login in
  memory.
- **eglcore.** `RecoveryKey`; `lockFolder`, `unlockFolder`, `relock`, `recoverFolder`,
  `changeFolderPassword`, `renewRecoveryKey`, `removeFolderLock`, `setLockLabel`; JNI forwarders
  for the matching `egl_*` calls.
- **Dependencies.** `androidx.biometric:biometric:1.1.0`, `androidx.fragment:fragment-ktx:1.8.5`
  (`FragmentActivity` for `BiometricPrompt`). `MainActivity` is a `FragmentActivity`.
- **Tests.** JVM unit tests of `LoginHash` / `AppLock` (set, change, turn off, waiting after
  wrong tries), `RecoveryKey` / folder lock JSON, in `:eglcore:testDebugUnitTest` /
  `:app:testDebugUnitTest` of the Mobile workflow.
- **Docs.** `android/README.md`, the README status table, and this decision.
- **Out of scope.** Settings (A7), release (A8).
