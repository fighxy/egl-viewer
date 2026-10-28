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
| 2. Adding files and the media pipeline | Implemented (2026-10-28), waiting for the done check on Windows: the `media` crate (thumbnails, palettes, EXIF), batches with one duplicate question, Add files, dropping and pasting, progress, the Imports panel, counts (D-037) |
| 3–7. Grid, folders, tags, search and smart folders, editing and trash | Planned |
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

## Layout

```
crates/library-core/   models, queries, edits, errors, the LibrarySource trait (no IO)
crates/library-native/ the native library: create, open, the library lock, recovery, adding a file
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
