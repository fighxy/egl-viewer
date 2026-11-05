# egl-viewer Android (A1–A3)

Jetpack Compose phone client over the native `.eglib` core. **No Eagle. No Canvas.**
Desktop Tauri remains the default product. Shares the same C ABI as iOS
(`crates/library-ffi`, D-082 / D-092), wrapped for Kotlin by the `eglcore` module
(D-101 foundation, D-102 browsing, D-103 preview / details / actions).

Stages: **A1** foundation → **A2** browsing → **A3** preview/details (this README) → A4
adding → A5 organizing → A6 locks → A7 settings → A8 release.

## What works

| Layer | State |
|---|---|
| `eglcore` | Typed `Library`, view-id APIs, tags, extensions, smart folders, thumbs, `Item` / `ItemEdit`, selection tags, undo, delete / empty trash, view ids, original path / bytes |
| App foundation (A1) | `AppModel`, Welcome (create, SAF open, recent), open library |
| Browsing (A2) | Places, folder tree + smart folders (locked folders label-only), lazy grid/list by view id, thumbs, sort / search / filters, selection + Select All via view target, saved view per library |
| Preview / details / actions (A3) | Full-screen pager + zoom + chrome toggle; details sheet (name, rating, tags, folders, notes, link, facts); selection / preview actions (rate, tags, folder, share, trash; Trash restore / delete / empty); undo snackbar; share originals as temp copies (≤100) |
| Where libraries live | App-specific storage + SAF; cloud-looking path opens **read-only** (D-021) |
| Strings | English / Russian / Simplified Chinese |
| Native core | Real `libegl_library.so` when built with cargo-ndk; **no mock fallback** |
| Host tests | `./gradlew :eglcore:testDebugUnitTest :app:testDebugUnitTest` (BrowseModel + details/actions/share against an in-memory fake; no device) |

**Not yet:** gallery add (A4), organize / locks / settings / release.

## Layout

```
android/
  eglcore/                     Kotlin core over JNI (parallel to ios/EglCore)
  app/
    …/app/                     AppModel, OpenLibrary (toast / undo / originals), LibraryLocations
    …/browse/                  BrowseModel, SelectionBar, Place, SavedViews, FilterSheet, BrowseScreen
    …/preview/                 PreviewScreen, ZoomingImage, OriginalCache
    …/details/                 DetailsSheet, ItemDetailsModel, ShareFiles
crates/library-ffi/            C ABI (staticlib iOS, cdylib Android)
```

## Build `libegl_library.so` (arm64)

```bash
rustup target add aarch64-linux-android
cargo install cargo-ndk --version 4.1.2 --locked
cargo ndk -t arm64-v8a -o android/eglcore/src/main/jniLibs \
  build -p library-ffi --release
```

Output: `android/eglcore/src/main/jniLibs/arm64-v8a/libegl_library.so`

## CI (D-084 / D-101 / D-102 / D-103)

Workflow: [`.github/workflows/mobile.yml`](../.github/workflows/mobile.yml).

| Job | What it runs |
|---|---|
| `library-ffi` | Layout/header sync, `cargo test` / clippy for `library-ffi` |
| `android` | NDK + cargo-ndk `.so` (cargo-ndk **4.1.2**), JVM unit tests, `assembleDebug` |

## Product constraints

- Native `.eglib` only; never whole item-id lists over JNI — windows by view id.
- Writer lock / sync-folder read-only (`syncFolder` on open, D-021).
- Tags stay case-sensitive.
- Permanent deletes have no undo; share uses temp copies, never mutates library files.
- UI stays thin; logic lives in Rust.
