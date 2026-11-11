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

The project started from the newest stable releases available that day, subject to D-003:

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
`eagle-api` (added with the live connection, D-013) maps Eagle's errors onto it one to one. architecture.md §15.2 is updated
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

~~All UI strings are English until localisation adds Russian and English through a
message catalogue. New strings go through one place per screen, so extracting them later is
mechanical. Confirmed on 2026-10-28: the first version ships in English only; localisation is in
the "Out of scope" list of spec §9.~~

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

## D-010 Libraries added when a feature needs them (2026-10-28)

The first slice has a single screen, so TanStack Router and TanStack Query (architecture.md §8) are not
installed yet. They arrive with the first feature that needs routing or server state (the grid
slice). The same holds for the store plugin (dropped by D-022), and for HTTP (`reqwest`) and the secret store
(`keyring`), which come with the Eagle connection (D-013).

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
a late addition. The spec and the architecture were re-prioritised the same day:

- The first slices build the native library: create and open, adding files and the media pipeline, the
  grid, folders, tags, search and smart folders, editing and trash.
- Then Eagle, in the order users need it: import an Eagle library, connect to a running
  Eagle (live connection and writes), export into Eagle. Hardening and the release come last.
- The Eagle protocol content, the 41 corrections and the fixtures are unchanged; they moved to
  spec §8. The `LibrarySource` trait makes this a reordering, not a rewrite.
- The existing code stays: the Connect screen becomes one of the Eagle screens, reached from
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
secondary actions under "Eagle". The existing Connect screen is reached from there and has a Back
button. On start the app reopens the last library unless it was closed explicitly.

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
Detail: `docs/age-locked-folders.md`. Revisit if a
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
- The storage of locked folders is reserved in version 1 and stays empty until folder locks land:
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
  change in Settings → General (D-027), with the same location check. Changing it does
  not move existing libraries.
- Detection is best effort, in `app-store` behind a trait: the `OneDrive`, `OneDriveConsumer` and
  `OneDriveCommercial` variables; the sync roots registered with the Windows Cloud Files API
  (`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager\*\UserSyncRoots`:
  OneDrive, recent Dropbox, iCloud, Box); the markers `.dropbox.cache` and `.stfolder` in the
  folder or an ancestor; a network path when the canonical path is a UNC path. Google Drive and
  Yandex Disk are not guaranteed; add them when a reliable signal is known. The registry reader is
  a new dependency, chosen and recorded when the shell code landed.
- Create is refused in a sync folder, on a network path, and inside another native library or an
  Eagle `.library` folder. Open: a library in a sync folder opens read-only with `immutable=1`
  (SQLite writes no `-wal` or `-shm` there) and a message to move the folder out while the app is
  closed; a network path is refused. No override in v1; "Move library…" can come with the library
  settings slice.
- This replaces "warn and offer the backups" in spec §3.

## D-022 Settings and recent libraries are app-store files (2026-10-28)

- `app-store` writes small JSON files in the app's local data folder
  (`%LOCALAPPDATA%\com.fighxy.eglviewer`; not Roaming, because the paths belong to one computer):
  re-read, change, write a temp file, rename. Several app processes (D-024) merge instead of
  overwriting each other, and the code is tested without Tauri.
- `tauri-plugin-store` is not used (D-010 expected it early): it keeps a file in memory and
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
- `library-native` gets `add_file` for one file first, with tests and without a command or
  UI, so the schema is exercised before the grid. Batches, the media pipeline and the UI stay in
  batch add. Later: when no item has the same size, the file cannot be a duplicate, which saves the
  lookup for large imports.

## D-024 Several processes, one window and one library each (2026-10-28)

- No single-instance plugin: a second app process may run, and on the same library it is
  read-only (the create/open done check). Two libraries at the same time means two processes until
  multi-window support comes.
- Switching libraries in a window opens and checks the new one first and closes the current one
  only on success.
- `AppState` holds `Option<OpenLibrary>` behind one accessor, so a map per window is a local
  change later.
- `LibraryInfo` gains `id`, `readOnly` and `readOnlyReason` (`otherInstance`, `syncFolder`,
  `readOnlyMedium`). `Capabilities.locked_folders` is false for the native library until folder locks land.
  App login (spec §12.2) stays outside the first slice; Welcome remains the entry route, so a gate can be
  put in front of it later.

## D-025 Native window frame, the app's own menu bar, one command registry (2026-10-28)

- The first slice keeps the native Windows frame and puts the app's own menu bar (shadcn `Menubar`) under
  it. Snap Layouts, the system shadow and the dark title bar come from Windows.
- Rejected: the native Tauri menu, whose classic Win32 menu bar does not match the shadcn look and
  the dark theme. Postponed: a custom title bar that merges the title and menu rows.
  Its HTML caption buttons lose the Snap Layouts of Windows 11 unless a plugin such as
  `tauri-plugin-decorum` restores them (a new dependency, to be checked then). Moving the menu bar
  into a custom title bar moves the component, not the menu.
- One command registry (`ui/src/commands/`) feeds the menu bar, context menus, shortcuts, the
  Keyboard Shortcuts dialog and a later command palette. Commands of later features are absent, not
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
- Grid: justified rows from item aspect ratios (D-067; the grid shipped squares first). List
  unchanged. Masonry can still come later. Double-click and `Space` open the full-size preview
  (`Space` hold-vs-toggle and letterbox click-out: D-073); `Enter` moves focus to the right bar
  (spec §7 said before that `Enter` opens the detail).
- Right bar: fields save on `Enter` or blur, `Esc` reverts, no Save button. With several items it
  shows batch edits; with nothing selected, the open folder's properties.
- The shell builds the whole frame with empty zones, so later features fill zones instead of
  restructuring the screen.

## D-027 Settings and About are dialogs (2026-10-28)

- Settings is a dialog (`Ctrl+,`) with sections on the left, not a screen and not a second window
  (one window, D-024). General has: theme, reopening the last library on start, and the
  default folder for new libraries (D-021). Library settings stay a separate dialog in the
  Library menu.
- About shows the version, build date and commit, the WebView2 version, links and the
  third-party licences. The licence file is generated by the release build, because
  the MIT and Apache licences of the dependencies require showing their texts. Open Log Folder in
  the Help menu waits for logs written to a file.

## D-028 Look and motion (2026-10-28)

- The theme follows Windows by default, with a System / Light / Dark override in the View menu and
  in Settings. The grid background is a neutral grey. Desktop density: rows about 28–32 px, body
  text 13–14 px.
- Not now: Mica or Acrylic backgrounds of Windows 11. They need a transparent window and WebView,
  a performance risk with a large grid; revisit before release.
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
- Which of these are Tauri or WebView2 settings and which are UI code was settled when the shell was
  built; arch §8 lists them.

## D-030 Dependencies of the shell slice (2026-10-28)

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

The shadcn CLI downloads components from `ui.shadcn.com`, which the build environment of that slice
could not reach. `menubar`, `dialog` and `switch` in `ui/src/components/ui/` were written by hand
on the `radix-ui` primitives already installed, in the style of the existing Nova components. They
belong to the project like CLI output (D-006); `pnpm dlx shadcn@<pinned> add <name> -c ui` can
replace them when the registry is reachable. No new npm package was needed.

## D-032 Create/open library and Welcome as built (2026-10-28)

- `library-native` exposes `Library` directly; the commands use it through `src-tauri/src/session.rs`,
  a Tauri-free layer that the tests drive. The native `LibrarySource` implementation arrives in
  the grid slice with `load_scope`, when there is something to load.
- Holding `Shift` on start (D-017, D-022) is not implemented yet: reading the key state needs a
  Windows call, and `unsafe` is forbidden in our crates, so it needs a crate with a safe wrapper,
  chosen separately. `--no-reopen` and the Settings switch "Reopen the last library on start" work.
- Daily backups are named by the UTC date. The library's display name is `meta.name`.
- The left bar shows its four rows without counts; counts come with the queries of the grid slice. The
  screens switch without TanStack Router (D-010): two screens do not need it yet.
- A read-only open of a library that needs a migration is refused with `notSupported`; with only
  format version 1 this cannot happen yet.
- Checked on Linux under Xvfb with the real app: create, quit, restart (reopened), a second
  instance (Welcome notice, then read-only with Open for editing), `kill -9` and restart (lock free,
  unclean close detected). The Windows checks of the done check are still open.

## D-033 Dependencies of the add-files slice (2026-10-28)

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

## D-037 Adding files and the media pipeline as built (2026-10-28)

- **The duplicate question:** the first duplicate of a batch pauses the import and asks "Skip
  duplicates", "Add anyway" or "Cancel import"; the answer holds for the rest of the batch. "Add the
  existing item to the target folder" joins with folders.
- **Waiting for later features:** mirroring dropped folders; tags and rating for a batch.
  The add dialog is the system file picker without extra options.
- **Paste** is Edit → Paste (`Ctrl+V`), new in the menu table of spec §7.1. Text fields keep their
  own `Ctrl+V`. A pasted image is named `Pasted YYYY-MM-DD HH-MM-SS.png` in UTC; the time zone
  needs a dependency that the add-files slice does not otherwise need.
- **Previews** run on a pool of at most four threads. `thumb_version = -1` marks a file that cannot
  be decoded, so it is not tried again; previews that a crash or a cancel left undone are made the
  next time the library opens, as a "Previews" task that does not hold back quitting.
- **Quitting or closing while adding:** the window's close is held back and the UI asks
  (`close-requested`); closing or switching the library asks the same in the UI. D-072 extends
  that confirmation to every quit, not only while imports or exports are running.
- **Counts** of the left bar and the toolbar arrive with add files (D-032 expected them with the grid):
  with files going in, the screen needs to show that they did.
- **Checks:** 1 020 files (1 000 previews, 20 PDF placeholders) in 2.6 s in a release build on Linux
  (the ignored test `a_thousand_mixed_files`). In the real app under Xvfb: pasting an image from
  the clipboard, adding files through the GTK file dialog, the Imports panel and the duplicate
  question. The Windows checks are still open.

## D-038 Dependencies of the grid slice (2026-10-28)

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
  end, before the final progress goes out. Patching snapshots per item comes with the edit slice.
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

## D-041 Grid, views and previews as built (2026-10-28)

- **`library-core`** has the pipeline of arch §6.4 in full (lock filter, subfolder union, tag and
  folder chips, type, rating, shape, search ids, sort), although folders, tags and search arrive
  in later slices: the core tests of spec §10 already run against it. An empty tag or folder
  selection filters nothing, whatever the mode. Shapes follow Eagle (arch §16.6), including the
  panoramic ones.
- **The native `LibrarySource`** reads scopes, the item, tags, thumbnails and originals. Folders,
  tag groups and smart folders answer empty lists until their slices; search answers
  `notSupported` until FTS5. The app reads the async trait on a blocking thread with a
  single poll (the native source never waits); a waiting source (Eagle) needs a runtime
  there.
- **The sort menu** shows ten keys. Duration (no video in v1) and Manual (folders) are
  supported by the sorter but hidden. The global sort, the layout (grid or list), the thumbnail
  size (80–320 px) and Show Names are preferences (`BrowsePreferences`); the last query per
  library comes with the release slice.
- **Thumbnail size** is two View menu items, Larger and Smaller (`Ctrl+=`, `Ctrl+-`), plus the
  slider and `Ctrl` + wheel; the item at the top stays in place when the size changes. Images
  smaller than their cell are not enlarged.
- **The details panel** is read-only: preview, palette, name, rating, tags, note, link, file info
  and EXIF. Several selected items show their count; the stack of thumbnails and the total size
  come with batch edits.
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

## D-043 Folders as built (2026-10-28)

Folders are built in two parts: **first** (folders, membership, Include subfolders, the folder sort,
manual order) and **then** locked folders (age, Argon2id, the recovery secret, item keys, the
Locked folders section). The first part's done check is the folder one without the locked-folder part.

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
  the edit slice.
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
  "not available yet" until tags and batch edits land.
- **New Folder** (`Ctrl+Shift+N`, the Library menu) creates a folder at the top level and opens
  it; New Subfolder, Rename (`F2`, double click), Colour, Move To and Delete are in the tree's
  context menu. A folder deleted while it is open answers an empty view, and the screen goes
  back to All.
- **Not yet:** `item_fts.folders` is written with search; covers come with the folder
  covers later; removing a batch from a folder waits for the batch panel.
- **Checks:** the 10 000-item demo library in the real app under Xvfb: creating, nesting by
  drag, colour, dragging 12 items onto a folder, Select All into a folder, Include subfolders,
  Manual with a drag to the front, the context menu, deleting a folder. The Windows checks are
  still open.

