# egl-viewer iOS (experimental draft)

SwiftUI iPhone shell over the native `.eglib` core. **No Eagle. No Canvas.** Desktop
Tauri remains the default product (D-082, D-085).

Concept screens: Project-store `docs/ios-ui-concept.md`.

## What works today

| Layer | State |
|---|---|
| `crates/library-ffi` C ABI | Over `app-service`, the desktop's own layer (D-092): views by id, events through a callback, edits with undo, trash, folders, tags, smart folders, locked folders, backups, checks, exports. Tested on the host (`cargo test -p library-ffi`), header checked against the exports |
| `ios/EglCore` Swift package | Typed `Library`, `EglError` and `Codable` models over the C ABI; XCTests run on macOS against the real core in CI (`ios-core`) |
| SwiftUI screens | Welcome → Browse → Preview → Folders/tags edit sheet, with desktop-aligned tokens |
| Document picker / bookmarks | **Live:** `fileImporter` for `.eglib` / folders; security-scoped bookmarks in `UserDefaults` (`LibraryAccess`); recents resume access |
| Linking Rust into the app | Optional via `Config/WithFfi.xcconfig` (`EGL_HAS_FFI` + `libegl_library.a`) |
| Thumbnails | FFI WebP bytes into grid / preview when linked; extension placeholder otherwise |
| Media pipeline on device | Out of scope (thumbs only; no full decode pipeline) |

Without `libegl_library.a`, the app runs on **mock items** so the four screens can be
reviewed in Xcode / Simulator. Open/Create still run the pickers; open falls back to mock.

## Layout

```
ios/
  README.md
  EglViewer.xcodeproj/     open this on a Mac
  EglViewer/
    EglViewerApp.swift
    Screens/               Welcome, Browse, Preview, EditSheet
    Theme/Tokens.swift     desktop light tokens (D-028)
    Config/WithFfi.xcconfig  opt-in EGL_HAS_FFI + link flags
    Bridge/
      LibraryBridge.swift  UI façade (mock by default)
      LibraryAccess.swift  bookmarks / recents
      NativeFfiSession.swift  gated by EGL_HAS_FFI
      egl_library.h        copy of crates/library-ffi/include/
  EglCore/                 Swift package over the C ABI (CEglLibrary module, EglCore, tests)
crates/library-ffi/        C ABI over app-service (no Eagle)
```

## Build the Rust static library (Mac)

Needs a Mac with Xcode, Rust, and the system libraries `library-native` / `media`
already expect (including libheif for HEIC, same as desktop D-063).

```bash
# from the repo root
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
cargo build -p library-ffi --release --target aarch64-apple-ios
cargo build -p library-ffi --release --target aarch64-apple-ios-sim
```

Outputs:

- `target/aarch64-apple-ios/release/libegl_library.a`
- `target/aarch64-apple-ios-sim/release/libegl_library.a`

Header: `crates/library-ffi/include/egl_library.h` (also under `ios/EglViewer/Bridge/`).

## Wire FFI into Xcode

1. Open `ios/EglViewer.xcodeproj`.
2. Build `libegl_library.a` for the active destination (device or simulator).
3. Apply `EglViewer/Config/WithFfi.xcconfig` to the target (or copy its settings):
   - `SWIFT_ACTIVE_COMPILATION_CONDITIONS` includes `EGL_HAS_FFI`
   - `LIBRARY_SEARCH_PATHS` / `OTHER_LDFLAGS` link `-legl_library`
4. Header / Swift include paths already point at `EglViewer/Bridge` (modulemap).
5. If the linker asks, keep `-lc++` / Security / SystemConfiguration from the xcconfig
   (rusqlite is bundled; other crates may still need system frameworks).
6. Run on a device or Simulator. `LibraryBridge` then opens real `.eglib` folders
   through the C ABI and shows WebP thumbs.

Linux / CI: run `cargo test -p library-ffi` (or the workspace check). The default
Xcode configuration stays mock so unsigned simulator CI does not need the `.a`.

## EglCore tests (Mac)

```bash
brew install libheif
cargo build -p library-ffi --release
cd ios/EglCore
swift test -Xlinker -L"$PWD/../../target/release" -Xlinker -rpath -Xlinker "$PWD/../../target/release"
```

## Product constraints

- Native `.eglib` only.
- Writer lock and sync-folder read-only rules still apply (`sync_folder` on
  `egl_open`, D-021). iCloud / Files picks open read-only via `syncFolder`.
- Tags stay case-sensitive.
- UI stays thin; logic lives in Rust.
