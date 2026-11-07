# egl-viewer Android (A1–A8)

Jetpack Compose phone client over the native `.eglib` core. **No Canvas.** Of Eagle it has
only the import: a `Name.library` becomes a new library on the phone (D-116); no live
connection to a running Eagle, no export into Eagle.
Desktop Tauri remains the default product. Shares the same C ABI as iOS
(`crates/library-ffi`, D-082 / D-092), wrapped for Kotlin by the `eglcore` module
(D-101 foundation through D-108 release).

Stages: **A1** foundation → **A2** browsing → **A3** preview/details → **A4** adding →
**A5** organizing → **A6** locks → **A7** settings → **A8** release (this README).

## What works

| Layer | State |
|---|---|
| `eglcore` | Typed `Library`, view-id APIs, tags, extensions, smart folders, thumbs, `Item` / `ItemEdit`, selection tags, undo, delete / empty trash, view ids, original path / bytes, `addFiles` / import progress / duplicate answer, platform ImageDecoder for HEIC, folder / tag / tag-group / smart-folder writes, `smartRules`, locked-folder lock / unlock / recover / relock / label, rename / stats / backups / check / repair / generate previews / export |
| App foundation (A1) | `AppModel`, Welcome (create, SAF open, recent), open library |
| Eagle import (D-116) | Welcome → Import Eagle Library: pick `Name.library` (All files access asked when it lies in shared storage, D-111); name, item count and size; the new library's name and Eagle's trash on or off; background copy with progress and Stop, then a report dialog. `Library.eagleLibraryInfo` / `importEagle` over JNI. The Eagle library is only read |
| Browsing (A2) | Places, folder tree + smart folders (locked folders label-only), lazy grid/list by view id, thumbs, sort / search / filters, selection + Select All via view target, saved view per library |
| Preview / details / actions (A3) | Full-screen pager + pinch/double-tap zoom + chrome toggle; at fit size horizontal swipes page (D-125); details sheet; selection / preview actions; undo snackbar; share originals as temp copies (≤100) |
| Adding (A4) | + menu: Gallery, Files, Folder, Paste; drops onto places; Trash excluded; folder context adds into folder; Folder pick walks the whole SAF tree (nested folders; empty MIME still copied — D-121) into `egl_add_files`; temp `picked-*` copies; SAF grants held until import ends; progress + Stop; duplicate dialog; HEIC via ImageDecoder platform decoder |
| Organizing (A5) | Folder CRUD (create / rename / move / delete); Tags screen with groups, rename / merge / delete; smart folder editor (conditions + v1 rules); Save as Smart Folder from browse |
| Locks + app login (A6) | Locked folders (lock / unlock / recover / Lock All / label); relock on background; App Lock PIN/password (PBKDF2) + BiometricPrompt / Keystore; login overlay + Recents privacy cover |
| Settings (A7) | Library name / storage / rebuild thumbnails; backups + restore; check and repair; export (ZIP, 7z or folder; optional passphrase of 8+ characters: age or AES-256 ZIP, AES-256 7z with hidden names; D-112, D-113) via SAF / share; Decrypt Archive (`.age` → ZIP, Save / Share; D-111); **Theme** (System / Light / Dark, DayNight tokens; D-119); App Lock (wrong-try wait survives a restart), version, language link; no Eagle |
| Release (A8) | Adaptive launcher + Play Store icon; Privacy screen + `PRIVACY.md` + backup exclusions; TalkBack labels for cells and ratings; 10,000-item JVM perf test; tagged APK/AAB by `android-vX.Y.Z` |
| Where libraries live | App-specific storage, or anywhere on the phone after **All files access** (`MANAGE_EXTERNAL_STORAGE`, asked when a library outside app storage is opened; D-111); cloud-looking path opens **read-only** (D-021) |
| Strings | English / Russian / Simplified Chinese, every screen from the catalogs (plurals included); `StringsCatalogTest` keeps the keys in step and catches hardcoded English (D-111) |
| Native core | Real `libegl_library.so` when built with cargo-ndk; **no mock fallback**. If it does not load, Welcome and the error show the loader's reason (D-115) |
| Host tests | `./gradlew :eglcore:testDebugUnitTest :app:testDebugUnitTest` (BrowseModel, details/actions/share, adding / picked copies, smart rules / organize JSON, AppLock / RecoveryKey, settings / export JSON, a11y labels, release assets, 10k perf; no device) |

