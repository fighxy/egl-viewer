# egl-viewer Android (experimental draft)

Jetpack Compose phone shell over the native `.eglib` core. **No Eagle. No Canvas.**
Desktop Tauri remains the default product (D-083). Shares the same C ABI as iOS
(`crates/library-ffi`, D-082).

UI flow matches the iOS concept: Welcome → Browse (places / filters / grid) →
Preview → folders/tags sheet. Visual language tracks desktop tokens (D-028).

## Size (why this draft exists)

| Layer | Approx. product LOC |
|---|---|
| iOS SwiftUI shell (reference) | ~1.1k Swift |
| This Compose shell (Kotlin UI + bridge) | ~1.2k Kotlin |
| JNI forwarder | ~200 C++ |
| Gradle / manifest / resources | scaffolding only |

Same four screens and the same C ABI — not a media-pipeline rewrite. If it had
needed multi‑kLOC of Android-only logic, we would have stopped at an estimate.

## What works today

| Layer | State |
|---|---|
| `library-ffi` C ABI | Shared with iOS; host-tested. `cdylib` produces `libegl_library.so` |
| Compose screens | Welcome → Browse → Preview → Edit sheet (desktop-aligned tokens) |
| SAF / document picker | Stubbed — Open/Create use mock paths or in-memory samples |
| JNI | `app/src/main/cpp/egl_jni.cpp` forwards to `egl_*`; CMake optional |
| Linking Rust | Documented below; **not required** to browse UI with mock data |
| On-device media pipeline | Out of scope |

Without the `.so`, the app runs on **mock items** so the four screens can be
reviewed in Android Studio / an emulator.

## Layout

```
android/
  README.md
  settings.gradle.kts / build.gradle.kts
  app/
    build.gradle.kts
    src/main/
      AndroidManifest.xml
      java/app/eglviewer/
        MainActivity.kt
        ui/screens/     Welcome, Browse, Preview, EditSheet
        ui/theme/       Tokens
        bridge/         LibraryBridge, models, NativeFfiSession / EglNative
      cpp/              egl_jni.cpp + CMakeLists.txt + egl_library.h
crates/library-ffi/     C ABI (staticlib iOS, cdylib Android)
```

## Build `libegl_library.so` (arm64)

Needs the Android NDK, Rust, and [`cargo-ndk`](https://crates.io/crates/cargo-ndk)
(or equivalent `aarch64-linux-android` target + linker).

```bash
# from the repo root
rustup target add aarch64-linux-android
cargo install cargo-ndk
# point at your NDK, e.g. $ANDROID_NDK_HOME
cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs \
  build -p library-ffi --release
```

Output: `android/app/src/main/jniLibs/arm64-v8a/libegl_library.so`

Header: `crates/library-ffi/include/egl_library.h` (also under `android/app/src/main/cpp/`).

## Wire FFI in Android Studio

1. Open the `android/` folder in Android Studio (Giraffe+ / AGP 8.7).
2. Drop `libegl_library.so` into `app/src/main/jniLibs/arm64-v8a/` as above.
3. In `app/build.gradle.kts`, uncomment the `ndk { … }` block and the
   `externalNativeBuild { cmake { … } }` block so `egl_jni` links against the
   Rust `.so`.
4. Run on an arm64 device/emulator. `LibraryBridge` loads `libegl_jni.so` and
   opens real `.eglib` folders when present; otherwise it keeps mock data.

## CI (D-084)

Workflow: [`.github/workflows/mobile.yml`](../.github/workflows/mobile.yml) (path-filtered).

| Job | Runner | What it runs |
|---|---|---|
| `library-ffi` | ubuntu | Layout/header sync, `cargo test` / clippy for `library-ffi` (needs `libheif-dev` on the runner — apt, no secrets) |
| `android` | ubuntu | JDK 17 + Android SDK platform 35, `./gradlew :app:assembleDebug` **without NDK** (CMake stays commented; mock FFI) |
| `ios` | `macos-latest` | Unsigned simulator `xcodebuild` for the sibling shell |

No repository secrets. Gradle uses the committed wrapper (`gradlew`) and the
actions Gradle cache. Desktop Windows `CI` is unchanged.

Local without an SDK: `bash scripts/check-mobile-shell.sh` and
`cargo test -p library-ffi`. With Android Studio / SDK:
`cd android && ./gradlew :app:assembleDebug`.

## vs Tauri Android

Tauri can target Android, but this draft is a **native Compose shell** over the
same `library-ffi` C ABI as iOS — not a WebView. Desktop Tauri on Windows stays
the shipping product; mobile shells stay thin and experimental.

## Product constraints

- Native `.eglib` only.
- Writer lock / sync-folder read-only (`syncFolder` on open, D-021).
- Tags stay case-sensitive.
- UI stays thin; logic lives in Rust.
