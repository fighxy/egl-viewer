# egl-viewer

A standalone desktop app for collections of images and design references: its own library
(`Name.eglib`) with folders, tags, ratings, notes, search, filters and smart folders.
Compatible with [Eagle](https://eagle.cool): import Eagle libraries, connect to a running Eagle
on the same PC or in the network, export into Eagle. **Shipping Windows desktop:** the native
GPUI Kit host (`gpui-desktop`). Windows first; no AI features.

- What the app does: [`docs/spec.md`](docs/spec.md)
- How it is built: [`docs/architecture.md`](docs/architecture.md)
- Decisions taken so far: [`docs/decisions.md`](docs/decisions.md)
- The native library format: [`docs/library-format.md`](docs/library-format.md)
- Captured Eagle responses for tests: [`fixtures/eagle-api/`](fixtures/eagle-api/)

## Status

| Area | State |
|---|---|
| Project skeleton | Done (2026-10-28): workspace, core types, the `LibrarySource` trait, Welcome screen, Connect screen without networking, tests, CI |
| Native library: create and open | Implemented (2026-10-28), CI on Windows green, the manual done check on Windows still open: `library-native` (create, open, the OS lock, recovery, backups, `add_file`), library locations and recent libraries in `app-store`, the library commands, the window frame with the menu bar, Welcome, the empty library screen, Settings, About and Keyboard Shortcuts (D-032) |
| Adding files and the media pipeline | Implemented (2026-10-28), CI on Windows green, the manual done check on Windows still open: the `media` crate (thumbnails, palettes, EXIF), batches with one duplicate question, Add files, dropping and pasting, progress, the Imports panel, counts (D-037). Import Folder… reviews one picked folder before copy (D-061) |
| Grid and detail | Implemented (2026-10-28), waiting for the done check on Windows: the local pipeline, the sorter and views in `library-core`, `load_scope` in SQL and the native `LibrarySource`, the view commands, the `thumb` scheme, the virtualized justified grid and list, selection, sort, thumbnail size, the read-only details panel, the full-size preview (D-041, D-067) |
| Folders | Implemented (2026-10-28), waiting for the done check on Windows. 4a: the folder tree with counts, create, rename, colour, description, folder tags, move with the cycle guard, sibling order, delete, membership (picker, dragging items onto folders, files added into the open folder, mirrored dropped folders), Include subfolders, the folder's own sort, manual order by dragging (D-042, D-043). 4b: locked folders, encrypted at rest with age, each with its own password, Argon2id, recovery key and item keys; the Locked folders section (locked folders are listed only there, not also in Folders, D-059), unlock and Lock All Folders, change password, a new recovery key, removing a lock (D-044, D-045). A YubiKey 5C is an optional third wrap (retired PIV slots 82–95); app login is exactly one of password, PIN, or WebAuthn (D-056) |
| Tags | Implemented (2026-10-28), waiting for the done check on Windows: the tag editor in the right bar with vocabulary suggestions, tagging a selection with tags on only some items dimmed, the Tags filter (counts, Any / All / Exact / None, recent tags, groups), the filter bar, rename, merge and delete everywhere, tag groups, Tag groups and Recent tags in the left bar (D-046). Dropping items onto a tag row there adds that tag (D-057) |
| Search, filters and smart folders | Implemented (2026-10-29), waiting for the done check on Windows: search as in Eagle (any part of a word, `OR`, `-`, parentheses) through an FTS5 trigram index (format version 3), the filter bar with Tags, Folders, Type, Rating and Shape, smart folders with their own tree, Eagle's rules evaluated locally for the v1 subset, the rule editor, Save as smart folder (D-047, D-048) |
| Editing and trash | Implemented (2026-10-29), waiting for the done check on Windows: name (the stored file is renamed too), note, link and rating edited in place, batch rating, undo of the last edits (`Ctrl+Z`), the trash with restore, delete permanently and Empty Trash (files removed, with progress and cancel — D-074), dragging items onto the Trash row (same confirmation as Delete, D-073), Open with the default app, Show in Explorer, Copy File, dragging originals out from the right bar, the right-click menu, Library Settings with backups and restore, Check Library with repair, Rebuild Thumbnails (D-049 to D-052). Backup… / Export… copy originals to a folder, a ZIP or a 7z (optional passphrase of at least 8 characters: an age file encrypted while written, an AES-256 ZIP for 7-Zip / WinRAR, or a 7z with hidden names; D-112, D-113), with progress and cancel (D-064 to D-066); File → Decrypt Archive… opens an encrypted export again (D-110) |
| Eagle: import from a library folder | Implemented (2026-11-01), the done check on Windows with a real Eagle library still open: the `eagle-folder` crate (read-only, retries on half-written JSON, passwords dropped), the resumable import in transactions of 500 with the `eagle_import` state table (format version 5, D-072), Eagle thumbnails as the first thumbnails, locked folders imported locked with no identity and plaintext files, the wizard from Welcome and the File menu, the report (imported, duplicates by hash, skipped, locked folders) (D-071) |
| Eagle: live connection (browse) | Implemented (2026-11-01), done checks against a real Eagle still open: `eagle-api` transport (`EagleHttp`, errors, pagination), `SecretStore` on Credential Manager, `connect_eagle` / `eagle_session` / `disconnect_eagle`, Connect screen wired, `EagleSource` browse (D-075) |
| Eagle: live edits, adds, thumbnails | Implemented (2026-11-01), done checks against a real Eagle still open: `ItemMutator` / folder updates / tag rename-merge, same-PC `path` and remote data-URL `item/add` with “Importing…” cells, same-PC thumbnails (incl. `noThumbnail` downscale); remote thumbs wait for the helper plugin; undo of item edits stays native-only |
| Eagle: export into Eagle | Implemented (2026-11-01), done check against a real Eagle still open: Export… offers “Into a running Eagle”, folders top-down then items (same-PC `path` / remote data URL), comments, tag groups and smart folders through the API; `eagle_id` columns store the mapping so a second export updates (D-077) |
| Hardening and the Windows release | In progress: restore place/filters/selection per library (D-078); file logs + Help → Open Log Folder (D-079); clear Eagle token from Connect/Export forms after send (D-080); a11y Home/End and filter aria (D-081); updater (D-062) and bundled libheif (D-070) already in. Custom title bar (D-025), Authenticode, notarization, 100k/manual done checks and the clean-machine done check remain open |
| iOS app | Done (2026-11-05): the foundation on `EglCore` and the shared core (D-091–D-100): Welcome, create in On My iPhone, open from Files (iCloud read-only), recent libraries; browsing (D-094): places, folders and smart folders, a lazy grid and a list by view id, sort, thumbnail size, search, filters, selection with Select All, the saved view; the preview with zoom, item details and edits, actions on a selection with undo, sharing originals (D-095); adding from Photos, Files, the clipboard and drops with the duplicate question, HEIC through ImageIO (D-096); folders, tags, tag groups and the smart folder editor (D-097); locked folders, locking on leaving and the app login with Face ID (D-098); Settings with backups, check and repair, rebuilt thumbnails and export to Files (D-099); Eagle library import into a new library (D-116); light and dark, en / ru / zh-Hans; icon, privacy manifest, accessibility audit, 10,000 items, unsigned IPA releases by tag `ios-vX.Y.Z` (D-100); see [`ios/README.md`](ios/README.md) |
| Android app | Done (2026-11-05): A1–A8 (D-101…D-108, ≈ iOS D-093…D-100): Kotlin `eglcore` over JNI / `library-ffi`, real `.so` (no mock), Welcome, places, lazy grid by view id, thumbs, filters, selection; full-screen preview pager + zoom, details sheet, selection actions with undo snackbar, share originals (≤100 temp copies); adding from Gallery / Files / folder / paste / drops with duplicate question and HEIC through ImageDecoder; folders / tags / tag groups / smart folder editor; locked folders + App Lock (PIN / biometrics, relock on background); Settings with backups, check and repair, rebuilt thumbnails, export and Decrypt Archive (no Eagle); libraries anywhere on the phone with All files access (D-111); Eagle library import into a new library (D-116); icon, Privacy screen, TalkBack labels, 10,000-item perf test, APK/AAB releases by tag `android-vX.Y.Z` (D-108); en / ru / zh; see [`android/README.md`](android/README.md) |
| Mobile CI | [`.github/workflows/mobile.yml`](.github/workflows/mobile.yml): Linux `library-ffi` and the shells' layout and strings; Android cargo-ndk `.so`, JVM unit tests, `assembleDebug`, tagged APK/AAB (`android-release`); macOS `EglCore` tests (`ios-core`), app unit and UI tests on a simulator (`ios-app`), the unsigned device IPA (`ios-ipa`, rolling `ios-latest`) (D-084, D-092, D-093, D-100, D-101, D-108) |
| GPUI desktop (the shipping Windows app) | Portable zip + per-user NSIS (`scripts/package-gpui-desktop.ps1` / `package-gpui-nsis.ps1`, install to `%LOCALAPPDATA%\Programs\egl-viewer-gpui`, display name “egl-viewer (GPUI)”). Help → Check for Updates… / silent check against the rolling `gpui-latest` `latest-gpui.json` (Windows: confirm → silent NSIS → relaunch; macOS/Linux open-download; Authenticode / minisign deferred) (D-153). About licenses. CI (`.github/workflows/ci.yml`, push and pull request to `feat/gpui-desktop` and `main`): rustfmt, clippy, workspace tests and the portable zip on Windows; Linux runs the workspace tests except the GPUI bin and builds `gpui-desktop`; macOS builds `gpui-desktop`. Feature slices remain (D-129…D-132). Library menu (Create, Open, Switch Ctrl+L, Clear History, Clear Cache and Reload) and Eagle-style Settings (D-137). Minimum window 960×600 (D-138). Show in Explorer stays up with the app (D-139). Right-click menus for items and folders, Eagle's item shortcuts (rating keys, `F2`, tag copy / paste), Add to Folder(s) `Ctrl+Shift+J` and Go to Folder `Ctrl+J` (D-140). Eagle-like chrome: thumbnails fill their cells, captions with pixel size, a two-row toolbar with a size slider and a filter bar, icons and a visible selection in the left bar (D-141). Rating filter behind a button, size slider centred (D-142). Full-size preview decodes in the background at full resolution (HEIC, PSD, TIFF too), shows the thumbnail first and preloads neighbours (D-143). The exe carries its icon (D-144). Scrolling follows the keyboard focus and the preview, resets per view, keeps its anchor on repack; scrollbars on every pane (D-145). Filter and Sort menus open under their button (D-146). The quit confirmation has Quit / Cancel buttons (D-147). Open / closed hand cursors on Windows via a patched GPUI backend in `vendor/` (D-148). Right bar: tag and folder chips with Eagle-style picker popovers (D-149). Redesigned start screen (D-150). Name, note and link edited in place in the right bar; item actions only in the context menu (D-151). Drag image with the thumbnail and a count; drop targets tinted (D-152). Compact bars above the library: menu 30 px, toolbar 36 px, filter bar 30 px (D-157). Include Subfolders sits by ⊕ in the Folders header, not in the toolbar (D-158). Folders take any colour: the eight dots plus a colour picker, stored as `#rrggbb`, sent to Eagle as the nearest of its eight (D-159). The ⋯ of the Tags header and of a locked folder open menus under the button (D-160). ⊕ New Folder is a popover under the button (D-161). Tag groups are rows of the Tags section that unfold into their tags, with Ungrouped after them and context menus for groups and tags (D-162). ⊕ Save Smart Folder is the same popover (D-163). Right-click on Trash: size, Restore All, Empty Trash… (D-164), confirmed in a popover at the row (D-165), then a window with the progress and the result (D-172). About shows the pinned gpui-kit version; the licenses wrap and scroll (D-166). Views of any size: the grid is laid out for the whole view and fetches the pages on screen as it scrolls; Select All is a flag (D-167). A Colour filter with a Documents preset finds photographed papers by their palette (D-176); any colour can be picked with sliders or hex (D-178). Animated GIFs: a player in the preview (play / pause `P`, frame steps `,` / `.`, a frame track to click or drag, 0.25×–2× speed, playing at any rotation) and playback under the pointer in the grid (D-154). Video and audio (D-168…D-171): FFmpeg (LGPL DLLs) with the Scrobb audio path in the `av` crate; video posters and durations in the library; a player in the preview (play / pause `P` or `K`, frame steps `,` / `.`, `J` / `L` 5 s, `M` mute, a seek track with scrubbing, volume, 0.25×–2× speed); frames under the pointer in the grid; All · Photo · Video · Audio modes as a segmented control in the middle of the toolbar, the size slider beside the view buttons and Add Files as a labelled button (D-173). Dates show in the computer's time zone (D-174). The Imports button shows only while something is imported or a report waits; View → Imports opens the panel (D-175). In a narrow window the modes show icons only and the size slider hides (D-177). Search follows the typing, and text fields keep Backspace, arrows and Ctrl+A / C / V / Z (D-179). Photos and videos open on their own from Explorer: `gpui-desktop.exe "<file>"` shows the preview over the whole window and steps through the folder; the NSIS setup registers the types per user for "Open with" and Default apps (D-180). Deferred: Authenticode, minisign, clean-machine done check, merging libraries, interface zoom |

What is out of scope is in [`docs/spec.md` §9](docs/spec.md). A tag `vX.Y.Z` (the workspace
version in `Cargo.toml`, notes in `docs/releases/X.Y.Z.md`) builds the Windows GPUI portable zip and
publishes the GitHub Release with it (legacy `latest-gpui.json` on that path still exists; the
GPUI app reads the rolling channel below). A tag `gpui-vX.Y.Z` builds Windows (NSIS setup +
portable zip), Linux and macOS GPUI packages, publishes a **prerelease** (never Latest), and
updates the rolling `gpui-latest` prerelease with `latest-gpui.json` via
[`.github/workflows/gpui-release.yml`](.github/workflows/gpui-release.yml) (D-153). A tag
`ios-vX.Y.Z` publishes the iPhone IPA; a tag `android-vX.Y.Z` publishes the Android APK and AAB
(D-108).

## Requirements (Windows)

- Rust via `rustup` (the toolchain in `rust-toolchain.toml` installs on first use)
- Visual Studio Build Tools 2022 with "Desktop development with C++"
- [vcpkg](https://vcpkg.io) with `libheif[core,libde265]:x64-windows` for HEIC/HEIF (D-063);
  set `VCPKG_ROOT` and `VCPKGRS_DYNAMIC=1`, and put `installed/x64-windows/bin` on `PATH` for
  tests. A portable release build also runs `pwsh scripts/bundle-heif-dlls.ps1` so the zip
  ships `heif.dll` and `libde265.dll` beside the exe (D-070)
- The LGPL FFmpeg for video and audio (D-168), from the same vcpkg:
  `vcpkg install "ffmpeg[core,avcodec,avformat,avfilter,swresample,swscale,dav1d,zlib]:x64-windows"`
  (never the `gpl` or `nonfree` features); `scripts/bundle-ffmpeg-dlls.ps1` stages its DLLs
- LLVM for libclang (`winget install LLVM.LLVM`, or set `LIBCLANG_PATH`): `ffmpeg-sys-next`
  generates its bindings with bindgen
- Eagle 4.0 or newer, only for the Eagle features (import, live connection, export)

## Getting started

```bash
pwsh scripts/run-gpui-desktop.ps1
```

(or `cargo run -p gpui-desktop` with `VCPKG_ROOT` / `VCPKGRS_DYNAMIC` / the vcpkg
`installed/x64-windows/bin` heif PATH as above). If workspace `target/` grows past 20 GiB on
Windows, `pwsh scripts/clean-target-if-huge.ps1` runs `cargo clean` (D-135; never touches
`.eglib` or vcpkg).

## Commands

| Command | What it does |
|---|---|
| `pwsh scripts/run-gpui-desktop.ps1` | Run the GPUI Kit desktop host with vcpkg heif on PATH (D-133 / D-134) |
| `pwsh scripts/package-gpui-desktop.ps1` | Stage/zip GPUI portable layout + heif DLLs and the FFmpeg DLLs (Windows) |
| `pwsh scripts/bundle-ffmpeg-dlls.ps1` | Stage the LGPL FFmpeg DLLs and notices; refuses a GPL or non-free FFmpeg (D-168) |
| `pwsh scripts/package-gpui-nsis.ps1` | Build per-user NSIS setup from the portable stage (needs `makensis`) |
| `bash scripts/package-gpui-linux.sh` | Stage/tar GPUI Linux x86_64 layout + bundled libheif |
| `bash scripts/package-gpui-macos.sh` | Stage/zip unsigned `EglViewer.app` + Frameworks libheif (macOS) |
| `pwsh scripts/clean-target-if-huge.ps1` | If workspace `target/` is over 20 GiB, run `cargo clean` (D-135; never touches `.eglib` / vcpkg) |
| `pwsh scripts/package-gpui-desktop.ps1 -CleanHugeTarget` | Same size check before a GPUI portable build |
| `npm run check` | rustfmt, clippy and Rust tests (the Windows `check` job) |
| `cargo test --workspace` | Rust tests |
| `EGL_DEMO_PARENT=<folder> cargo test --release -p library-native -- --ignored demo_library` | Makes `Demo.eglib` with 10 000 images in that folder, for checking the app by hand (`EGL_DEMO_COUNT` changes the number) |
| `EGL_DEMO_SOURCE=<folder>` with the command above | Makes `Demo.eglib` from the files in that folder instead (videos and audio to try, D-170) |
| `cargo run -p av --example play -- <file> [seconds]` | Plays a file on the default audio device and prints the clock, frames and a seek (D-168) |

## Layout

```
crates/library-core/   models, queries and views (filters, sorter), edits, errors, the LibrarySource trait (no IO)
crates/library-native/ the native library: create, open, the library lock, recovery, adding files, scopes in SQL, edits, backups, export of originals, checks
crates/app-service/    the open native library as an app uses it: views, undo, edits, tasks (desktop and mobile, D-091)
crates/desktop-session/ toolkit-free desktop Session over app-service (D-127); shared by the desktop hosts
crates/library-ffi/    C ABI over app-service for the mobile apps (D-082, D-083, D-092); no Eagle
crates/app-store/      settings, recent libraries, library locations, secrets
crates/eagle-api/      a running Eagle as a LibrarySource (transport for the live connection)
crates/eagle-folder/   read-only Eagle `.library` folder parsing (D-071)
crates/media/          decoding, thumbnails, palettes and EXIF for the native library
crates/gpui-desktop/   GPUI Kit desktop host (D-126–D-133); `cargo run -p gpui-desktop`; portable zip via `scripts/package-gpui-desktop.ps1`
ios/                   experimental SwiftUI iPhone draft (D-082); see ios/README.md
android/               Jetpack Compose app (A1–A8, D-101–D-108); see android/README.md
docs/                  spec, architecture, decisions
fixtures/eagle-api/    real Eagle 4.0.0 request and response captures
fixtures/eagle-library/ synthetic Eagle `.library` for the folder import (D-071)
```

## Tests

Every change comes with tests. Rust logic is tested with `cargo test` (unit tests next to the
code, contract tests in `crates/*/tests`).
