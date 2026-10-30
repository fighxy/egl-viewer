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
| 2. Adding files and the media pipeline | Implemented (2026-10-28), CI on Windows green, the manual done check on Windows still open: the `media` crate (thumbnails, palettes, EXIF), batches with one duplicate question, Add files, dropping and pasting, progress, the Imports panel, counts (D-037) |
| 3. Grid and detail | Implemented (2026-10-28), waiting for the done check on Windows: the local pipeline, the sorter and views in `library-core`, `load_scope` in SQL and the native `LibrarySource`, the view commands, the `thumb` scheme, the virtualized grid and list, selection, sort, thumbnail size, the read-only details panel, the full-size preview (D-041) |
| 4. Folders | Implemented (2026-10-28), waiting for the done check on Windows. 4a: the folder tree with counts, create, rename, colour, description, folder tags, move with the cycle guard, sibling order, delete, membership (picker, dragging items onto folders, files added into the open folder, mirrored dropped folders), Include subfolders, the folder's own sort, manual order by dragging (D-042, D-043). 4b: locked folders, encrypted at rest with age, each with its own password, Argon2id, recovery key and item keys; the Locked folders section (locked folders are listed only there, not also in Folders, D-059), unlock and Lock All Folders, change password, a new recovery key, removing a lock (D-044, D-045). A YubiKey 5C is an optional third wrap (retired PIV slots 82–95); app login is exactly one of password, PIN, or WebAuthn (D-056) |
| 5. Tags | Implemented (2026-10-28), waiting for the done check on Windows: the tag editor in the right bar with vocabulary suggestions, tagging a selection with tags on only some items dimmed, the Tags filter (counts, Any / All / Exact / None, recent tags, groups), the filter bar, rename, merge and delete everywhere, tag groups, Tag groups and Recent tags in the left bar (D-046). Dropping items onto a tag row there adds that tag (D-057) |
| 6. Search, filters and smart folders | Implemented (2026-10-29), waiting for the done check on Windows: search as in Eagle (any part of a word, `OR`, `-`, parentheses) through an FTS5 trigram index (format version 3), the filter bar with Tags, Folders, Type, Rating and Shape, smart folders with their own tree, Eagle's rules evaluated locally for the v1 subset, the rule editor, Save as smart folder (D-047, D-048) |
| 7. Editing and trash | Implemented (2026-10-29), waiting for the done check on Windows: name (the stored file is renamed too), note, link and rating edited in place, batch rating, undo of the last edits (`Ctrl+Z`), the trash with restore, delete permanently and Empty Trash (files removed), Open with the default app, Show in Explorer, Copy File, dragging originals out from the right bar, the right-click menu, Library Settings with backups and restore, Check Library with repair, Rebuild Thumbnails (D-049 to D-052) |
| 8–11. Eagle: import, live connection, export | Planned |
| 12. Hardening and the Windows release | Planned |

The full plan is in [`docs/spec.md` §9](docs/spec.md).

## Requirements (Windows)

- Rust via `rustup` (the toolchain in `rust-toolchain.toml` installs on first use)
- Visual Studio Build Tools 2022 with "Desktop development with C++"
- WebView2 Runtime (part of Windows 11)
- Node.js 24 LTS and pnpm 12 (`npm install --global pnpm`)
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
crates/library-native/ the native library: create, open, the library lock, recovery, adding files, scopes in SQL, edits, backups, checks
crates/app-store/      settings, recent libraries, library locations, secrets
crates/eagle-api/      a running Eagle as a LibrarySource (transport from phase 9)
crates/media/          decoding, thumbnails, palettes and EXIF for the native library
(later crates)         eagle-folder (phase 8)
src-tauri/             the Tauri app: commands, configuration, capabilities, icons
ui/                    React + TypeScript + Tailwind + shadcn/ui
docs/                  spec, architecture, decisions
fixtures/eagle-api/    real Eagle 4.0.0 request and response captures
```

## Tests

Every change comes with tests. Rust logic is tested with `cargo test` (unit tests next to the
code, contract tests in `crates/*/tests`), the UI with Vitest and Testing Library, with Tauri IPC
mocked through `@tauri-apps/api/mocks`.
