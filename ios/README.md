# egl-viewer for iPhone

A SwiftUI app over the same native `.eglib` core as the desktop: `crates/app-service` through
the C ABI of `crates/library-ffi`, wrapped for Swift by the `EglCore` package (D-091, D-092,
D-093–D-100). **No Eagle.** It has the desktop's features for native libraries, except
auto-update and the YubiKey.

## What works

| Part | State |
|---|---|
| `EglCore` (`ios/EglCore`) | Typed `Library`, `EglError`, `Codable` models; XCTests on macOS against the real core (CI `ios-core`) |
| App foundation | `AppModel` (`@Observable`), Welcome (create, open from Files, recent, On My iPhone), the open library with its counts, read-only banner, errors, light and dark, English / Russian / Simplified Chinese |
| Where libraries live | New ones in **On My iPhone** (`Documents/Libraries`, visible in the Files app). Others through the Files picker, kept with security-scoped bookmarks; a library in iCloud Drive opens **read-only** (D-021, D-093) |
| Browsing (D-094) | Places with counts, the folder tree and smart folders; a lazy grid or a list per place, cells loaded in windows by view id; thumbnails decoded off the main thread and cached; sort, thumbnail size, search, a filter sheet (tags any / all / none, type, rating, shape); selection with Select All over the whole view; the view saved per library |
| Preview and details (D-095) | Full-screen pager over the view with zoom; originals decoded at screen size; details with name, rating, tags (suggestions), folders, notes, link and facts; actions on a selection, from the preview and the long-press menu (rate, tags, folder, share, trash, restore, delete permanently, Empty Trash) with Undo; originals shared under their item names |
| Adding (D-096) | The + menu: Photos (originals, HEIC kept), Files (files and folders, security scopes held during the import), Paste; drops; progress with Stop and a summary; the duplicate question (skip, add anyway, add to the folder, stop) for the batch; HEIC dimensions and thumbnails through ImageIO, lent to the core as its platform decoder |
| Organizing (D-097) | Folders: new, subfolder, edit (name, description, colour), move, delete with or without their items; the tag list by group with rename, merge, delete and tag groups; a tag's items; smart folders: new, subfolder, edit, delete, Save as Smart Folder, and the rule editor (Eagle's rules kept as JSON, unknown rules preserved) |
| Locks and login (D-098) | Locked folders: unlock, recovery key, lock with a label, change password, new recovery key, remove lock, Lock All; every open folder locks again when the app leaves the screen; the app's login (password or PIN, Face ID / Touch ID through the Keychain) with a privacy cover |
| Settings (D-099) | Library name, location and storage, Rebuild Thumbnails; backups (Back Up Now, Restore); Check and Repair; export of the library, folders or tags as ZIP (compression, optional age passphrase of at least 8 characters) or a folder tree, saved to Files or shared; Decrypt Archive for an encrypted export (D-110); App Lock, version and language |
| Release (D-100) | The icon, a privacy manifest and translated privacy strings; VoiceOver descriptions checked by Xcode's accessibility audit; a 10,000-item library timed in a test; a GitHub Release with the unsigned IPA for each tag `ios-vX.Y.Z` |

## Layout

```
ios/
  project.yml              XcodeGen spec; the .xcodeproj is generated, not committed
  EglCore/                 Swift package over the C ABI (CEglLibrary module, EglCore, tests)
  EglViewer/
    App/                   EglViewerApp, AppModel (+ OpenLibrary), LibraryLocations (+ recents),
                           SampleLibrary (debug: generated images for tests)
    Browse/                Place (+ saved view), BrowseModel, BrowseScreen, FilterSheet,
                           ThumbnailCache, SelectionActions (bar, menus, tags, folder picker,
                           toast)
    Preview/               PreviewScreen, ZoomingImage (+ OriginalCache)
    Details/               DetailsSheet, ItemDetailsModel, ShareFiles
    Adding/                AddMenu (+ pickers, progress, duplicate question), PickedFiles
    Organize/              FolderEditor (+ colours, move), TagsScreen (+ groups, merge),
                           SmartFolderEditor, SmartRules
    Locks/                 LockSheets (+ recovery key), AppLock (+ Keychain store),
                           LoginScreen (+ cover window), AppLockForm
    Settings/              SettingsScreen (+ library, backups, check), ExportScreen
    Screens/               WelcomeScreen (+ CreateLibrarySheet), LibraryScreen (places, folders)
    Theme/Tokens.swift     colours that follow light and dark mode
    Resources/             Info.plist (+ InfoPlist.xcstrings), Localizable.xcstrings (en, ru,
                           zh-Hans), PrivacyInfo.xcprivacy, Assets.xcassets (AppIcon)
  EglViewerTests/          unit tests (locations, recents, AppModel, BrowseModel, details,
                           actions, sharing, adding, organizing, locks and upkeep with the real
                           core; 10,000 items; what a release carries)
  EglViewerUITests/        UI tests on a simulator (create, open, close, reopen; Russian;
                           browse, search, filter, select, preview, details, trash and undo
                           on `-sampleLibrary 12`; organizing, the login, Settings; the
                           accessibility audit)
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
- `ios-release`: for a tag `ios-vX.Y.Z`, once the three jobs above pass, a GitHub Release with
  `EglViewer-X.Y.Z.ipa` and `SHA256SUMS` (D-100). Never marked as the latest release, which
  belongs to the desktop updater.

## Releasing

```bash
git tag ios-v0.2.3          # X.Y.Z becomes the app's version, the run number its build
git push origin ios-v0.2.3
```

The workflow refuses a tag that is not `ios-vX.Y.Z` (`scripts/ios-release-version.sh`). The notes of
`docs/releases/X.Y.Z.md`, shared with the desktop release `vX.Y.Z`, open the release text.

## Install

The IPA is not signed. Sign it with your own Apple ID while installing it, with AltStore,
SideStore, Sideloadly, or Xcode (Devices and Simulators). A free Apple ID signs for 7 days; then
the app has to be signed again, and its libraries in On My iPhone stay. Check the download first:
`sha256sum -c SHA256SUMS`.

## Product rules

- Native `.eglib` only; one writer per library (the core's lock); a library in iCloud Drive
  opens read-only.
- Open locked folders lock again when the app leaves the screen; the app's login never leaves
  the Keychain of the device.
- Tags stay case-sensitive.
- The UI stays thin; logic lives in Rust.