## Layout

```
android/
  eglcore/                     Kotlin core over JNI (parallel to ios/EglCore)
  app/
    …/app/                     AppModel, OpenLibrary (toast / undo / imports / organize / locks / export)
    …/browse/                  BrowseModel, SelectionBar, Place, SavedViews, FilterSheet, BrowseScreen
    …/preview/                 PreviewScreen, ZoomingImage, OriginalCache
    …/details/                 DetailsSheet, ItemDetailsModel, ShareFiles
    …/adding/                  AddMenu, PickedFiles, import banner / duplicate dialog
    …/organize/                FolderEditor, TagsScreen, SmartFolderEditor, SmartRules
    …/locks/                   AppLock, LoginScreen, LockSheets, AppLockForm
    …/settings/                SettingsScreen, LibraryInfo, Backups, Check, Export, Privacy
    …/a11y/                    StarsText, AccessibilityLabels
    …/theme/                   EglTokens (light/dark), ThemePreference, EglTheme
    res/values + values-night  DayNight theme + window background
    res/mipmap-*               Adaptive launcher icons
    ic_launcher-playstore.png  512×512 Play Store icon
  PRIVACY.md                   Store / data-handling notes
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

## CI (D-084 / D-101…D-108)

Workflow: [`.github/workflows/mobile.yml`](../.github/workflows/mobile.yml).

| Job | What it runs |
|---|---|
| `library-ffi` | Layout/header sync, `cargo test` / clippy for `library-ffi`; the JNI bridge against the real core on the host JVM (`scripts/check-android-jni.sh`: UTF-8 and emoji both ways, events and the platform decoder from the core's threads) |
| `android` | NDK + cargo-ndk `.so` (cargo-ndk **4.1.2**), JVM unit tests, `assembleDebug`, then `scripts/check-android-apk.sh` on the APK (dependencies by name, `libegl_library.so` SONAME, 16 KB pages, no stray Cargo `.so`; D-115); on `android-v*` also `assembleRelease` + `bundleRelease`, checked the same way |
| `android-release` | For a tag `android-vX.Y.Z`: GitHub Release with APK, AAB and `SHA256SUMS` (never latest) |

## Releasing

```bash
git tag android-v0.2.77          # X.Y.Z becomes versionName, the run number versionCode
git push origin android-v0.2.77
```

The workflow refuses a tag that is not `android-vX.Y.Z`
(`scripts/android-release-version.sh`). Notes from `docs/releases/X.Y.Z.md` (shared with the
desktop and iOS releases of the same version) open the release text. Check the download:
`sha256sum -c SHA256SUMS`.

The APK and the AAB are signed with the project's release key (D-109). Android installs an
update only over an app signed with the same key, and removing the app removes the libraries
in its storage, so the key never changes. Create it once and keep a copy outside GitHub:

```bash
keytool -genkeypair -v -keystore egl-viewer-release.jks -alias egl-viewer \
  -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 egl-viewer-release.jks    # the value of ANDROID_KEYSTORE_BASE64
```

Repository secrets (Settings → Secrets and variables → Actions): `ANDROID_KEYSTORE_BASE64`,
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`. A release tag fails
without them, and fails if the APK comes out signed with a debug key. Local `assembleRelease`
without `ANDROID_KEYSTORE_FILE` gives an unsigned APK. For Google Play, enrol in Play App
Signing with this key as the upload key.

## Product constraints

- Native `.eglib` only; never whole item-id lists over JNI — windows by view id.
- Writer lock / sync-folder read-only (`syncFolder` on open, D-021).
- Tags stay case-sensitive.
- Permanent deletes have no undo; share uses temp copies, never mutates library files.
- File bytes never pass through the WebView; Rust reads files by path (temp copies / SAF paths).
- UI stays thin; logic lives in Rust.
- Locked-folder passwords and the app login never leave the device; recovery keys are shown once.
- Export passphrases are never stored; Eagle is not an export destination.
- No tracking / analytics; see [`PRIVACY.md`](PRIVACY.md).
