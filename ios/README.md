# egl-viewer for iPhone

A SwiftUI app over the same native `.eglib` core as the desktop: `crates/app-service` through
the C ABI of `crates/library-ffi`, wrapped for Swift by the `EglCore` package (D-091, D-092,
D-093). **No Eagle.** The plan is the desktop's features in a series of PRs; this one is the
foundation.

## What works

| Part | State |
|---|---|
| `EglCore` (`ios/EglCore`) | Typed `Library`, `EglError`, `Codable` models; XCTests on macOS against the real core (CI `ios-core`) |
| App foundation | `AppModel` (`@Observable`), Welcome (create, open from Files, recent, On My iPhone), the open library with its counts, read-only banner, errors, light and dark, English / Russian / Simplified Chinese |
| Where libraries live | New ones in **On My iPhone** (`Documents/Libraries`, visible in the Files app). Others through the Files picker, kept with security-scoped bookmarks; a library in iCloud Drive opens **read-only** (D-021, D-093) |
| Browse, details, adding, folders, tags, locks, maintenance | The next PRs of the series |

## Layout

```
ios/
  project.yml              XcodeGen spec; the .xcodeproj is generated, not committed
  EglCore/                 Swift package over the C ABI (CEglLibrary module, EglCore, tests)
  EglViewer/
    App/                   EglViewerApp, AppModel (+ OpenLibrary), LibraryLocations (+ recents)
    Screens/               WelcomeScreen (+ CreateLibrarySheet), LibraryScreen
    Theme/Tokens.swift     colours that follow light and dark mode
    Resources/             Info.plist, Localizable.xcstrings (en, ru, zh-Hans)
  EglViewerTests/          unit tests (locations, recents, AppModel with the real core)
  EglViewerUITests/        UI tests on a simulator (create, open, close, reopen; Russian)
```

## Build and run (Mac)

Needs Xcode 16+, Rust (the toolchain in `rust-toolchain.toml`) and
[XcodeGen](https://github.com/yonaskolb/XcodeGen) 2.42.0.

```bash
# from the repo root: the core as a static library for the simulator (or aarch64-apple-ios
# for a device); a dylib next to it would be linked but not embedded, so remove it
rustup target add aarch64-apple-ios-sim aarch64-apple-ios
IPHONEOS_DEPLOYMENT_TARGET=17.0 cargo rustc -p library-ffi --release \
  --target aarch64-apple-ios-sim --crate-type staticlib
rm -f target/aarch64-apple-ios-sim/release/libegl_library.dylib

cd ios
xcodegen generate
open EglViewer.xcodeproj
```

Strings live in `EglViewer/Resources/Localizable.xcstrings`. `scripts/check-ios-strings.py`
(part of `scripts/check-mobile-shell.sh`) checks that every key has the three languages, that
Russian plurals have all four forms, and that the code and the catalog name the same keys.

## EglCore tests (Mac)

```bash
brew install libheif
cargo build -p library-ffi --release
cd ios/EglCore
swift test -Xlinker -L"$PWD/../../target/release" -Xlinker -rpath -Xlinker "$PWD/../../target/release"
```

## CI (`.github/workflows/mobile.yml`)

- `ios-core`: `EglCore` tests on macOS against the core built for the host.
- `ios-app`: the project generated with XcodeGen, the core for arm64 simulators, unit and UI
  tests on an iPhone simulator.
- `ios-ipa`: a device build, unsigned, packed as `EglViewer.ipa`; published as the rolling
  `ios-latest` pre-release from `main` (sideload only, D-086).

## Product rules

- Native `.eglib` only; one writer per library (the core's lock); a library in iCloud Drive
  opens read-only.
- Tags stay case-sensitive.
- The UI stays thin; logic lives in Rust.
