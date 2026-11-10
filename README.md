<img src="src-tauri/icons/128x128.png" alt="" width="56">

# egl-viewer

A standalone app for collections of images and design references, with its own `.eglib` libraries.

[![Release](https://img.shields.io/github/v/release/fighxy/egl-viewer?label=release)](https://github.com/fighxy/egl-viewer/releases/latest)
[![License](https://img.shields.io/badge/license-unlicensed-lightgrey)](#license)
[![CI](https://github.com/fighxy/egl-viewer/actions/workflows/ci.yml/badge.svg)](https://github.com/fighxy/egl-viewer/actions/workflows/ci.yml)

## Features

- A library is a folder (`Name.eglib`) with folders, tags, ratings, notes, search, filters and smart folders.
- Import an Eagle library. On the desktop, connect to a running Eagle on the same PC or the network, and export into Eagle.
- Locked folders, trash, backups, and export of the original files.
- The same `.eglib` libraries on the desktop, iPhone and Android.
- English, Russian and Simplified Chinese.

What the app does: [`docs/spec.md`](docs/spec.md). How it is built: [`docs/architecture.md`](docs/architecture.md). The library format: [`docs/library-format.md`](docs/library-format.md).

## Current state

egl-viewer is a desktop app and two phone apps. `main` is the stable line: the Tauri desktop app (latest release 0.2.77) together with the iOS and Android apps. A native Rust desktop shell built with GPUI is in early alpha on [`feat/gpui-desktop`](https://github.com/fighxy/egl-viewer/tree/feat/gpui-desktop).

| App | Line | Maturity |
| --- | --- | --- |
| Desktop | `main` | Stable. Tauri 2, a Rust core and a React UI. Release 0.2.77 for Windows, macOS and Linux. |
| Desktop | [`feat/gpui-desktop`](https://github.com/fighxy/egl-viewer/tree/feat/gpui-desktop) | Early alpha. Native GPUI shell. No release builds. |
| iOS | `main` | Alpha. SwiftUI over [`crates/library-ffi`](crates/library-ffi). Unsigned IPA. |
| Android | `main` | Very early alpha. Jetpack Compose over the same crate. |

Recent work on `feat/gpui-desktop`: Eagle-like chrome, a folder picker, item shortcuts, a rating filter, background preview decode, and drag-and-drop. That branch drops Tauri and the web UI; GPUI is the desktop shell there.

On `main`, the phone apps are SwiftUI and Jetpack Compose over `crates/library-ffi`, on the same `.eglib` libraries. Eagle import into a new library on iPhone and Android was added in 0.2.55. Release 0.2.77 adds an Android theme (system, light or dark) and folder import that walks nested folders, and on iPhone a borderless justified grid plus a preview that shows the image and swipes between items. Both phone UIs are English, Russian and Simplified Chinese. See [`ios/README.md`](ios/README.md) and [`android/README.md`](android/README.md).

## Downloads

Desktop buttons install the stable Tauri app, **0.2.77**, which is [the latest release](https://github.com/fighxy/egl-viewer/releases/latest). iOS and Android are alpha builds of the same version, on their own tags.

[![Windows x64](https://img.shields.io/badge/Windows-x64_setup-444444?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/fighxy/egl-viewer/releases/download/v0.2.77/egl-viewer_0.2.77_x64-setup.exe)
[![macOS Apple Silicon](https://img.shields.io/badge/macOS-Apple_Silicon-444444?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/fighxy/egl-viewer/releases/download/v0.2.77/egl-viewer_0.2.77_aarch64.dmg)
[![Linux x64](https://img.shields.io/badge/Linux-x64_AppImage-444444?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/fighxy/egl-viewer/releases/download/v0.2.77/egl-viewer_0.2.77_amd64.AppImage)

[![Android, very early alpha](https://img.shields.io/badge/Android-very_early_alpha-444444?style=for-the-badge&logo=android&logoColor=white)](https://github.com/fighxy/egl-viewer/releases/download/android-v0.2.77/EglViewer-0.2.77.apk)
[![iOS alpha, unsigned IPA](https://img.shields.io/badge/iOS-alpha_%C2%B7_unsigned_IPA-444444?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/fighxy/egl-viewer/releases/download/ios-v0.2.77/EglViewer-0.2.77.ipa)

| Platform | File | Release |
| --- | --- | --- |
| Windows x64 | `egl-viewer_0.2.77_x64-setup.exe` | [v0.2.77](https://github.com/fighxy/egl-viewer/releases/latest) |
| macOS (Apple Silicon) | `egl-viewer_0.2.77_aarch64.dmg` | [v0.2.77](https://github.com/fighxy/egl-viewer/releases/latest) |
| Linux x64 | `egl-viewer_0.2.77_amd64.AppImage` | [v0.2.77](https://github.com/fighxy/egl-viewer/releases/latest) |
| Android (very early alpha) | `EglViewer-0.2.77.apk` (`.aab` on the same release) | [android-v0.2.77](https://github.com/fighxy/egl-viewer/releases/tag/android-v0.2.77) |
| iOS (alpha, unsigned IPA) | `EglViewer-0.2.77.ipa` | [ios-v0.2.77](https://github.com/fighxy/egl-viewer/releases/tag/ios-v0.2.77) |

The macOS disk image is Apple Silicon only. Sideloading the IPA is described in [`ios/README.md`](ios/README.md).

## Build from source

These commands build the Tauri desktop app on `main`. Use Rust from `rust-toolchain.toml`, Node.js 24 and pnpm 12.

```bash
pnpm install
pnpm dev
```

`pnpm build` on Windows produces the NSIS installer. `pnpm check` runs formatting, Clippy, Rust tests, Biome, TypeScript and the UI tests.

HEIC previews need libheif. On Windows, install `libheif[core,libde265]:x64-windows` with vcpkg, set `VCPKG_ROOT` and `VCPKGRS_DYNAMIC=1`, and use Visual Studio Build Tools 2022 (Desktop development with C++) plus the WebView2 Runtime. On macOS, `brew install libheif`. On Linux the release build is an AppImage; the package list is the `appimage` job in [`.github/workflows/ci.yml`](.github/workflows/ci.yml).

iPhone and Android: [`ios/README.md`](ios/README.md), [`android/README.md`](android/README.md).

## License

No license is granted. The workspace license in `Cargo.toml` is `UNLICENSED`. Dependencies keep their own licenses.
