# egl-viewer Android (A1 + A2)

Jetpack Compose phone client over the native `.eglib` core. **No Eagle. No Canvas.**
Desktop Tauri remains the default product. Shares the same C ABI as iOS
(`crates/library-ffi`, D-082 / D-092), wrapped for Kotlin by the `eglcore` module
(D-101 foundation, D-102 browsing).

Stages: **A1** foundation → **A2** browsing (this README) → A3 preview/details → A4
adding → A5 organizing → A6 locks → A7 settings → A8 release.

## What works

| Layer | State |
|---|---|
| `eglcore` | Typed `Library`, view-id APIs (`openView` / `viewRange` / `closeView`), tags, extensions, smart folders, thumbnails, `editItems`, events |
| App foundation (A1) | `AppModel`, Welcome (create, SAF open, recent), open library |
| Browsing (A2) | Places (All / Uncategorized / Untagged / Trash), folder tree + smart folders (locked folders show label, do not open), lazy grid/list by view id, thumbs + palette/type placeholder, sort / size / search debounce / filter sheet, selection + Select All via `{viewId, all, ids}`, saved view per library (`SharedPreferences`) |
| Where libraries live | App-specific storage + SAF; cloud-looking path opens **read-only** (D-021) |
| Strings | English / Russian / Simplified Chinese |
| Native core | Real `libegl_library.so` when built with cargo-ndk; **no mock fallback** |
| Host tests | `./gradlew :eglcore:testDebugUnitTest :app:testDebugUnitTest` (BrowseModel against an in-memory fake; no device) |

**Not yet:** preview/details/undo (A3), gallery add (A4), organize / locks / settings / release.

## Layout

```
android/
  eglcore/                     Kotlin core over JNI (parallel to ios/EglCore)
  app/
    …/app/                     AppModel, OpenLibrary, LibraryLocations
    …/browse/                  BrowseModel, Place, SavedViews, FilterSheet, BrowseScreen
    …/screens/                 WelcomeScreen, LibraryScreen (places → browse)
crates/library-ffi/            C ABI (staticlib iOS, cdylib Android)
```

## Build `libegl_library.so` (arm64)

```bash
rustup target add aarch64-linux-android
cargo install cargo-ndk
cargo ndk -t arm64-v8a -o android/eglcore/src/main/jniLibs \
  build -p library-ffi --release
```

Output: `android/eglcore/src/main/jniLibs/arm64-v8a/libegl_library.so`

## CI (D-084 / D-101 / D-102)

Workflow: [`.github/workflows/mobile.yml`](../.github/workflows/mobile.yml).

| Job | What it runs |
|---|---|
| `library-ffi` | Layout/header sync, `cargo test` / clippy for `library-ffi` |
| `android` | NDK + cargo-ndk `.so`, JVM unit tests (incl. BrowseModel), `assembleDebug` |

## Product constraints

- Native `.eglib` only; never whole item-id lists over JNI — windows by view id.
- Writer lock / sync-folder read-only (`syncFolder` on open, D-021).
- Tags stay case-sensitive.
- UI stays thin; logic lives in Rust.
