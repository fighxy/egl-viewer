# egl-viewer iOS (experimental draft)

SwiftUI iPhone shell over the native `.eglib` core. **No Eagle. No Canvas.** Desktop
Tauri remains the default product (D-082).

Concept screens: Project-store `docs/ios-ui-concept.md`.

## What works today

| Layer | State |
|---|---|
| `crates/library-ffi` C ABI | Compiles and is tested on host (`cargo test -p library-ffi`): create/open/close, library info, windowed scope, folders, tags, thumbnail path/bytes, `ItemEdit` |
| SwiftUI screens | Welcome → Browse (places / search / filters / grid / tabs) → Preview → Details/Tags/Folders sheet; desktop-aligned tokens shared with Android (D-085) |
| Document picker / iCloud bookmarks | Stubbed — Open/Create use mock paths or in-memory samples |
| Linking Rust into the app | Documented below; not required to browse the UI with mock data |
| Media pipeline on device | Out of scope for this draft (thumbs by path/bytes are exposed in FFI only) |

Without `libegl_library.a`, the app runs on **mock items** so the four screens can be
reviewed in Xcode / Simulator.

## Layout

```
ios/
  README.md
  EglViewer.xcodeproj/     open this on a Mac
  EglViewer/
    EglViewerApp.swift
    Screens/               Welcome, Browse, Preview, EditSheet
    Theme/Tokens.swift     desktop light tokens (D-028)
    Bridge/
      LibraryBridge.swift  UI façade (mock by default)
      NativeFfiSession.swift  gated by EGL_HAS_FFI
      egl_library.h        copy of crates/library-ffi/include/
crates/library-ffi/        C ABI over library-native (no Eagle)
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
2. Add `libegl_library.a` for the active destination (device or simulator).
3. Set **Header Search Paths** to `$(SRCROOT)/EglViewer/Bridge` (or the crate `include/`).
4. Set **Swift Compiler – Custom Flags → Active Compilation Conditions** to include
   `EGL_HAS_FFI` for Debug/Release.
5. Link system libs SQLite needs if the linker asks (`libsqlite3` is bundled via
   rusqlite’s `bundled` feature; still add `-lc++` / Security / SystemConfiguration
   if Apple’s linker reports missing symbols from transitive crates).
6. Run on a device or Simulator. `LibraryBridge` then opens real `.eglib` folders
   through the C ABI.

## CI (D-084)

Workflow: [`.github/workflows/mobile.yml`](../.github/workflows/mobile.yml) (path-filtered).

| Job | Runner | What it runs |
|---|---|---|
| `library-ffi` | ubuntu | Layout/header sync (`scripts/check-mobile-shell.sh`), `cargo test` / clippy for `library-ffi` |
| `ios` | `macos-latest` | `xcodebuild -scheme EglViewer -sdk iphonesimulator` with `CODE_SIGNING_ALLOWED=NO` (mock FFI, no signing secrets) |
| `android` | ubuntu | `./gradlew :app:assembleDebug` (sibling shell) |

Desktop Windows `CI` is unchanged and still runs `cargo test --workspace` (includes
`library-ffi`). Local Linux can run the layout script + FFI tests; full Xcode needs a Mac.

## Product constraints

- Native `.eglib` only.
- Writer lock and sync-folder read-only rules still apply (`sync_folder` on
  `egl_open`, D-021).
- Tags stay case-sensitive.
- UI stays thin; logic lives in Rust.