## D-044 Dependencies of locked folders (2026-10-28)

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

## D-045 Locked folders as built (2026-10-28)

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
  the Eagle import.
- **Checks:** the demo library in the real app under Xvfb: lock a folder of 12 items (also in
  another folder), the recovery key dialog, the counts and the Locked folders section, a wrong
  password, unlocking (the folder opens with decrypted previews and originals), Lock All
  Folders, removing the lock (the original file names come back). The Windows checks are still
  open.

## D-046 Tags as built (2026-10-28)

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
  the filter slice adds type, rating, shape and folders.
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
- **The search row's tags column** stays empty until search fills `item_fts` from the tables.
- **Checks:** the 10 000-item demo library in the real app under Xvfb: tagging one item
  (`Poster`, `Swiss` and `poster` as three tags), tagging a selection of three with dimmed tags
  and "add to every item", the four modes on two tags (Any 3, All 3, Exact 2, None 9 997),
  renaming `Poster` onto `poster` (the merge notice, then one tag on every item), a coloured tag
  group, a group tag opening All filtered, removing the filtered tag (the empty result with
  Clear filters). The Windows checks are still open.

## D-047 Search and format version 3 (2026-10-29)

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

## D-048 Filters and smart folders (2026-10-29)

- **The filter bar** has Tags, Folders (the four modes over folder ids), Type (Images, then the
  library's extensions with counts from the new `extensions` call), Rating (unrated or 1–5) and
  Shape (Eagle's five shapes, panoramic ones included), Clear all and Save as smart folder.
  Filters stay when the place changes; the Filter button counts them.
- **Smart folders** have their own tree in the left bar, nested, a new one after its siblings
  (moving and reordering wait for later, and so do counts). A smart folder shows the
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

## D-049 Dependencies of edits and trash (2026-10-29)

The newest versions older than the minimum release age (D-002, D-003):

| Where | Package | Version | Why |
|---|---|---|---|
| Rust | `tauri-plugin-opener` | 2.7.2 | Open with the default app, Show in Explorer (`SHOpenFolderAndSelectItems` on Windows) and links in the browser. Used as a library: its free functions only. The plugin is not registered, so the WebView gets no opener commands and the UI cannot open a path itself |
| Rust | `drag` | 2.1.1 | Dragging originals out of the window (OLE `DoDragDrop` on Windows, GTK on Linux). It brings `windows` 0.52 next to Tauri's newer `windows`; only a second copy at build time |

`rusqlite` turns on its `backup` feature (no new crate) for restoring a backup into the open
connection (D-051). Copy File uses `arboard`, already there for Paste.

## D-050 Item edits, renames and undo (2026-10-29)

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

## D-051 Trash, backups and maintenance (2026-10-29)

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

## D-052 File actions and the right bar (2026-10-29)

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
  the hardening slice. Eagle import, the live connection and export are not in 0.1.1.

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

Two locks, and they do not share keys (spec §12).

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
- **Still open.** Authenticode / Apple notarization are separate; SmartScreen may still
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

- D-029 already said the WebView2 context menu is replaced by the app's own. The first slice still
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
from the menu and still add into the library as usual. Covered by `LibraryScreen` and the
item-actions UI tests.

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

## D-075 Eagle connection libraries: reqwest, url, keyring, wiremock (2026-11-01)

The Eagle connection (docs/spec.md §8.3) adds four pinned
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

## D-076 eagle-api writes, adds, same-PC thumbnails (2026-11-01)

The writes slice (docs/spec.md §8.3) turns a live Eagle session writable and serves
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

## D-077 Export into a running Eagle (2026-11-01)

The export slice (docs/spec.md §8.4, docs/architecture.md §15.7) exports a native
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

Hardening: after reopen (D-022), restore where the user was
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

Tightens the hardening slice’s “token never reaches the WebView longer than needed”.

- **Connect.** On submit, the Connect form copies the token into the IPC payload
  and clears the password field immediately. Rust wraps it in `SecretString`
  before storing in the keyring (D-010 / D-075).
- **Export into Eagle.** The Export dialog clears its token field once
  `start_export` accepts the request.
- **Still true.** `EagleSessionInfo` has no token field; POST bodies keep the
  token in JSON only; there is still no crash-report pipeline to scrub. Logs
  follow D-079.

## D-081 Accessibility polish; custom title bar still postponed (2026-11-01)

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
imported row came from (the Eagle import) and what an export into a running Eagle created (the live-connection writes).
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

## D-088 Data-safety fixes from the pre-release review (2026-11-03)

A review of the work merged with the edits slice found these defects in the core crates; each fix has
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

The pre-release review found these defects in the app layer (`src-tauri`); each has a
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

## D-090 UI fixes from the pre-release review (2026-11-03)

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
  `docs/releases/X.Y.Z.md`, which the iOS release of the same version shows too. Android
  releases ship later under `android-vX.Y.Z` (D-108).


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
  have no durable path the core can read). A Folder pick walks the whole tree, including
  nested folders; documents with an empty MIME are still copied (D-121). Paste reads the
  clipboard only when the user picks Paste.
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

## D-107 Android A7 settings: library upkeep, backups, check/repair, export (2026-11-05)

The seventh PR of the Android series (D-101…D-106), mirroring iOS D-099. Stacked on
`feat/android-a6-locks` (A6 / D-106). Eagle is not a destination on the phone.

- **Settings** opens from the library screen (gear) and from Welcome. The library part:
  - **Library**: the name (renaming changes the library's own name, not its folder), where it
    lives, what it takes on disk (originals, thumbnails, database, backups) and Rebuild
    Thumbnails (the core's preview task in the background, progress in the import banner);
  - **Backups**: Back Up Now and the list (daily, by hand, before an update, before a restore)
    with Restore, confirmed. A restore is another database: views and undo start over;
  - **Check and Repair**: the core's check (database, missing originals, files and thumbnails
    without an item) and Repair with the desktop's two choices; the core backs up first;
  - **Export**: the whole library, or folders (with or without their subfolders) and tags; a
    ZIP archive (four compression levels, optionally encrypted with an age passphrase, never
    stored) or a folder tree; the preview counts items, bytes and items a closed locked folder
    leaves out. The export is written into the app's temporary folder, then saved via SAF
    (`CreateDocument`) or shared; the temporary copy goes when the screen closes or a new
    export starts.
  The app part: App Lock (moved here from Welcome), the version, the language with a link to
  the system app-language / app-details settings.
- **eglcore.** JNI and `Library` gain `rename`, `stats`, `backups`, `backUpNow`,
  `restoreBackup`, `check`, `repair`, `generatePreviews`, `exportPreview`, `startExport`,
  `exports`, `cancelExport`; models `LibraryStats`, `Backup` / `BackupKind`, `LibraryCheck`,
  `RepairOptions`, `PreviewsTicket`, `ExportSelection`, `ExportFormat` / `ExportCompression`,
  `ExportRequest`, `ExportPreview`, `ExportProgress` / `ExportSummary` / `ExportState`. Typed
  `LibraryEvent.ExportProgress`.
- **Tests.** JVM unit tests of settings / export JSON and `formatBytes`, in
  `:eglcore:testDebugUnitTest` / `:app:testDebugUnitTest` of the Mobile workflow.
- **Docs.** `android/README.md`, the README status table, and this decision.
- **Out of scope.** Release: icon, privacy, a11y, 10k, tagged APK/AAB (A8).


## D-108 Android A8 release: icon, privacy, accessibility, 10,000 items, tagged APK/AAB (2026-11-05)

The last PR of the Android series (D-101–D-107), mirroring iOS D-100. Stacked on
`feat/android-a7-settings` (A7 / D-107). Completes A1–A8.

- **Icon.** Adaptive launcher icons (`mipmap-*` + `mipmap-anydpi-v26`) drawn from the desktop
  Android set under `src-tauri/icons/android`, wired on the application; a 512×512 Play Store
  icon at `android/app/ic_launcher-playstore.png`. `scripts/check-mobile-shell.sh` checks the
  Play icon size.
- **Privacy.** In-app Privacy screen (Settings) in en / ru / zh: no tracking, libraries stay
  on the phone, App Lock / recovery keys / export passphrases never leave the device, no
  network of the app's own. `android/PRIVACY.md` for the store listing. Auto Backup and
  device-to-device transfer exclude the login, recents, saved views and library files
  (`backup_rules.xml`, `data_extraction_rules.xml`).
- **Accessibility.** Ratings read as "3 stars" (`StarsText` / `details_stars` plurals), not
  the symbols; grid and list cells say their name, rating and "Loading" until they arrive;
  folder pickers mark the chosen folder as selected. JVM tests cover the label helpers.
- **10,000 items.** A JVM unit test fills a `FakeViewSession` with 10,000 cells and times the
  paths that must not grow with the library: opening the view, a window at the end, the name
  sort, a search, a tag filter, Select All (a rule over the view, no id list). The budgets
  (3–5 s) are loose for shared CI; the timings go to the log as `perf:` lines.
- **Releases.** A tag `android-vX.Y.Z` runs the Mobile workflow (path filters do not apply to
  tags): the release build takes `X.Y.Z` as `versionName` and the run number as `versionCode`,
  and checks that the APK says so; once `library-ffi` and `android` pass, `android-release`
  publishes a GitHub Release `android-vX.Y.Z` with `EglViewer-X.Y.Z.apk`,
  `EglViewer-X.Y.Z.aab` and `SHA256SUMS`, and the changes since the previous `android-v` tag.
  It is created with `--latest=false`: the desktop updater reads `releases/latest` (D-062).
  `scripts/android-release-version.sh` turns the tag into the version and refuses anything
  else. The APK is signed with the CI debug key for sideloading; the AAB is for Play Console
  after re-signing with the upload keystore. The first Android release tag is
  `android-v0.2.3` (cut by Ivan when ready).
- **Tests.** `AccessibilityLabelsTest`, `ReleaseTest`, `PerformanceTest` in
  `:app:testDebugUnitTest` of the Mobile workflow.
- **Docs.** `android/README.md`, `android/PRIVACY.md`, the README status table, and this
  decision.

## D-109 Android JNI: callbacks on the core's threads, UTF-8 strings, one release key (2026-11-05)

A review of the Android series (D-101–D-108) found three faults that no test could see: the
JVM unit tests run fakes, and nothing ran the real `egl_jni.cpp`.

- **Callbacks.** The core reports imports, exports and refreshed views, and asks the platform
  decoder for HEIC, on threads it started. `egl_jni.cpp` looked the Kotlin classes up there
  with `FindClass`, which on an attached native thread searches the system class loader and
  cannot see the app's classes: every event and every HEIC decode was lost
  (`NoClassDefFoundError`). `JNI_OnLoad` now finds `Library.dispatchEventFromJni` and
  `PlatformDecoder.decodeFromJni` once and keeps global references; a callback that throws
  has its exception cleared before the thread returns to the core.
- **Strings.** `GetStringUTFChars` and `NewStringUTF` speak Java's modified UTF-8: a character
  outside the BMP (an emoji) became two 3-byte surrogates, the core refused the string as
  invalid UTF-8 (a library named with an emoji could not be created) and the core's 4-byte
  sequences were not safe going back. The bridge now converts from and to UTF-16 itself
  (`Utf8`, `new_string`); an unpaired surrogate or a broken sequence becomes U+FFFD. The note
  in `egl_library.h` says so (all three copies).
- **Host check.** `scripts/check-android-jni.sh` compiles `egl_jni.cpp` with the JDK's
  `jni.h`, links the core built for the host and runs `android/eglcore/src/hostTest` with
  `-Xcheck:jni`, loading the app classes through a class loader of their own, as Android does:
  a library and an item named with emoji, an emoji tag, import events from the core's
  threads, the platform decoder called for a HEIC. It failed on the code before this change
  (first the emoji name, then with ASCII names the events and the decoder) and passes now. The
  Mobile workflow runs it in the `library-ffi` job.
- **Release key.** D-108 signed the release APK with the runner's debug key, which is new on
  every runner: an update could not install over the previous release, and uninstalling first
  removes the libraries in the app's storage. Release builds now take one keystore from
  repository secrets (`ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`,
  `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`); a release tag fails without them and fails if
  `apksigner` shows a debug certificate. Without `ANDROID_KEYSTORE_FILE` a local release build
  stays unsigned. The AAB is signed with the same key, the upload key for Play App Signing.
- **Events on the main thread.** Now that events arrive, `OpenLibrary` hands a refreshed
  view to the browse models on the main thread and reads the tree again on the IO
  dispatcher, instead of doing both on the core's thread. The host check also reads the
  library from inside the callback, as `OpenLibrary` does, to show the core takes that
  without a deadlock.
- **Backups.** The backup rules left out `Libraries/` in the `file` domain, but libraries
  live in `getExternalFilesDir(null)/libraries`, the `external` domain (or
  `filesDir/libraries` without external storage): Auto Backup and device transfer carried
  whole libraries, against D-021. Both rule files now exclude `libraries/` in both domains;
  `ReleaseTest` checks them against `LibraryLocations`.
- **Names from other apps.** A provider's `DISPLAY_NAME`, a document name in a picked tree
  or a clipboard suggestion went straight into `File(folder, name)`: "../../shared_prefs/…"
  would write outside the copy's folder ("Dirty Stream"). `PickedFile.safeName` keeps the
  last segment only, never "." or "..", at most 255 bytes, and `PickedFile.child` refuses a
  path that leaves the folder.
- **Biometric unlock.** Turning it on encrypted a token with a Keystore key that needs a
  strong biometric for every use (`setUserAuthenticationRequired`, timeout 0) outside any
  prompt, so it failed and the switch showed an error. Turning it on now only makes the
  key; the unlock prompt authorises one encryption of random bytes with it, which
  succeeds only after a match, and an invalidated key (biometrics changed) turns the
  option off. This needs a device to confirm: the unit tests use the in-memory store.

## D-110 Exports: encrypted while written, cancel while encrypting, passphrase length, Decrypt Archive (2026-11-05)

A review of the archive export (D-064 to D-066) found three faults, and a gap: an `.age` file is
of little use to someone who has never used a command line.

- **No plaintext on disk.** An encrypted export wrote the whole ZIP as `<base>.zip.part` into
  the destination and encrypted it afterwards. While it ran, the originals of unlocked locked
  folders lay there in the clear, a synced destination (Dropbox, OneDrive) could upload the
  part file, and a deleted file can be recovered. The ZIP now streams into the age writer
  (`zip::ZipWriter::new_stream`, data descriptors; ZIP64 for entries of 4 GiB or more, which a
  stream must know up front): only `<base>.zip.age.part` is written, renamed to
  `<base>.zip.age` when complete, and removed on a failure or a cancel. The name is made
  unique against an existing `.zip.age`, not only `.zip`.
- **Cancel while encrypting.** Encryption was a second pass that never looked at the cancel
  flag. With the stream there is one pass, and the copy checks the flag every megabyte, so a
  cancel also stops a single large original. `ExportState::Encrypting` is no longer reported;
  the variant stays so older clients still read every state.
- **Passphrase length.** Any non-empty passphrase was taken. The core now refuses one under
  `MIN_PASSPHRASE_CHARS` = 8 characters (counted as characters, not bytes); the desktop dialog
  and the iPhone export screen say so before they start. scrypt slows guessing; it does not
  make a short passphrase strong.
- **Decrypt Archive.** `library_native::decrypt_archive` opens a passphrase age file into
  `name.zip` (or `name (2).zip`, or `name.decrypted` for a file without `.age`) in a chosen
  folder, never inside a native or an Eagle library. The plaintext is written as `.part` and
  renamed only after the whole stream authenticated: a wrong passphrase, a truncated or
  damaged file, a file encrypted for a key or not an age file at all gives a plain reason and
  leaves nothing. Desktop: **File → Decrypt Archive…** (`decrypt_archive` command with a
  progress channel over the archive bytes read, `cancel_decrypt`, one at a time; the result
  next to the archive by default, Show in Explorer allowed for it). The progress, the cancel
  and this command's shape follow a parallel attempt at the same change (PR #69, closed),
  which wrote straight over a chosen file and deleted it on a failure. iPhone: Settings → Decrypt
  Archive, from Files into a temporary folder, then Save to Files or Share. C ABI:
  `egl_decrypt_archive` (no library handle), in the three header copies. Android's export
  screen (D-107) checks the length too (`passphraseLongEnough`, code points); its Decrypt
  Archive and the JNI forwarder come later.
- Outside the app the archive still opens with `age -d -o name.zip name.zip.age` (or `rage`).
- **UI tests open All after the import.** BrowseUITests failed on this PR's CI with no change
  to browsing: the tap on All landed while the sample library was still importing and the
  list of places redrew, on a simulator that took 19 s to find the row. `openAll(expecting:)`
  (EglViewerUITests/UITestSupport.swift) waits for the row to show the full count, then
  taps, and taps once more if no browse screen came; Browse, Preview and Accessibility use it.

## D-111 Android follow-ups: catalog strings, attempts kept, Decrypt Archive, All files access (2026-11-05)

The Android review (D-109) left five items. They land together.

- **Strings.** About 30 labels of browsing (sort keys, filters, shapes, layout sizes, the
  selection title, the item count) and the action toasts were English literals. They come from
  the catalogs now, translated as on iPhone, with Android plurals for the counts (Russian with
  one / few / many / other). Toasts are worded by the host: `BrowseModel` hands
  `BrowseHost.say(ActionPhrase)` a kind and a count, so the model stays free of Android
  resources and the JVM tests read English. `StringsCatalogTest` checks that the three
  catalogs have the same keys and plurals, and fails on `Text("Word…`, `contentDescription =
  "Word…` or `show("Word…` in app code.
- **Wrong tries survive a restart.** The App Lock counter and the wait after five wrong tries
  were held in memory, so killing the app cleared them. `LoginStore` keeps `failures` and
  `wait_until` next to the hash; a right password clears both.
- **FakeViewSession** moved from the APK into the unit tests that use it.
- **Decrypt Archive.** Settings → Decrypt Archive picks an `.age` file, copies it into the
  cache, and opens it with `Library.decryptArchive` (JNI `decryptArchive` over
  `egl_decrypt_archive`, D-110) into a temporary folder; the ZIP is then saved through the
  system's Save dialog or shared. The folder is removed when the screen closes. The host JNI
  check encrypts an export with a non-ASCII passphrase and opens it with a wrong and a right one.
- **Libraries outside app storage.** The user chose broad access over copying a library in:
  the app declares `MANAGE_EXTERNAL_STORAGE` (and `READ` / `WRITE_EXTERNAL_STORAGE` up to API
  29). Opening a library whose real path lies outside app storage without that access shows
  what it is for and opens the system page; on return the open is retried. Without it the app
  still works with libraries in its own storage. A library is never copied or moved: it stays
  the one source of truth where it is. Google Play reviews this permission; a store build may
  need the declaration form, or a build without it.

## D-112 ZIP with a password: AES-256 entries next to the age file (2026-11-05)

An age file (D-066, D-110) is the strongest choice, but a person who gets one needs this app,
`age` or `rage` to open it. A ZIP export with a passphrase now offers a second protection, chosen
in the dialog: **ZIP password**, a standard `name.zip` whose entries are AES-256 encrypted
(WinZip AES, written by `zip` with its `aes-crypto` feature).

- Opens in 7-Zip, WinRAR, PeaZip, Keka and The Unarchiver, checked here against `7z t` with a
  non-ASCII passphrase. The archive tools built into Windows and macOS do not open AES entries;
  the dialog says so.
- Weaker than age, and the dialog says that too: the names and the sizes of the files stay
  readable, and the key derivation is PBKDF2-SHA1 with 1,000 rounds, so a short passphrase falls
  quickly. The 8-character floor (D-110) applies; age stays the default.
- Written like an unencrypted ZIP: `name.zip.part`, renamed when complete, removed on a failure
  or a cancel. Entries are encrypted as they are written, so no plaintext copy touches the disk.
- Wire format: `{"kind":"zip","compression":…,"encryption":"age"|"aes"}`; a request without
  `encryption` (older phones) means age. Swift `.zip(_, encryption:)` and Kotlin
  `ExportFormat.Zip(compression, encryption)` default to age.
- Dependencies: the `aes-crypto` feature of `zip` 9.2.0 adds `pbkdf2` 0.13.0, `sha1` 0.11.0 and
  `constant_time_eq` 0.6.1 (plus `aes`, `hmac` and `zeroize`, already in the tree), the newest
  versions older than the minimum release age (D-003) that `zip` accepts.

## D-113 7z export: AES-256 with hidden names (2026-11-05)

The export formats gain **7z**, written by `sevenz-rust2` 0.24.2 (pure Rust; the newest release
older than the minimum release age, D-003), with `default-features = false` and only `aes256` and
`compress`. It adds `lzma-rust2` 0.23.0 (the newest its `^0.23` allows); `cbc`, `sha2` 0.11 and
`crc32fast` are already in the tree.

- **With a passphrase** the entries and the header that lists the names are AES-256 encrypted:
  without the passphrase nobody sees what the archive holds, as with age, and it still opens in
  7-Zip, WinRAR, PeaZip and Keka. The key derivation is set to 7-Zip's own 2^19 SHA-256 rounds;
  the crate's default (2^8) would make guessing as cheap as for a ZIP.
- **Compression** maps to LZMA2 presets 1 / 5 / 7, or stored entries for None. "Smallest" is
  preset 7, not 9: preset 9 needs about 670 MiB to compress, too much for a phone, and photos and
  videos barely shrink anyway.
- Each original is its own stream (not solid), so a cancel stops between or inside files and a
  damaged spot costs one file. The archive is written as `name.7z.part` and renamed when complete;
  a failure or a cancel removes it. A reader wrapper stops on the cancel flag and keeps the
  original's read error apart from a write error.
- Writing an archive goes through one `ArchiveSink` trait (ZIP or 7z): the loop over the
  originals, the size check and the manifest are shared.
- Decrypt Archive stays age-only: a 7z or a ZIP with a password opens in the tools above.
- Desktop: a fourth format button and the ZIP protection choice with their hints. iPhone and
  Android: ZIP / 7z / Folder, and age or ZIP password under Encrypt. Tests: export round trips
  read back with `zip` and `sevenz-rust2` (wrong and right passwords, no names in the bytes of
  an encrypted 7z, cancel, failure cleanup), the C ABI with both formats, the JSON shapes in Swift
  and Kotlin, and the desktop dialog's request.

## D-114 Version 0.2.5 (2026-11-06)

- **Version 0.2.5 everywhere:** the workspace crates (and `Cargo.lock`), the desktop
  (`tauri.conf.json`, `ui/package.json`), Android (`versionName`, the version the shell gives
  the core) and iOS (`MARKETING_VERSION`). Release tags: `v0.2.5`, `ios-v0.2.5`,
  `android-v0.2.5` (first Android release).

## D-115 Android: the native core loads on the phone (2026-11-06)

The first Android release (0.2.5, D-114) opened no library on a phone: "native core is not
linked". `System.loadLibrary("egl_jni")` failed because `libegl_jni.so` needed
`libegl_library.so` by the CI machine's absolute path
(`/home/runner/work/…/jniLibs/arm64-v8a/libegl_library.so`). The Rust cdylib had no SONAME,
and when CMake links an imported library by its full path and that library has no SONAME, the
linker records the path itself as DT_NEEDED. The host JNI check (D-109) did not notice: it
linked with `-L -l`, and on the host the path exists anyway.

- **SONAME.** `crates/library-ffi/build.rs` links the cdylib with `-soname,libegl_library.so`
  on Android and Linux, so anything linked against it needs it by name.
- **16 KB pages.** Android 15+ devices with 16 KB pages refuse 4 KB-aligned libraries;
  `libegl_jni.so` (NDK 27) was 4 KB-aligned. Both libraries now link with
  `-z max-page-size=16384`.
- **No stray `.so`.** cargo-ndk copies every cdylib it built; `sevenz-rust2` (D-113) declares
  one, and `libsevenz_rust2-<hash>.so` rode along in the APK. CI deletes everything but
  `libegl_library.so` after cargo-ndk.
- **Checked where it broke.** `scripts/check-android-apk.sh` reads the native libraries of a
  built APK as the loader does: DT_NEEDED by name only, the SONAME, 16 KB LOAD alignment, no
  `lib<crate>-<hash>.so`. CI runs it on the debug APK and on the release APK before upload; run
  against the 0.2.5 APK it reports all three faults. The host JNI check now links the bridge by
  the core's full path, as CMake does, and fails without a SONAME (checked by removing
  `build.rs`).
- **A reason, not a build hint.** If the bridge still does not load, `EglNative` keeps the
  `UnsatisfiedLinkError` message; Welcome and the open error show it under a plain sentence in
  the three languages, instead of "build libegl_library.so with cargo-ndk", which is for
  developers.
- 0.2.5's APK cannot be fixed in place: the next Android release carries this.

## D-116 Eagle import on the phones (2026-11-06)

The phones had no Eagle at all (D-099). Asked for, they now import an Eagle library folder
(`Name.library`) into a new native library, as the desktop's wizard does (docs/spec.md §8.2).
Opening an Eagle library in place, read-only, was the other choice; it needs a second source
of views in the core and was not taken. A live connection and export into Eagle stay
desktop-only.

- **One import, three apps.** The import task moved from `src-tauri` into `app-service`
  (`LibraryService::import_eagle`, `eagle_library_info`, `EagleLibrarySummary`, `eagle_error`).
  The desktop calls it and now checks the Eagle folder before it creates the target library,
  so a wrong folder leaves no empty library behind. The ts-rs binding is unchanged.
- **C ABI.** `egl_eagle_library_info(path, out)` needs no library handle and answers
  `EGL_ERR_NOT_A_LIBRARY` with a plain reason for a folder that is not one;
  `egl_import_eagle(lib, {"path","importTrash","underRootFolder"}, out)` starts the task on the
  open library and reports through `importProgress` like adding files, the last progress with
  the report. In the three header copies; Swift `Library.eagleLibraryInfo` / `importEagle`,
  Kotlin the same over JNI, with the report and the summary as models on both.
- **Flow.** Welcome → Import Eagle Library → pick the folder (iPhone: the Files picker, the
  security scope held until the import ends; Android: the tree picker, All files access asked
  as for a library in shared storage, D-111) → name, item count and size → the new library's
  name (the Eagle name; `Name 2` when taken) and Eagle's trash on or off → the library is
  created in the app's storage, opened, and the copy runs in the background with progress and
  Stop → a report: imported, same content as another item, could not be imported, left in the
  trash, locked folders that came in without a password. A stopped import resumes by importing
  the same Eagle library into that library again: items it already has are skipped.
- **Tests.** The C ABI with the Eagle fixture (summary, a plain folder refused, the import, the
  folders at the root, a missing folder refused); the host JNI check with an Eagle library named
  in Cyrillic and emoji; EglCore against the fixture; an iPhone app test from the pick to the
  report and the free name; Kotlin parsing of the report and the summary; the free name on
  Android.

## D-117 The library folder shows the app's icon on Windows and Linux (2026-11-06)

A `.eglib` stays a plain folder (D-016); the question of a single-file library was settled
against it (crash-safe adds, originals readable without the app, incremental backups). Asked
for only a recognisable icon, the desktop now gives the folder the app's icon where the file
manager reads one from the folder itself:

- **Windows:** a hidden `.egl-icon.ico` (the app's `icon.ico`) and a hidden, system
  `desktop.ini` pointing at it by a relative path, so a moved or copied library keeps the icon;
  the folder gets the system attribute, which is what makes Explorer read `desktop.ini` (the
  read-only attribute would do it too, but gets in the way of deleting the folder). The
  attributes are set with `attrib`, since the crate forbids the unsafe Win32 call.
- **Linux:** `.directory` with `Icon=./.egl-icon.png` (the 128 px app icon), which KDE's Dolphin
  reads. GNOME's Nautilus keeps custom folder icons in its own metadata, not in the folder, so it
  shows a plain folder.
- **Not on macOS, iOS or Android.** On macOS and iOS a folder takes an icon only when its type
  is registered as a package, which would also show the library as one item; that was declined.
  Android file managers take no icon from a folder.
- **When.** On every open of a library this instance writes, so a library made before gets the
  icon too. The files are written only when missing and never replace one that is there (a
  user's own `.directory` stays theirs); a read-only library (another instance, a sync folder)
  is left untouched. A failure is logged and the library opens anyway.
- They are cosmetic files of the format (docs/library-format.md §1): nothing reads them, and
  Check Library walks only `files/` and `thumbs/`.

## D-118 Version 0.2.55 (2026-11-07)

- **Version 0.2.55 everywhere:** the workspace crates (and `Cargo.lock`), the desktop
  (`tauri.conf.json`, `ui/package.json`), Android (`versionName`, the version the shell gives
  the core) and iOS (`MARKETING_VERSION`). Release tags: `v0.2.55`, `ios-v0.2.55`,
  `android-v0.2.55`.

## D-119 Android dark theme: DayNight, tokens, System / Light / Dark (2026-11-07)

Android had only a forced light XML theme and hardcoded light `EglTokens`. Dark mode is now
first-class, aligned with the desktop (D-028):

- **XML.** `Theme.EglViewer` parents `Theme.AppCompat.DayNight.NoActionBar`;
  `values-night` sets the window background to the dark token so the splash is not white.
- **Tokens.** Light and dark `EglColors` match `ui/src/index.css` `:root` / `.dark`. Compose
  reads them through `LocalEglColors` / `EglTokens`; the app root wraps `MaterialTheme` with a
  matching ColorScheme. The preview stage stays always dark (as on desktop and iOS).
- **Preference.** Settings → Theme offers System / Light / Dark (wire values `system` /
  `light` / `dark`). Stored in SharedPreferences; applied with
  `AppCompatDelegate.setDefaultNightMode` from `EglApplication` so the first frame follows the
  choice. Status colours (destructive / success / warning) are named tokens with dark pairs.
- **Out of scope.** Desktop Tauri and iOS theme UI are unchanged (iOS already follows system
  colours). No shared mobile preference file with the desktop's `preferences.json`.

## D-121 Android folder pick: recursive SAF walk, no MIME skip (2026-11-07)

A4's Folder action (`OpenDocumentTree`, D-104) copied a picked tree into `picked-*` and handed
the folder to `egl_add_files`. Nested folders were walked, but each child was taken only when
`DocumentFile.isFile` was true. That helper returns false when the provider leaves the MIME
type empty, so photos (JPEG, PNG, HEIC, WebP, …) on some trees never reached the copy and the
library looked incomplete. Trash places still omit Add; a folder place still targets that
folder.

- **Take every non-directory.** The tree copy recurses as before; a child that is not a
  directory is copied even when its MIME is null or blank. Directories alone decide nesting.
  A display name with no extension may gain one from a known image/video MIME (HEIC/WebP
  included), matching the formats the core previews (D-015 / D-104).
- **Filesystem when readable.** If the tree URI resolves to a primary-storage path and the app
  may read it (app storage or All files access, D-111), that path is passed straight to
  `egl_add_files` so the core walks it; otherwise the SAF copy runs while the persistable read
  grant is held and is released when the import ends. Temp `picked-*` copies are removed
  afterwards (and leftovers at launch), as in D-104.
- **Tests.** Host JVM coverage of the recursive listing (nested folders, empty MIME kept,
  blank / "." names skipped) and of the MIME→extension map, in `:app:testDebugUnitTest`.
- **Docs.** `android/README.md` Adding row; this decision.

## D-122 iOS browse grid: borderless justified packing (2026-11-07)

The iOS «All» grid used fixed square cells with a grey rounded “frame” and letterboxing
around each thumb (`scaledToFit` inside `EglTokens.grid`). Tall screenshots and landscapes
wasted space; the desktop already ships Flickr / Google Photos–style justified rows (D-067).

- **Borderless.** Grid thumbs sit flush on the background: no grey tile fill, no inset
  padding. Cells clip to a light corner radius; selection is a stroke only. Thumbs
  `scaledToFill` the aspect box (crop), matching desktop `object-cover`. List rows keep a
  small muted placeholder only while the window has not arrived. Filenames stay under the
  thumb in a caption line (less chrome than a framed card).
- **Justified default.** `BrowseLayout.grid` packs justified rows from
  `egl_view_dimensions` (already wrapped as `Library.dimensions`): one height per row, cell
  widths from aspect, rows fill the width, gap/padding 4 pt, size control = target row
  height (small / medium / large). Missing sizes are square. Pure packing lives in
  `JustifiedPacking` with XCTest coverage of the same cases as `justified.test.ts`.
- **Square and list kept.** `BrowseLayout.square` is the previous uniform adaptive grid
  (also borderless). List mode is unchanged. Select, search, filters and chrome are
  untouched.
- **Android.** Left alone; its system-UI grid polish is separate. Shared FFI dimensions
  already existed.
- **Docs.** `ios/README.md` browsing row; this decision.

## D-123 iOS preview: SwiftUI zoom, no nested scroll (2026-11-07)

Full-screen preview opened on a black stage (chrome correct: close, title, 1 of N, actions)
and paging felt broken. Present on `main` before D-122; not introduced by the borderless
justified grid work (D-122).

- **Cause.** `ZoomingImage` was a `UIScrollView` `UIViewRepresentable` that sized its image
  view only from `updateUIView`. On open the scroll view's bounds were often still zero, so
  the image stayed at a zero frame over the dark stage. The nested UIScrollView also competed
  with the SwiftUI pager for horizontal swipes even when `isScrollEnabled` was false at fit
  size; a shared `zoomed` flag could leave paging disabled after a page change.
- **Fix.** Zoom is pure SwiftUI (`MagnifyGesture` + double-tap, pan only while zoomed via
  `GestureMask`), filling the page frame from a `GeometryReader`. The pager resets `zoomed`
  when the visible index changes; the page's decode task ignores a transient `pixels == 0`
  and includes the pixel size in its id; `OriginalCache` clamps the ImageIO max-pixel size to
  at least 1. The zoom image (`preview.zoom`) takes the item name as its accessibility label
  so VoiceOver / the a11y audit (D-100) see a description.
- **Tests.** `OriginalCacheTests` decodes a sample original and checks the memory cache;
  existing `PreviewUITests` still cover swipe / rate / trash;
  `AccessibilityUITests` covers the preview description.
- **Docs.** This decision; preview row in `ios/README.md`.

## D-124 Version 0.2.77 (2026-11-07)

- **Version 0.2.77 everywhere:** the workspace crates (and `Cargo.lock`), the desktop
  (`tauri.conf.json`, `ui/package.json`), Android (`versionName`, the version the shell gives
  the core) and iOS (`MARKETING_VERSION`). Release tags: `v0.2.77`, `ios-v0.2.77`,
  `android-v0.2.77`.
- **0.2.77 is a higher SemVer than 0.2.55** (patch 77 against 55). The desktop updater
  compares versions as SemVer and is left unchanged. The iOS and Android note jobs still
  take the previous tag with `git tag --sort=-v:refname`; for these tags that is `0.2.55`.
  Android `versionCode` and the iOS build number stay the GitHub Actions run number.

## D-126 Experimental GPUI Kit desktop shell (2026-11-08)

Spike of a second desktop client next to Tauri/`ui/`, not a replacement. Same idea as the
mobile shells (D-082 / D-083): prove the stack by calling Rust services directly.

Numbering note: open PR #85 already reserves **D-125** for mobile GitHub Release visibility,
so this decision is **D-126** (not D-125).

- **Crate.** `crates/gpui-desktop` (bin `gpui-desktop`) depends on `gpui-kit` **=0.9.0** (=0.9.1 since D-155)
  (Longbridge, https://gpui-kit.com / crates.io `gpui-kit`), and (after D-127)
  `desktop-session` / `library-core` / `library-native`. Workspace pin is exact (D-002);
  0.9.0 was published 2026-11-02, older than the supply-chain age rule in D-003. The app never
  lists Zed's GPUI crates itself — `gpui-kit` re-exports them.
- **Not gpui-ce.** This spike uses Longbridge's kit (`gpui-kit` / `gpui-component` /
  `gpui-base` over `gpui-pre`), not Community Edition packaging.
- **What it does.** Opens a native window, opens a native `.eglib` (path field, argv, or a
  temp copy of `fixtures/library/v5/Fixture.eglib`), loads an All-items view through
  `Session` (`open_view` + `view_range`), and shows a simple thumb / palette grid.
  OS file drop is probed via GPUI `ExternalPaths` / `on_drop` (status + optional `img`
  preview; no library import yet). No WebView, no Tauri IPC, no Eagle.
- **Out of scope for the spike.** Menu bar / commands (D-025), justified packing (D-067),
  edits, import, settings, Eagle, packaging, and replacing `src-tauri` / `ui/`. Those stay
  the product path.
- **Run.** `cargo run -p gpui-desktop` (same `VCPKG_ROOT` / `VCPKGRS_DYNAMIC` / PATH needs as
  the rest of the workspace for libheif). Tests: `cargo test -p gpui-desktop`.
- **Revisit** when deciding whether a native GPUI shell becomes a real desktop track or stays
  an experiment beside Tauri.

## D-127 Toolkit-free `desktop-session` crate (2026-11-08)

Extract the desktop `Session` out of `src-tauri` so Tauri and the GPUI experiment share one
API (GPUI migration).

Numbering: **D-125** is reserved by open PR #85 (mobile release visibility); **D-126** is the
GPUI Kit spike. This extract is **D-127**. Open Android work that may claim nearby numbers
must skip 125–127.

- **Crate.** `crates/desktop-session`: `Session`, Eagle session / import request types, the
  `Clipboard` trait, export-into-Eagle, folder-icon bytes (D-117), and the existing session
  tests. No `gpui`, no `tauri`. Depends on `app-service`, `app-store`, `eagle-api`,
  `eagle-folder`, `library-core`, `library-native`, `media`, `yubikey`, and (on Windows) the
  WebAuthn bridge.
- **Hosts.** `src-tauri` is a thin command / window / `thumb` adapter over `Session`.
  `gpui-desktop` opens libraries through the same `Session` (no direct `LibraryService`
  glue for open/list/thumbs).
- **YubiKey / WebAuthn (GPUI migration note, not a blocker).** They live **in** `desktop-session`
  for now (`yubikey` module + Windows `webauthn` / unsupported stub): the Session methods
  already call them, and neither needs a Tauri or GPUI window handle today (WebAuthn uses
  `GetForegroundWindow`). Revisit moving them under a future `egl-desktop::platform` (or a
  host-injected `Authenticator` / PIV trait) if a GPUI host needs its own window HWND or a
  different prompt UX; until then keep one implementation next to Session.
- **Behaviour.** Mechanical move; no intentional Session API or library behaviour change.
- **Verify.** `cargo test -p desktop-session`, `cargo test -p egl-viewer`,
  `cargo test -p gpui-desktop` (with the usual `VCPKG_ROOT` / heif PATH).

## D-129 GPUI browse shell (2026-11-08)

First usable native browse UI on `gpui-desktop`, still experimental beside Tauri/`ui/`.

Numbering: **D-125** reserved by open PR #85; **D-126** spike; **D-127** session extract;
**D-128** claimed by Android preview-swipe work. This slice is **D-129**.

- **Ships.** Welcome (create / open / recent / forget); startup reopen + `--no-reopen` +
  Shift-at-start via `device_query` (closes the D-032 Windows-key gap for the GPUI host);
  library chrome (menubar strip, left scopes + read-only folder tree, toolbar
  grid/list/thumb size/names/subfolders/Sort By, status, read-only banner, details stub);
  live justified grid + list with Kit `v_virtual_list`, Session thumbs / palette placeholders;
  click / Ctrl / Shift / Select All / rubber-band / arrow keys / Enter→details; cell
  `aria_label`s; Kit `h_resizable` panel drag with width persistence; real F11 via
  `Window::toggle_fullscreen`; About with `EGL_COMMIT`; Session `Emit` mailbox → Entity poll
  for `views-invalidated`; command actions + keymap; Settings → General; Shortcuts; quit
  confirm; Open Log Folder; GPUI `prompt_for_paths`; per-library session restore.
- **i18n.** en / ru / zh-CN catalogs for Welcome and shell chrome inside the crate (mechanical
  port of the React keys); locale preference persists in the GPUI AppStore.
- **AppStore.** Stable `%LOCALAPPDATA%\egl-viewer-gpui` (not the Tauri app id) so prefs survive
  restarts without colliding with the shipping desktop until cutover.
- **Deps.** `device_query = "=4.0.1"` (safe Shift query; crate contains the Win32/FFI `unsafe`,
  not this package — D-002 / D-003 / D-032).
- **Not yet / deferred.** Search field remains a stub label (until the search slice); Eagle Welcome actions
  stay disabled (until the Eagle slice); windowed `view_range` fetch beyond the first 500 cells can tighten
  further for huge libraries; Tauri host still lacks Shift-at-start (unchanged D-032 for
  shipping). Rubber-band hit-testing uses content coordinates best-effort.
- **Run.** `cargo run -p gpui-desktop` (optional path / `--no-reopen`). Tests:
  `cargo test -p gpui-desktop`. Tauri/`ui/` unchanged.

## D-130 GPUI preview + details read path (2026-11-08)

Full-size preview and read-only details on `gpui-desktop`, still experimental beside Tauri/`ui/`.

Numbering: **D-125** reserved by open PR #85; **D-126** spike; **D-127** session extract;
**D-128** Android preview swipe; **D-129** browse shell. This slice is **D-130**.

- **Ships.** Right-bar details via `Session::item_detail` (one / many / none / trash placeholder):
  thumb, palette, name, rating, tags, folders, note, link, info, EXIF (comments RO when present);
  multi-select stack + size + tags; Open / Reveal / Copy / Open link through platform helpers over
  `original_paths`. Full-size preview overlay: fit / zoom / pan / rotate `R` (session-only, D-058),
  ←/→ in the view, Esc / letterbox click close, Space hold-vs-toggle peek (D-073) via
  `device_query` key-up poll; double-click opens preview. Image path: originals for
  JPEG/PNG/WebP/GIF/BMP/SVG; HEIC via `media::render_bytes` (webp); TIFF/PSD → 512 thumb.
  Best-effort appear fade; skipped when `EGL_REDUCED_MOTION` / Windows `MinAnimate=0`.
- **Deps.** `arboard`, `image`, `media`, `serde_json`, Windows `winreg` (reduced-motion) on
  `gpui-desktop` only (D-002 / D-003).
- **Not yet (superseded by D-131 for edits).** Morph-from-cell View Transitions (plan §7
  best-effort); Eagle.
- **Run.** `cargo run -p gpui-desktop`. Tests: `cargo test -p gpui-desktop`. Tauri/`ui/` unchanged.

## D-131 GPUI edits / organize / import / locks (2026-11-08)

Day-to-day native-library editing on `gpui-desktop` via `desktop-session` (still experimental
beside Tauri/`ui/`). Numbering: free id after D-130; **D-131**. Follow-up closed the DnD /
filters / locks / import-preview gaps on the same day.

- **Ships.** Details edits (name/note/link/rating/tags; dimmed partial tags); Trash / Restore /
  Delete permanently; Empty Trash on a background thread with progress + Cancel; Undo
  `Ctrl+Z`. Search `Ctrl+F`; filter dialogs (tag mode, folders, type, shape) + rating chips +
  Clear. Folder CRUD; folder properties (colour / description / sort incl. Manual); Manual
  cell reorder. Sidebar vocabulary + tag drop; tag groups CRUD; rename/merge/delete tags.
  Smart folders list/open + Save as smart from query. Locks: unlock / lock / Lock All; manage
  (change password, recover, renew, remove, label, save recovery file); YubiKey
  bind/unbind/unlock when present. Import: Add Files; `preview_import` dialog (images/all);
  OS drop; Paste image; Imports panel + duplicates. In-window DnD via GPUI `on_drag` /
  `on_drop` / `can_drop` / `on_drag_move` (items→folder/tag/trash; folder before/inside/
  after; Manual cells). Library Settings (rename, reveal, stats, backups, check, rebuild).
- **Still open.** Smart rule editor UI (3.C.4); OS drag-out (3.F.2); recent-tags strip; full
  import tree checkboxes; OS clipboard file-list paste; folder multi-membership checklist.
- **Deps.** No new crates (reuses `arboard` from D-130).
- **Run.** `cargo run -p gpui-desktop`. Tests: `cargo test -p gpui-desktop` (23). Tauri/`ui/`
  unchanged.

## D-132 GPUI Eagle / export archives / app login (2026-11-08)

Secondary flows on `gpui-desktop` via `desktop-session` (still experimental beside Tauri/`ui/`).
Numbering: free id after D-131; **D-132**.

- **Ships.** Unified Backup/Export overlay (ZIP / 7z / mirror / into Eagle; age or AES ZIP;
  passphrase rules; destination memory; progress + cancel + reveal; refuse library dest).
  Decrypt Archive (`.zip.age`) on a background thread with cancel. Library → Backup…;
  folder/tag Export… from toolbar. Eagle import wizard (pick `.library` → create native →
  `import_eagle_library`). Connect overlay (host/port/token; Credential Manager via Session;
  clear token after send D-080); live browse adopts `EagleApi` library; edits/thumbs reuse
  Session eagle routing. App login gate before Welcome (password / PIN / WebAuthn); Settings
  login enroll (set password/PIN/WebAuthn / clear). ExportProgress Emit → Entity poll.
- **Still open / partial.** Free multi-folder/tag picker in Backup (whole-library only for
  free scope); compression picker UI thin (defaults Balanced); Eagle import resume/report
  polish; dedicated connection-lost Emit (none in Session today — same as Tauri surface);
  WebAuthn needs real YubiKey (CI uses software path in `app-store` tests only).
- **Deps.** No new crates.
- **Run.** `cargo run -p gpui-desktop`. Tests: `cargo test -p gpui-desktop` (30). Tauri/`ui/`
  unchanged.

## D-133 GPUI desktop cutover path (dual path) (2026-11-08)

Packaging, updater channel, CI, and docs for the GPUI host **beside** the shipping Tauri
desktop. Numbering: free id after D-132; **D-133**. **Do not delete** `src-tauri` / `ui/` until
Ivan confirms retirement.

- **Product stance.** Intended Windows desktop UI toolkit = **GPUI Kit** (`crates/gpui-desktop`).
  Tauri 2 + React `ui/` remain the **shipping** Windows release and `pnpm dev` / `pnpm build`
  path until cutover is confirmed. Dual docs and CI first; quarantine is documentation +
  release-path clarity, not deletion.
- **Packaging (ships).** `scripts/package-gpui-desktop.ps1` builds release `gpui-desktop.exe`,
  stages `heif.dll` / `libde265.dll` (+ LGPL notices) beside the exe (same D-070 rule as Tauri),
  copies About licenses + icon, and writes `dist/egl-viewer-gpui_<version>_x64.zip`. Per-user
  NSIS setup is D-153 (`scripts/package-gpui-nsis.ps1`).
- **Packaging (deferred).** `.eglib` file association, Authenticode / SmartScreen signing for the
  GPUI artifact. Clean-machine install done check stays open.
- **Updater (ships, partial → D-153).** Help → Check for Updates… and a silent post-gate check
  fetch the rolling `gpui-latest` manifest (not GitHub Latest / Tauri `latest.json`). Skip with
  `EGL_SKIP_UPDATE_CHECK`. Manifest writer: `scripts/write-latest-gpui-json.mjs`.
- **Updater (deferred).** Minisign verification and Authenticode for the Windows installer.
- **Licenses.** About → Third-party licenses shows
  `crates/gpui-desktop/assets/THIRD_PARTY_LICENSES.txt` (summary; fuller cargo inventory later).
- **CI.** `.github/workflows/ci.yml` job `gpui-desktop`: `cargo test -p gpui-desktop`, package
  script, upload `gpui-desktop-portable` artifact. Mobile path-filters (D-084) untouched. Tauri
  `check` / `nsis` / `dmg` / `appimage` / `release` jobs unchanged.
- **Chrome.** Native GPUI window frame is enough for the GPUI host; custom title-bar merge
  (D-025) is not required on this path (still open on the Tauri side).
- **Deps.** `reqwest` with `blocking` on `gpui-desktop` only (workspace pin 0.12.28, D-002 /
  D-003). No new crate versions beyond existing workspace pins.
- **Verify.** `cargo test -p gpui-desktop`; `pwsh scripts/package-gpui-desktop.ps1` on Windows
  with vcpkg libheif. Tauri/`ui/` behaviour unchanged.

## D-134 GPUI Show in Explorer uses shell select, not cwd (2026-11-08)

Show in Explorer from the GPUI details pane opened `target\debug` (the process cwd under
`cargo run`) instead of the item original inside the open `.eglib`.

- **Cause.** `platform::reveal_paths` spawned `explorer.exe` with a single
  `/select,…` `CreateProcess` argument. When that path is relative, missing, or otherwise
  rejected by Explorer, Windows opens the **process current directory** — not the media
  file. Session `original_paths` already return absolute paths under the library root;
  the host must not fall back to cwd.
- **Fix.** Require absolute, existing paths; refuse relatives with an error. On Windows call
  `SHOpenFolderAndSelectItems` / `ILCreateFromPathW` (same approach as `tauri-plugin-opener`,
  in `reveal_win.rs` with `#![allow(unsafe_code)]` like the WebAuthn bridge). macOS / Linux
  keep `open -R` / folder open on the resolved absolute path.
- **Deps.** `windows` **0.62.2** on `gpui-desktop` (Windows only), same pin as
  `desktop-session`, with Shell / Com features for reveal (D-002).
- **Heif launch PATH.** Local vcpkg `heif.dll` may still import GPL `libx265.dll`.
  `scripts/run-gpui-desktop.ps1` puts `installed\x64-windows\bin` on `PATH` before
  `cargo run`. `scripts/package-gpui-desktop.ps1` still prefers decode-only
  `bundle-heif-dlls.ps1` (D-070); if that refuses x265, it copies every vcpkg-bin DLL
  `heif.dll` imports (dumpbin walk) so the portable layout launches, with a GPL warning.
  CI / NSIS stay decode-only.
- **Verify.** `cargo test -p gpui-desktop`; Show in Explorer on an RSPECT item opens the
  file under `D:\RSPECT.eglib\files\…`, not `target\debug`.

## D-135 Auto-clean workspace `target/` over 20 GiB (2026-11-08)

GPUI Kit and the full workspace leave large `target/` trees on Windows; reclaim disk when the
folder exceeds **20 GiB**. Free id after D-134.

- **Script.** `scripts/clean-target-if-huge.ps1` measures only the workspace `target/` under the
  repo root, reports size vs threshold, and if over 20 GiB runs `cargo clean` (or `Remove-Item`
  on `target/` when cargo is missing). `-WhatIf` / `-Force` / `-ThresholdGiB` for dry-run,
  unconditional clean, and a different limit.
- **Safety.** Never deletes `.eglib` libraries, `vcpkg`, `dist/`, `node_modules/`, or any path
  outside repo `target/`. Refuses if `target` resolves outside the repo root.
- **Wiring.** Documented in README Commands. Optional flag on packaging:
  `pwsh scripts/package-gpui-desktop.ps1 -CleanHugeTarget`. Not hooked into `pnpm check` (CI
  runners should not wipe build caches mid-job).
- **Run.** From the repo root: `pwsh scripts/clean-target-if-huge.ps1`.

## D-136 GPUI tests must not pollute recent libraries (2026-11-08)

`gpui-desktop` tests called `library::session()`, which shared the real AppStore under
`%LOCALAPPDATA%\egl-viewer-gpui`. Opening temp Spike/Fixture libraries wrote them into
`recent.json`; after the temp dirs vanished, Welcome showed many "(missing)" rows.

- **Tests.** `session()` uses a disposable store under the process temp dir. Production still
  uses `session_with_emit` / `session_with_mailbox` → `platform::app_data_dir()`.
- **Welcome.** `AppStore::prune_missing_recent` forgets rows whose `library.db` is gone;
  Session exposes it; GPUI calls it on startup and when rendering Welcome.
- **Verify.** `cargo test -p app-store prune_missing`; `cargo test -p gpui-desktop`
  `test_session_does_not_pollute_persistent_recent`.

## D-137 GPUI Library menu and Eagle-style Settings (2026-11-08)

The GPUI host showed library commands as a row of buttons. Eagle's Library menu is
Create…, Open…, Switch… (`Ctrl+L`) with the open library checked and its path under the
name, Clear History, Clear Cache and Reload, and Merge….

- **Menu bar.** File, Edit, View, Library and Help are dropdowns. Library owns create,
  open and switch. File keeps add, import, Eagle, close, Settings and Exit.
- **Switch.** The submenu lists recent libraries (name, path, a check on the open one).
  `Ctrl+L` opens the same list. Lock All on this host is `Ctrl+Shift+L`. The Tauri
  command registry still binds `Ctrl+L` to Lock All until cutover.
- **Clear History.** `AppStore::clear_recent_history` drops every recent row except the
  open library. A removed path also loses its saved place, as Forget does.
- **Clear Cache and Reload.** A writable native library calls `rebuild_thumbnails` (the
  `thumbs/` cache; the old file stays until the new preview replaces it) and reloads the
  view. A read-only library reloads and says thumbnails were not rebuilt. A live Eagle
  library has no local thumbnail cache here; the view reloads and the status says Eagle
  serves its own thumbnails.
- **Merge.** There is no merge-libraries command (merging tags is a different action).
  The item is left out.
- **Settings.** A window with a search field and General, Library and Login on the left,
  the section on the right, Apply (write, stay open), Save (write and close) and Cancel
  (drop the draft). General: theme swatches, language, thumbnail size (the grid zoom that
  is stored; there is no interface-zoom percent), reopen last library, ask before trash.
  Library: default folder for new libraries. Login: password, PIN, WebAuthn and clear,
  which take effect immediately rather than with the draft. No AI or plugin sections.
- **Verify.** `cargo test -p app-store clear_recent`; `cargo test -p app-store same_library`;
  `cargo test -p gpui-desktop`.

## D-138 GPUI minimum window is 960×600 (2026-11-08)

The GPUI host opened at 1100×720 with no minimum, so the frame could be dragged down to a
square or smaller and the side bars no longer fit. Tauri stays 900×600 (`tauri.conf.json`).

- **Open.** `WindowOptions::window_min_size` is 960×600 (gpui-kit / GPUI `window_min_size`).
  The requested size is clamped up on each axis that is below that minimum; 1100×720 is
  already larger and stays. Width and height are different numbers.
- **Verify.** `cargo test -p gpui-desktop min_window`; `cargo test -p gpui-desktop clamp`.

## D-139 Show in Explorer must not re-enter the GPUI message loop (2026-11-08)

Show in Explorer opened an Explorer window and then `gpui-desktop` exited.

- **Cause.** Not a window-close and not `WM_QUIT`. The details button calls
  `SHOpenFolderAndSelectItems` on the UI thread while GPUI is inside `Window::dispatch_event`
  (`gpui` `window.rs` around the input callback, mutable `App` borrow). The shell API runs a
  nested Win32 loop (`SHOpenOrGetFolderView` → `CallWindowProcW` on the GPUI window). Paint,
  activation and the 32 ms session poll then borrow `App` again. The poll's
  `AsyncApp::update_entity` unwraps that borrow and panics (`async_context.rs`). Explorer is
  already visible. `CreateProcess` (`explorer.exe`, `cmd /C start`) can pump the same queue
  (GPUI's own Windows note on `Command::spawn`).
- **Fix.** Group and validate paths on the caller so a bad path still reaches the status bar.
  `SHOpenFolderAndSelectItems` / `ShellExecuteEx` run on a thread named `explorer-reveal`.
  File-manager, open-with, and URL `Command`s run on `shell-open`. The click returns and
  drops the `App` borrow before the shell call. A sent activation is handled afterwards.
  Unit tests skip the real shell unless `EGL_REVEAL_SMOKE=1`.
- **Verify.** `cargo test -p gpui-desktop reveal_shell_is_not`; `cargo test -p gpui-desktop detached_shell`.
  Show in Explorer on an item: Explorer selects the file and gpui-desktop stays open.

## D-140 GPUI right-click menus, Eagle item shortcuts and the folder panel (2026-11-10)

The GPUI host had no right-click menu anywhere, and several things Eagle does from the grid
needed the right bar or a toolbar button. Shortcuts follow Eagle's Windows defaults
(`default-preferences.js` of Eagle 4) where the key is free on this host.

- **Item menu.** A right click in the grid or the list. A cell outside the selection becomes
  the selection first; a cell inside keeps the whole selection. Preview, Open with default
  app, Show in Explorer, Copy file(s), Copy Name, Copy File Path, Rename…, Add to
  Folder(s)…, Remove from Current Folder (only in a folder), Rating ▸ five stars down to
  Remove Rating, Add Tags…, Copy Tags, Paste Tags, Clear Tags, Move to Trash. In the Trash:
  the reading commands, Restore, Delete permanently. On empty space with nothing selected:
  Add Files…, Import Folder…, Paste, New Folder, Select All. A read-only library keeps the
  reading commands and disables the rest. A live Eagle library disables what needs a local
  file (open, reveal, copy file, copy path).
- **Folder menu.** A right click in the folder tree opens the folder under the cursor, then
  offers New Subfolder, Rename…, Lock…, Export…, Delete…. These are the commands that
  already acted on the open folder. Outside a folder the menu is New Folder.
- **Shortcuts.** `F2` renames the one selected item, or the open folder when nothing is
  selected (before: the folder only). `1`–`5` rate the selection and `0` removes the rating;
  Eagle ships these commands without default keys, the digits are our choice. In the preview
  `0` still fits the image. `Ctrl+Shift+J` Add to Folder(s), `Ctrl+J` Go to Folder,
  `Shift+Delete` Remove from Current Folder, `Ctrl+T` Add Tags, `Ctrl+Shift+C` /
  `Ctrl+Shift+V` Copy / Paste Tags, `Ctrl+Alt+C` Copy File Path. A focused text field takes
  plain keys first (GPUI prefers character input), so digits typed into a field do not rate.
  Not taken from Eagle: `Ctrl+1`…`Ctrl+7` for places (`Ctrl+1` / `Ctrl+2` are Grid / List
  here) and `Ctrl+Enter` / `Shift+Enter` (already Open with default app here).
- **Folder panel.** One panel with a name field for both commands. Go to Folder: a click or
  Enter on the first match opens the folder. Add to Folder(s): for one item the ticks start
  from its folders and Apply sends the complete set (`SetFolders`), so a tick can also be
  removed; for several items the ticked folders are added (`AddToFolders`) and no item
  leaves a folder. A typed name and Enter with nothing ticked takes the first match. Locked
  folders are not listed (D-059); an item keeps its membership in them. This closes the
  "folder multi-membership checklist" gap of D-131.
- **Tags clipboard.** Copy Tags keeps the names inside the app, not on the system
  clipboard, as Eagle does. Case is kept. Paste adds; it removes nothing.
- **Copy Name / Copy File Path.** Text on the system clipboard, one line per item; names
  carry the extension.
- **Tests.** The menus are built from a plain model (`item_menu_model`,
  `folder_menu_model`) so they are tested without a window; so are the panel's rows, its
  filter and the edit it sends. A test parses every key binding and refuses a keystroke
  bound twice.
- **Not in this step.** Batch rename, the smart folder rule editor, dragging originals out
  to other apps, recent tags, pasting a file list, the context menu of tags and smart
  folders.
- **Verify.** `cargo test -p gpui-desktop ops_items`; `cargo test -p gpui-desktop commands`.

## D-141 GPUI chrome in Eagle's manner (2026-11-10)

Side by side with Eagle the GPUI host had plain defects: thumbnails did not fill their
cells, the selection could not be seen, the toolbar was one crowded row of text buttons with
white "on" states, and the left bar ran off the bottom of the window.

- **Thumbnails fill the cell.** The justified layout gives a cell the item's aspect ratio,
  but the image was drawn as a square of the row height, which left empty bands between
  cells. The image now covers the whole cell, with 4 px corners. Gap 8 px and padding 12 px
  (before 4 / 8); this host no longer matches the constants of `justified.ts` (D-067).
- **Captions.** With Show Names on: the name centred under the thumbnail and the pixel size
  under it, as in Eagle. The caption strip is 38 px (before 20). List rows show the size at
  the right.
- **Selection.** A 2 px ring drawn over the image, so selecting never moves or resizes a
  thumbnail, and the name on a filled pill. The colour is the theme's `blue`: the kit's
  `accent` is a hover grey and was invisible on a dark grid. Rows of the left bar and of the
  list use `list_active` / `list_hover`.
- **Toolbar, two rows.** Row 1: the name of the open place (not the library, which is in the
  window title) and its count; a thumbnail size slider between − and +; icon buttons with
  tooltips for Add Files, Imports, Sort By, Include Subfolders, Show Names, Grid / List;
  search. Row 2, the filter bar: Tags, Folders, Type, Shape, rating 0–5, Clear filters, and
  Export of the open folder or the one filtered tag at the right. A switch that is on is a
  selected ghost button, not a white primary one. Dragging the slider repacks the grid on
  every step and writes the preference once, on release; `Ctrl+=` / `Ctrl+-` move the slider.
- **Left bar.** Places stay on top; folders, locked folders, smart folders and tags scroll
  together below (tags used to be cut off at 40 and pushed the buttons out of the window).
  Icons on rows; folder icons take the folder colour. Section buttons sit in the section
  header: + for a folder, + for Save as smart folder, … for Tag groups. The header's Aa /
  Del / Lock text buttons are gone: rename, delete and lock are in the folder's right-click
  menu (D-140). A tag that filters the view is highlighted.
- **Right bar.** Rating is five stars filled up to the rating; a click on the current
  rating removes it. Folder colour is a row of colour dots. The bar is refreshed whenever
  the view is: after a change of place it kept showing the item selected before.
- **Icons.** `AllAssets` (the full Lucide set of `gpui-kit-assets`) instead of `Assets`:
  the default bundle has only the icons the kit's own components use. No new dependency.
- **Main thread stack 8 MiB (Windows, MSVC).** `build.rs` passes `/STACK:8388608` to the
  linker for the binary. The shell is one element tree built on the main thread, a debug
  build keeps every builder temporary on the stack, and Windows gives 1 MiB: the new
  toolbar overflowed it on opening a library (0xC00000FD), as the shell did once before
  (commit 185c477). Splitting functions only moved the limit.
- **Still open.** A view loads its first 500 items and no more (noted in D-129): a folder of
  2227 items shows 500. Waterfall layout, hover preview, a back / forward history and the
  filter bar's Color and Date filters are not here.
- **Verified** on a 300-item demo library by capturing the app's own window: grid, captions,
  selection, the left bar, the toolbar, the item's right-click menu.
  `cargo test -p gpui-desktop style`; `cargo test -p gpui-desktop`.

## D-142 GPUI rating filter behind a button; size slider in the middle (2026-11-10)

- **Rating filter.** The six inline buttons `☆ 0 1 2 3 4 5` of the filter bar (D-141) are now
  one Rating button with a star icon, like Tags, Folders, Type and Shape. It opens a menu:
  Any rating, Not rated, ★ … ★★★★★; the current choice is selected. The button reads
  "Rating" when off, "Not rated" for 0 and the stars otherwise, and is selected while the
  filter is on. The filter's meaning is unchanged (exactly that rating; 0 is unrated).
- **Toolbar row 1.** The place and the actions sit in two halves that share the free width
  equally (`flex_1` each), so the thumbnail size slider is in the middle of the row instead
  of wherever the place name ended. A long place name truncates; if the actions need more
  than half the row, the slider moves aside rather than overlapping them.
- **Verified.** `cargo test -p gpui-desktop` (label test in `ops_organize`); not yet checked
  by running the app.

## D-143 GPUI full-size preview loads off the UI thread, at full resolution (2026-11-10)

Opening the preview felt slow. Causes, in order of weight:

1. A debug build (`cargo run`, `scripts/run-gpui-desktop.ps1` without `-Release`) compiled the
   image decoders without optimisation: decoding one large JPEG took seconds.
2. The original was read on the UI thread, HEIC was even decoded there; then GPUI decoded the
   bytes again while painting, with nothing on screen until it finished. Every rotation
   decoded and re-encoded a PNG on the UI thread, and GPUI's asset cache kept every decoded
   preview for the life of the window.
3. HEIC showed its 512 px thumbnail, TIFF and PSD too (the WebView limit of spec §7.3); GPUI has
   no WebView.

What changed:

- **Dev profile.** `image`, `zune-jpeg`, `png`, `image-webp`, `gif`, `tiff` and their inflate
  crates build with `opt-level = 3` in dev, like the crypto crates. Release is unchanged.
- **Thumbnail first.** The preview opens at once with the right bar's thumbnail, sized to
  where the full image will land (the library's pixel size), and swaps in the full image when
  the background decode lands. "Loading…" shows in the top bar meanwhile. A failed decode
  keeps the thumbnail.
- **Background decode** (`browse/preview_image.rs`): read the original, decode with the EXIF
  orientation applied (GPUI's own decoder ignored it), scale down only past 8192 px on the long
  edge (GPU texture limits), convert to BGRA, and hand GPUI a `RenderImage`, so painting
  never decodes. GIF without rotation and SVG stay encoded: GPUI animates GIFs and renders
  SVG itself.
- **Full resolution for HEIC, PSD, TIFF.** `media::decode_full` returns the whole image,
  display-oriented, for every previewable kind but SVG. The HEIC plane is copied row by row
  instead of pixel by pixel.
- **Neighbours** (`browse/preview_cache.rs`): the open item and the one before and after
  decode together; stepping usually finds the next image ready. The cache keys on item id and
  thumbnail version and frees evicted textures with `App::drop_image`; closing the preview,
  opening or closing a library and Clear Cache free all of them.
- **Rotation** decodes the original again at the new angle in the background; the current
  frame stays up until the turned one is ready. Back at 0° the cached image returns at once.
- **Fit and controls.** The image box is computed from the window and the image's size
  (fit, never enlarged past 100 %, times the zoom) instead of a fixed 920×720 box, so the
  preview uses the whole window and a click beside the image really lands on the letterbox
  (a click on the image no longer closes it). Zooming keeps the point in the middle of the
  stage still (the pan offset scales with the zoom). Panning is tracked on the whole stage,
  so it continues when the pointer leaves the image. Stepping does not fade in again. New
  keys in the preview: `Home` / `End` (first / last), `Page Up` / `Page Down` (previous /
  next), `Ctrl+=` / `Ctrl+-` (zoom). Previous / next buttons sit at mid-height. The wheel
  zoomed the wrong way (GPUI reports a forward notch as positive `y`): forward now zooms in,
  back zooms out. Zoom goes from 10 % to 800 % of the fit (steps within 1 % of the fit snap to
  it); `0` or a click on the percentage fits and centres again. The image can be grabbed and
  dragged around the stage at any zoom, not only above 100 %. Over the image the cursor is a
  hand, also while dragging past its edge; on Windows the system hand (`PointingHand`), because
  GPUI maps `OpenHand` / `ClosedHand` to the arrow there.
- **Verified.** `cargo test -p gpui-desktop -p media` (decode, orientation, cap, rotation,
  cache eviction, fit box, zoom). Not yet measured in the running app.

## D-144 The gpui-desktop exe carries its icon (2026-11-10)

- **Problem.** The GPUI window and the taskbar showed the generic application icon. GPUI's
  Windows backend loads icon resource 1 from the exe for every window
  (`LoadImageW(module, 1, IMAGE_ICON)`); nothing put a resource into `gpui-desktop.exe`
  (the portable zip only copied `icon.ico` beside it).
- **Fix.** `crates/gpui-desktop/app.rc` declares `1 ICON` from `src-tauri/icons/icon.ico`
  (the same icon as the Tauri app); `build.rs` compiles it with `embed-resource` on Windows.
- **Dependency.** `embed-resource = "3.0.12"` as a Windows build-dependency: the version
  already in `Cargo.lock` through Tauri's `tauri-winres`, so no new code is downloaded; the
  newest-version rule (D-003) is met by keeping the lock's vetted version.
- **Verified.** The built exe's associated icon is the egl icon (extracted with
  `System.Drawing.Icon.ExtractAssociatedIcon`).

## D-145 GPUI scrolling: one handle for the items view, scrollbars on every pane (2026-11-10)

Nothing held a scroll handle, so the app could not move the view itself. Found and fixed:

- **Keyboard focus off screen.** Arrows, `Home` / `End`, `Page Up` / `Page Down` moved the
  selection below the window and the grid stayed put. The grid rows and the list now share
  one `VirtualListScrollHandle` (`RootApp::grid_scroll`); moving the focus scrolls just
  enough to show it (`ScrollStrategy::Nearest`).
- **After the preview.** Stepping through the preview selects each item; closing it now
  scrolls the grid to the item the preview ended on.
- **Opening a library** scrolls to the restored selection (centred).
- **Another view, same offset.** A new place, filter, sort or search kept the old pixel offset.
  The view's identity (`ViewKey`: place, filters, sort, direction) is compared after each
  reload; a different view starts at the top, a live update or an edit keeps the position.
- **Repacking.** The thumbnail slider, names on / off, grid / list and window width repack
  the rows; a pixel offset then pointed into other rows. The first item at the top of the
  view is kept at the top (`justified::first_index_at_offset`).
- **Rubber band while scrolled.** The band was measured in window coordinates, so a band drawn
  after scrolling selected items from the start of the view. It is now kept in content
  coordinates (scroll offset added) and drawn where the content is.
- **Scrollbars.** The grid / list, the left bar, the right bar, Settings, the Imports panel,
  Switch Library and the folder picker scroll with a visible scrollbar
  (`overflow_y_scrollbar`, `vertical_scrollbar`) instead of a bare `overflow_y_scroll`. The
  Type filter dialog had a height limit but no scrolling: a library with many extensions
  cut the list off; it scrolls now.
- **Preview.** The wheel over the preview zoomed the image and also scrolled the grid behind
  it. The preview overlay now occludes the mouse (`occlude`) and stops the wheel event.
- **Not done.** No autoscroll while a rubber band or a drag reaches the edge; the 12 px top
  padding of the grid stays fixed while the rows scroll under it.
- **Verified.** `cargo test -p gpui-desktop` (`offsets_map_to_rows_and_their_first_item`);
  not yet tried in the running app on a large library.

## D-146 GPUI filter and Sort menus open under their button (2026-11-10)

- **Problem.** Tags, Folders, Type, Shape, Rating and the toolbar's Sort opened a modal dialog
  in the middle of the window, away from the button that was clicked (Eagle opens them right
  under the button).
- **Single choices** (Sort, the tag match mode, Type, Shape, Rating) are now dropdown menus of
  gpui-component (`dropdown_menu`) anchored under their button; Sort, at the right of the
  toolbar, aligns its right edge with the button. The current choice is checked; a pick
  applies and closes the menu. The Type menu is capped at 60 % of the window height and
  scrolls. Builders live in `browse/filter_menus.rs` and read the library when the menu opens.
- **Folders** take several picks, so they are a `Popover` under the button that stays open:
  Any / All, then every folder with a check box; it re-reads the filter on every render.
  Rows are left-aligned check boxes indented 16 px per level (a `Button` centred its label, and
  indenting with spaces barely showed).
- **Ctrl+Shift+S** keeps the Sort dialog: a shortcut has no button to open under. Both use
  `filter_menus::sort_keys`, so the lists cannot drift apart. The old centred filter dialogs
  are gone.
- **Not yet like Eagle.** Eagle's Type and Tags menus have a search field, check boxes for
  several values and right-click to exclude; here Type is one value and the Tags button sets
  the match mode only (tags are picked in the left bar).
- **Verified.** `cargo test -p gpui-desktop` (`sort_keys`, `flatten_folders`); not yet tried
  in the running app.

## D-147 GPUI quit confirmation has its buttons (2026-11-10)

- **Problem.** Closing the window showed "Quit egl-viewer?" with no buttons: the dialog set
  `on_ok` / `on_cancel` but drew no footer, so the only way out was ×, which cancels. The
  message also still said "imports or exports (when added)".
- **Fix.** Cancel and a primary Quit button, as in the Move to Trash confirmation; × and Esc
  still cancel. The text is "Close the app? Imports still running will stop." (ru, zh too).
- **Closed hand while dragging the preview.** Windows has no closed-hand cursor and GPUI's
  Windows backend only sets system cursors; done with a patched backend in D-148.

## D-148 Open and closed hand cursors on Windows through a patched GPUI backend (2026-11-10)

- **Problem.** Over the preview's image the cursor should be an open hand that closes while
  dragging. Windows has no such cursors (only the pointing hand `IDC_HAND`), and
  `gpui-pre-windows` maps `CursorStyle::OpenHand` / `ClosedHand` to the arrow. It sets the
  cursor again on every mouse move, so the app cannot set its own from outside.
- **Patch.** `vendor/gpui-pre-windows` is the published 0.3.11 (0.3.12 since D-155) with one change in
  `src/util.rs` (`EGL-VIEWER-PATCH.md` there): `OpenHand` loads cursor resource 102 of the exe,
  `ClosedHand` resource 101, and without them the system hand. It is wired with
  `[patch.crates-io]`; the workspace excludes `vendor/`, so the copy is built as a dependency
  and not linted or tested as ours. Apache-2.0 allows the change; the note states it. When
  gpui-kit moves to a newer `gpui-pre-windows`, copy the new version and re-apply the change.
- **Cursors.** Drawn for this app, no third-party artwork:
  `crates/gpui-desktop/assets/cursors/make-cursors.ps1` writes `grab.cur` and `grabbing.cur`
  (32×32 canvas, a hand of about 20 px drawn anti-aliased with System.Drawing: white fill,
  black outline and lines between the fingers, alpha; hotspot in the palm); `app.rc`
  embeds them as cursors 101 and 102 next to the icon (D-144).
- **Preview.** `drag_cursor` is back to `OpenHand` over the image and `ClosedHand` while
  dragging, on every platform (macOS and Linux have both natively). The hand closes when the
  button goes down on the image (not on the first move) and opens on release; released
  outside the window, it opens on the next move (`pressed_button` is no longer Left).
- **Verified.** `cargo test -p gpui-desktop` (`the_hand_closes_while_dragging`,
  `the_hand_cursors_are_embedded_cursor_files`); the cursor pixels are found in the built exe.
  Not yet seen on screen.

## D-149 GPUI right bar: tag and folder chips with Eagle-style pickers (2026-11-10)

- **Problem.** Tags were ghost buttons (a click removed the tag) above a full-width "Add tag"
  that opened a modal dialog with a bare text field; folders were a line of names with no way
  to change them from the right bar.
- **Chips.** Tags and folders of the selection are chips with a ×. A tag on some of the
  selected items only is dimmed. Without tags the section shows "+ Add tag"; with tags a small
  "+" follows the chips; folders always end with "+".
- **Pickers** (`browse/picker.rs`), a popover under the "+" opening to the left, kept as one
  view per kind for the app's life:
  - search field with the caret in it on opening;
  - tags in two columns, by section: Recent (picked in this session), each tag group, Other;
    the count of items per tag; a search lists the matches, the ones starting with it first,
    and offers "Create tag “…”" when no tag has exactly that name (tags stay case-sensitive);
  - folders as the tree with check boxes and folder icons, Recent on top; a search lists
    matches by path; locked folders are not offered (D-059);
  - a click or Enter checks or unchecks the value on the selection and the popover stays
    open; arrows move (left / right move the caret while there is text); Esc closes; the
    footer shows the keys.
- **Ctrl+T** opens the tag picker when the right bar is shown; with the bar hidden it keeps the
  Add tag dialog, which has nothing to open under.
- **Not like Eagle yet.** Eagle's "Recommendations" (tags suggested for the item), the layout
  and filter toggles in the search row and Tab between lists are not here.
- **Verified.** `cargo test -p gpui-desktop` (sections, search, create, two-column arrows,
  folder tree and paths, recent list); not yet tried in the running app.

## D-150 GPUI start screen redesigned (2026-11-10)

- **Problem.** The start screen was a grey box of loose controls: two buttons, recent
  libraries as bare names with a red ×, and Eagle's two actions as centred text links. Missing
  libraries (an unplugged drive) were pruned on every render, which also wrote to the app
  store each frame.
- **Layout** (`screens/welcome.rs`), one card of two panes, in the theme's colours:
  - left: the app's mark, name and version, one line on what it is, then actions as rows with
    an icon, a title and what they do — Create library (primary), Open library, and under an
    "Eagle" label Import an Eagle library and Connect to a running Eagle; the status line at
    the bottom;
  - right: Recent libraries with their number, newest first; each row has the name, the
    folder it sits in, when it was last opened and a × (Remove from the list). A library whose
    folder is gone stays listed with "Not found" and does not open on a click. The list
    scrolls; without libraries a quiet empty state.
- Missing rows are pruned once at start-up, as before, no longer on every render.
- **Second pass** (the first looked rough): the card has its own background (in the light
  theme the shadow showed through the right pane as a grey block); the left pane takes the
  sidebar colour; only Create library is filled, the other actions are quiet rows that
  highlight on hover, every icon in a 32 px tile; hints stay on one line (ellipsis, shorter
  Russian texts); recent rows have the same tile and show their × on hover only; the count
  is a small pill.
- **Sidebar scrollbar.** The left bar's scrolled content has a 10 px right gutter
  (`style::SCROLLBAR_GUTTER`): the overlay scrollbar covered the counts (D-145).
- **Verified.** `cargo test -p gpui-desktop` (`recent_libraries_newest_first`,
  `the_line_under_a_name_is_its_folder`).

## D-151 GPUI right bar: name, note and link edited in place; actions only in the menu (2026-11-10)

- **In place.** Name, Note and Link were a value with an "Edit" button that opened a dialog.
  They are now fields in the right bar (`ops_details.rs`): Name and Link single-line, Note a
  text area that grows from 2 to 8 lines, with placeholders ("Add a note…", "https://").
  A change is saved when the field loses focus, and on Enter for Name and Link (Enter in a
  note is a new line). An empty name or unchanged text saves nothing and shows the item's
  value again; names and links are trimmed, notes kept as typed.
- **Saved to the right item.** A field remembers which item it was filled from and saves to
  that item, not to the selection: a click on another cell blurs the field after the
  selection has already moved. A field being typed in is not refilled when the right bar
  reloads.
- Read-only libraries and items in the trash show the values as text.
- **Info and EXIF** have larger titles (a chevron and the name, `text_sm`, semibold) that fold
  and unfold their section; folded sections stay folded while moving between items, for the
  session.
- **Actions.** Show in Explorer, Open with default app, Copy file and Move to Trash (and the
  trash's Restore / Delete permanently) are no longer buttons in the right bar; they stay in
  the item's right-click menu and their shortcuts.
- **Verified.** `cargo test -p gpui-desktop` (`ops_details`: unchanged, empty name, trimming);
  not yet tried in the running app.

## D-152 GPUI drag and drop to the left bar looks like Eagle's (2026-11-10)

- **Already there.** Items dropped on a folder join it, on a tag get it, on the Trash go to
  the trash (`handle_drop`); folders dropped on folders nest or reorder (D-140).
- **Drag image.** It was a blue label with the name. It is now the thumbnail of the item under
  the pointer (the focused or first selected one when several are dragged), at most 96 px,
  faded, with a blue badge counting the items when there are several. Folders and items
  without a thumbnail keep the label, also with the badge.
- **Drop target.** The left-bar row under a drag of items is tinted with the selection colour:
  folders, tags, the Trash, and the Folders header (which also takes folders, to the root).
  A folder dragged onto a folder keeps the zone lines (before / inside / after) instead.
- **Verified.** `cargo test -p gpui-desktop`; not yet tried in the running app.

## D-153 GPUI Windows NSIS + `gpui-latest` auto-update (2026-11-11)

Per-user Windows installer and in-app update channel for the GPUI host, without colliding
with the shipping Tauri app or GitHub Latest.

- **NSIS.** `scripts/installer/egl-viewer-gpui.nsi` + `scripts/package-gpui-nsis.ps1` package the
  same payload as the portable zip into
  `egl-viewer-gpui_<version>_windows-x64-setup.exe`. Install dir
  `%LOCALAPPDATA%\Programs\egl-viewer-gpui`, display name `egl-viewer (GPUI)`, current-user /
  `/S`, Start Menu + current-user desktop shortcut (`$DESKTOP\egl-viewer (GPUI).lnk`, same
  target/icon as Start Menu; removed on uninstall) + HKCU uninstall. Does not touch
  `%LOCALAPPDATA%\egl-viewer-gpui`. No MSI / Inno / Authenticode in this decision.
- **Release.** `gpui-v*` prereleases (`--latest=false`) attach the setup exe beside the zip.
  Dry-run (`workflow_dispatch` / PR) builds artifacts only. Rolling prerelease `gpui-latest`
  (same idea as `ios-latest`) holds `latest-gpui.json` pointing at the versioned NSIS URL;
  updated only on a real tag publish.
- **App.** Default endpoint
  `…/releases/download/gpui-latest/latest-gpui.json`. Windows: confirm → download setup → exit →
  silent NSIS → relaunch. macOS/Linux keep open-download. Minisign still deferred (empty
  signature). Windows **release** builds use `windows_subsystem = "windows"` (no console from
  Explorer / shortcuts / NSIS); debug keeps a console for `cargo run`. Logs append to
  `%LOCALAPPDATA%\egl-viewer-gpui\logs\egl-viewer-gpui.log` (D-079-style `Mutex<File>` writer;
  stderr still used when a console is attached; no `AllocConsole` in release).
- **Verify.** `cargo test -p gpui-desktop`; Windows dry-run of `.github/workflows/gpui-release.yml`
  produces the setup exe artifact.

## D-154 GPUI: playing animated GIFs in the preview and the grid (2026-11-11)

Before, the preview handed an unrotated GIF's bytes to GPUI, which animated it on its own clock
with no way to pause it; a rotated GIF showed its first frame only, and the decoded frames
stayed in GPUI's asset cache for the life of the window. The grid only had the still thumbnail.

- **Frames from `media`.** `media::decode_gif` returns every frame composited onto the canvas,
  with its delay, scaled to `max_edge` and halved (down to 128 px on the long edge) until all
  frames fit a byte budget; past that it refuses with `TooLarge` and the preview shows the first
  frame. Budgets: 384 MiB for the preview, 64 MiB under the pointer (cell size × 2, at most
  1024 px). The thumbnail pipeline is unchanged (first frame).
- **Our own frames, our own clock.** The preview decodes a GIF into one multi-frame
  `RenderImage` (`Shown::Animation`), turned like any other image, and paints one frame through
  `canvas` + `Window::paint_image`. GPUI's `img` is not used for it: it cannot pause or seek,
  races through 0 ms delays and stops in an inactive window. Its texture is freed with
  `App::drop_image` like the other previews (all frames at once).
- **`GifPlayer`** (`browse/gif_player.rs`, pure and tested): frame, play / pause, step (pauses,
  wraps), seek (keeps playing), speed 0.25×–2×. Delays of 10 ms or less play at 100 ms, as
  browsers do. Pauses do not count toward the next frame; after a long stall it skips whole
  loops. A turned GIF with the same frame count keeps its frame, speed and state.
- **One ticker.** A detached task sleeps until the next frame of whichever player is due (the
  preview's and the hover GIF's) and stops in the same update that sees nothing playing, so
  a quick pause and play can never leave a playing GIF without a ticker.
- **The bar.** At the bottom of the stage: previous frame, play / pause, next frame, a track of
  up to 240 segments (longer GIFs share segments; a seek lands on the first frame of its segment
  so the track shows it reached), the frame number, the loop length at 1× and the speed. Fixed
  widths keep the track still under a dragging pointer. The bar occludes the letterbox, so a
  click on it neither closes the preview nor starts a pan. Keys: `P`, `,`, `.` (in Help →
  Keyboard Shortcuts).
- **Grid.** Hovering a GIF cell in the justified grid reads the original in the background and
  plays it with its own `GifPlayer` in the cell (object-fit cover, rounded like the thumbnail);
  leaving the cell, another hover, opening or closing a library free its frames. Nothing plays
  on hover with Windows animation effects off, and a GIF opens paused in the preview then.
- **Not done here.** Animated WebP and APNG still show their first frame; the list view keeps
  still thumbnails; iOS and Android are unchanged.
- **Verify.** `cargo test -p media animation`, `cargo test -p gpui-desktop` (player, decode,
  keys); by hand on Windows: a GIF plays in the preview, `P` / `,` / `.` and the bar work,
  rotation keeps it playing, hovering a GIF in the grid plays it.

## D-155 gpui-kit 0.9.1 (2026-11-11)

- **Bump.** `gpui-kit` =0.9.0 → =0.9.1 (published 2026-11-09, older than the age rule of
  D-003). It moves the whole `gpui-pre-*` family from 0.3.11 to 0.3.12 and `gpui-component`,
  `gpui-base`, `gpui-kit-assets` to 0.9.1. No code in the app had to change.
- **The Windows patch moved with it.** `[patch.crates-io]` only applies to the exact version
  in `vendor/`, so 0.9.1 would have silently dropped the hand cursors of D-148 (the lock showed
  `gpui-pre-windows` from crates.io again). `vendor/gpui-pre-windows` is now the published
  0.3.12 with the same `load_cursor` change; upstream `src/util.rs` is identical in 0.3.11 and
  0.3.12, so the patch applies unchanged. `Cargo.lock` lists `gpui-pre-windows` 0.3.12 without
  a registry source, which is how to check that the patch is in use.
- **Clippy on Linux and macOS.** `updater::INSTALL_DIR_NAME` and `default_install_dir` serve
  only the Windows NSIS self-update; they are `#[cfg(windows)]` now (with their test), and the
  non-Windows fallback into the temp folder, which nothing used, is gone. `cargo clippy
  --workspace --all-targets -- -D warnings` passes on Linux again.

## D-156 CI: retry the vcpkg libheif install (2026-11-11)

- **Failure.** The `gpui-desktop` job of the merge of #93 died before building: vcpkg's curl got
  `SSL connect error` fetching the libde265 tarball from github.com, called it not transient and
  stopped. pwsh does not stop on a native command's exit code, so the step only failed later at
  "heif.dll missing", which hid the cause.
- **Change.** Every Windows `vcpkg install "libheif[core,libde265]:x64-windows"` (both `ci.yml`
  jobs and `gpui-release.yml`) runs up to three times, 20 s and 40 s apart, and throws
  "vcpkg install libheif failed after 3 attempts" when none succeeds. The decode-only checks
  (no x265, `heif.dll` and `libde265.dll` present) are unchanged.
