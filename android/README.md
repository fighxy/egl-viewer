# egl-viewer Android (A1 foundation)

Jetpack Compose phone client over the native `.eglib` core. **No Eagle. No Canvas.**
Desktop Tauri remains the default product. Shares the same C ABI as iOS
(`crates/library-ffi`, D-082 / D-092), wrapped for Kotlin by the `eglcore` module (D-101).

This is **stage A1** of the Android series (mirror of iOS D-093). Later stages: A2 browsing
→ A3 preview/details → A4 adding → A5 organizing → A6 locks → A7 settings → A8 release.

## What works (A1)

| Layer | State |
|---|---|
| `eglcore` (`android/eglcore`) | Typed `Library`, `EglError`, JSON models; create / open / close, info, counts, folder tree JSON, event-callback stub |
| App foundation | `AppModel`, Welcome (create, SAF open, recent, on-this-phone), open library with places/counts and read-only banner |
| Where libraries live | New ones in **app-specific storage** (`files/libraries`). Others through SAF (`OpenDocumentTree`) with persistable grants; a cloud-looking path opens **read-only** (D-021) |
| Strings | English / Russian / Simplified Chinese (`values`, `values-ru`, `values-zh-rCN`) |
| Native core | Real `libegl_library.so` when built with cargo-ndk; **no mock fallback**. Without the `.so`, create/open fail with a clear error |
| Host tests | `./gradlew :eglcore:testDebugUnitTest :app:testDebugUnitTest` (no device) |

**Not in A1:** item grid / view-id browsing (A2), preview, gallery import, Play Store.

## Layout

```
android/
  README.md
  settings.gradle.kts / build.gradle.kts
  eglcore/                     Kotlin core over JNI (parallel to ios/EglCore)
    src/main/java/…/core/      Library, EglError, Models, EglNative
    src/main/cpp/              egl_jni.cpp + egl_library.h + CMakeLists.txt
    src/main/jniLibs/          libegl_library.so (gitignored; cargo-ndk output)
    src/test/                  JVM unit tests
  app/
    src/main/java/…/app/       AppModel, LibraryLocations, RecentLibraries
    src/main/java/…/screens/   WelcomeScreen, LibraryScreen (places/counts)
    src/main/res/values*/      en / ru / zh-rCN strings
crates/library-ffi/            C ABI (staticlib iOS, cdylib Android)
```

## Build `libegl_library.so` (arm64)

Needs the Android NDK, Rust, and [`cargo-ndk`](https://crates.io/crates/cargo-ndk).
Android builds of the core skip system libheif (same idea as iOS D-086); HEIC via a
platform decoder is A4.

```bash
# from the repo root
rustup target add aarch64-linux-android
cargo install cargo-ndk
# point at your NDK, e.g. $ANDROID_NDK_HOME
cargo ndk -t arm64-v8a -o android/eglcore/src/main/jniLibs \
  build -p library-ffi --release
```

Output: `android/eglcore/src/main/jniLibs/arm64-v8a/libegl_library.so`

Header: `crates/library-ffi/include/egl_library.h` (also under `android/eglcore/src/main/cpp/`).

## Wire FFI in Android Studio

1. Open the `android/` folder in Android Studio (Giraffe+ / AGP 8.7).
2. Build the `.so` into `eglcore/src/main/jniLibs/arm64-v8a/` as above.
3. Gradle detects the `.so` and enables `ndk` + `externalNativeBuild` (CMake) so
   `egl_jni` links against the Rust library. Force with `./gradlew :app:assembleDebug -Pegl.ffi=true`.
4. Run on an arm64 device/emulator. `AppModel` opens real `.eglib` folders when a
   filesystem path is available.

Without the `.so`, `./gradlew :app:assembleDebug` still produces an APK (CMake off);
create/open show that the native core is not linked.

## CI (D-084 / D-101)

Workflow: [`.github/workflows/mobile.yml`](../.github/workflows/mobile.yml) (path-filtered).

| Job | Runner | What it runs |
|---|---|---|
| `library-ffi` | ubuntu | Layout/header sync, `cargo test` / clippy for `library-ffi` |
| `android` | ubuntu | JDK 17, Android SDK + NDK, **cargo-ndk** `libegl_library.so`, JVM unit tests, `assembleDebug` |
| `ios-core` / `ios-app` / … | macOS | Unchanged iOS pipeline |

No repository secrets. Desktop Windows `CI` is unchanged.

Local without an SDK: `bash scripts/check-mobile-shell.sh` and
`cargo test -p library-ffi`. With Android Studio / SDK:
`cd android && ./gradlew :app:assembleDebug :eglcore:testDebugUnitTest :app:testDebugUnitTest`.

## Product constraints

- Native `.eglib` only.
- Writer lock / sync-folder read-only (`syncFolder` on open, D-021).
- Tags stay case-sensitive.
- UI stays thin; logic lives in Rust.
- Series plan: one stage → one PR (A1…A8); next after this is **A2 browsing**.
