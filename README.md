# egl-viewer

A standalone desktop app for collections of images and design references: its own library
(`Name.eglib`) with folders, tags, ratings, notes, search, filters and smart folders.
Compatible with [Eagle](https://eagle.cool): import Eagle libraries, connect to a running Eagle
on the same PC or in the network, export into Eagle. Built with Tauri 2, a Rust core and a
React UI. Windows first; no AI features.

- What the app does: [`docs/spec.md`](docs/spec.md)
- How it is built: [`docs/architecture.md`](docs/architecture.md)
- Decisions taken so far: [`docs/decisions.md`](docs/decisions.md)
- The native library format: [`docs/library-format.md`](docs/library-format.md)
- Captured Eagle responses for tests: [`fixtures/eagle-api/`](fixtures/eagle-api/)

## Status

| Phase | State |
|---|---|
| 0. Project skeleton | Done (2026-10-28): workspace, core types, the `LibrarySource` trait, Welcome screen, Connect screen without networking, tests, CI |
| 1. Native library: create and open | Implemented (2026-10-28), CI on Windows green, the manual done check on Windows still open: `library-native` (create, open, the OS lock, recovery, backups, `add_file`), library locations and recent libraries in `app-store`, the library commands, the window frame with the menu bar, Welcome, the empty library screen, Settings, About and Keyboard Shortcuts (D-032) |
| 2. Adding files and the media pipeline | Implemented (2026-10-28), CI on Windows green, the manual done check on Windows still open: the `media` crate (thumbnails, palettes, EXIF), batches with one duplicate question, Add files, dropping and pasting, progress, the Imports panel, counts (D-037). Import Folder… reviews one picked folder before copy (D-061) |
| 3. Grid and detail | Implemented (2026-10-28), waiting for the done check on Windows: the local pipeline, the sorter and views in `library-core`, `load_scope` in SQL and the native `LibrarySource`, the view commands, the `thumb` scheme, the virtualized justified grid and list, selection, sort, thumbnail size, the read-only details panel, the full-size preview (D-041, D-067) |
| 4. Folders | Implemented (2026-10-28), waiting for the done check on Windows. 4a: the folder tree with counts, create, rename, colour, description, folder tags, move with the cycle guard, sibling order, delete, membership (picker, dragging items onto folders, files added into the open folder, mirrored dropped folders), Include subfolders, the folder's own sort, manual order by dragging (D-042, D-043). 4b: locked folders, encrypted at rest with age, each with its own password, Argon2id, recovery key and item keys; the Locked folders section (locked folders are listed only there, not also in Folders, D-059), unlock and Lock All Folders, change password, a new recovery key, removing a lock (D-044, D-045). A YubiKey 5C is an optional third wrap (retired PIV slots 82–95); app login is exactly one of password, PIN, or WebAuthn (D-056) |
| 5. Tags | Implemented (2026-10-28), waiting for the done check on Windows: the tag editor in the right bar with vocabulary suggestions, tagging a selection with tags on only some items dimmed, the Tags filter (counts, Any / All / Exact / None, recent tags, groups), the filter bar, rename, merge and delete everywhere, tag groups, Tag groups and Recent tags in the left bar (D-046). Dropping items onto a tag row there adds that tag (D-057) |
| 6. Search, filters and smart folders | Implemented (2026-10-29), waiting for the done check on Windows: search as in Eagle (any part of a word, `OR`, `-`, parentheses) through an FTS5 trigram index (format version 3), the filter bar with Tags, Folders, Type, Rating and Shape, smart folders with their own tree, Eagle's rules evaluated locally for the v1 subset, the rule editor, Save as smart folder (D-047, D-048) |
| 7. Editing and trash | Implemented (2026-10-29), waiting for the done check on Windows: name (the stored file is renamed too), note, link and rating edited in place, batch rating, undo of the last edits (`Ctrl+Z`), the trash with restore, delete permanently and Empty Trash (files removed, with progress and cancel — D-074), dragging items onto the Trash row (same confirmation as Delete, D-073), Open with the default app, Show in Explorer, Copy File, dragging originals out from the right bar, the right-click menu, Library Settings with backups and restore, Check Library with repair, Rebuild Thumbnails (D-049 to D-052). Backup… / Export… copy originals to a folder or ZIP (optional age passphrase), with progress and cancel (D-064 to D-066) |
| 8. Eagle: import from a library folder | Implemented (2026-11-01), the done check on Windows with a real Eagle library still open: the `eagle-folder` crate (read-only, retries on half-written JSON, passwords dropped), the resumable import in transactions of 500 with the `eagle_import` state table (format version 5, D-072), Eagle thumbnails as the first thumbnails, locked folders imported locked with no identity and plaintext files, the wizard from Welcome and the File menu, the report (imported, duplicates by hash, skipped, locked folders) (D-071) |
| 9. Eagle: live connection (browse) | Implemented (2026-11-01), done checks against a real Eagle still open: `eagle-api` transport (`EagleHttp`, errors, pagination), `SecretStore` on Credential Manager, `connect_eagle` / `eagle_session` / `disconnect_eagle`, Connect screen wired, `EagleSource` browse (D-075) |
| 10. Eagle: live edits, adds, thumbnails | Implemented (2026-11-01), done checks against a real Eagle still open: `ItemMutator` / folder updates / tag rename-merge, same-PC `path` and remote data-URL `item/add` with “Importing…” cells, same-PC thumbnails (incl. `noThumbnail` downscale); remote thumbs wait for the helper plugin; undo of item edits stays native-only |
| 11. Eagle: export into Eagle | Implemented (2026-11-01), done check against a real Eagle still open: Export… offers “Into a running Eagle”, folders top-down then items (same-PC `path` / remote data URL), comments, tag groups and smart folders through the API; `eagle_id` columns store the mapping so a second export updates (D-077) |
| 12. Hardening and the Windows release | In progress: restore place/filters/selection per library (D-078); file logs + Help → Open Log Folder (D-079); clear Eagle token from Connect/Export forms after send (D-080); a11y Home/End and filter aria (D-081); updater (D-062) and bundled libheif (D-070) already in. Custom title bar (D-025), Authenticode, notarization, 100k/manual done checks and the clean-machine done check remain open |

The full plan is in [`docs/spec.md` §9](docs/spec.md). The Project-store plan for auto-update is mirrored by **D-062** in [`docs/decisions.md`](docs/decisions.md); set GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` (and optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) before tagging a release, and attach `latest.json` (see `scripts/write-latest-json.mjs`) to the GitHub Release. Already-installed builds without the updater need **one last manual install** of the first updater-enabled release.

## Requirements (Windows)

- Rust via `rustup` (the toolchain in `rust-toolchain.toml` installs on first use)
- Visual Studio Build Tools 2022 with "Desktop development with C++"
- WebView2 Runtime (part of Windows 11)
- Node.js 24 LTS and pnpm 12 (`npm install --global pnpm`)
- [vcpkg](https://vcpkg.io) with `libheif[core,libde265]:x64-windows` for HEIC/HEIF (D-063);
  set `VCPKG_ROOT` and `VCPKGRS_DYNAMIC=1`, and put `installed/x64-windows/bin` on `PATH` for
  tests. A release / NSIS build also runs `pwsh scripts/bundle-heif-dlls.ps1` so the installer
  ships `heif.dll` and `libde265.dll` beside the exe (D-070)
- Eagle 4.0 or newer, only for the Eagle phases (9–11)

## Getting started

```bash
pnpm install
pnpm dev
```

`pnpm dev` starts Vite and the Tauri window with hot reload. The UI alone also runs in a browser
(`pnpm --filter egl-viewer-ui dev`, then http://localhost:5173), without the Rust side.

## Commands

| Command | What it does |
|---|---|
| `pnpm dev` | Run the app in development mode |
| `pnpm build` | Build the release app and the NSIS installer |
| `pnpm check` | Everything CI runs: rustfmt, clippy, Rust tests, Biome, TypeScript, UI tests |
| `cargo test --workspace` | Rust tests; also regenerates the TypeScript types in `ui/src/ipc/generated/` |
| `pnpm test` | UI tests (Vitest) |
| `pnpm lint` | Biome lint and format check |
| `pnpm typecheck` | TypeScript type check |
| `EGL_DEMO_PARENT=<folder> cargo test --release -p library-native -- --ignored demo_library` | Makes `Demo.eglib` with 10 000 images in that folder, for checking the app by hand (`EGL_DEMO_COUNT` changes the number) |

## Layout

```
crates/library-core/   models, queries and views (filters, sorter), edits, errors, the LibrarySource trait (no IO)
crates/library-native/ the native library: create, open, the library lock, recovery, adding files, scopes in SQL, edits, backups, export of originals, checks
crates/app-store/      settings, recent libraries, library locations, secrets
crates/eagle-api/      a running Eagle as a LibrarySource (transport from phase 9)
crates/eagle-folder/   read-only Eagle `.library` folder parsing (phase 8, D-071)
crates/media/          decoding, thumbnails, palettes and EXIF for the native library
src-tauri/             the Tauri app: commands, configuration, capabilities, icons
ui/                    React + TypeScript + Tailwind + shadcn/ui; i18n catalogs in ui/src/i18n/ (en, ru, zh-CN; D-069)
docs/                  spec, architecture, decisions
fixtures/eagle-api/    real Eagle 4.0.0 request and response captures
fixtures/eagle-library/ synthetic Eagle `.library` for the folder import (D-071)
```

## Tests

Every change comes with tests. Rust logic is tested with `cargo test` (unit tests next to the
code, contract tests in `crates/*/tests`), the UI with Vitest and Testing Library, with Tauri IPC
mocked through `@tauri-apps/api/mocks`.
