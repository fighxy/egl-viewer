#!/usr/bin/env bash
# Lightweight validation for the mobile shells (D-084 / D-101).
# Runs on Linux: layout + header copies stay in sync with library-ffi.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
fail=0

need() {
  if [[ ! -e "$root/$1" ]]; then
    echo "missing: $1" >&2
    fail=1
  fi
}

echo "Checking iOS / Android shell layout…"
python3 "$root/scripts/check-ios-strings.py" || fail=1
need crates/library-ffi/include/egl_library.h
need crates/library-ffi/src/lib.rs
need ios/README.md
need ios/project.yml
need ios/EglViewer/App/EglViewerApp.swift
need ios/EglViewer/App/AppModel.swift
need ios/EglViewer/App/LibraryLocations.swift
need ios/EglViewer/App/SampleLibrary.swift
need ios/EglViewer/Browse/Place.swift
need ios/EglViewer/Browse/BrowseModel.swift
need ios/EglViewer/Browse/BrowseScreen.swift
need ios/EglViewer/Browse/FilterSheet.swift
need ios/EglViewer/Browse/ThumbnailCache.swift
need ios/EglViewer/Browse/SelectionActions.swift
need ios/EglViewer/Preview/PreviewScreen.swift
need ios/EglViewer/Preview/ZoomingImage.swift
need ios/EglViewer/Details/DetailsSheet.swift
need ios/EglViewer/Details/ItemDetailsModel.swift
need ios/EglViewer/Details/ShareFiles.swift
need ios/EglViewer/Adding/AddMenu.swift
need ios/EglViewer/Adding/PickedFiles.swift
need ios/EglViewer/Organize/FolderEditor.swift
need ios/EglViewer/Organize/SmartFolderEditor.swift
need ios/EglViewer/Organize/SmartRules.swift
need ios/EglViewer/Organize/TagsScreen.swift
need ios/EglViewer/Locks/AppLock.swift
need ios/EglViewer/Locks/AppLockForm.swift
need ios/EglViewer/Locks/LockSheets.swift
need ios/EglViewer/Locks/LoginScreen.swift
need ios/EglViewer/Settings/SettingsScreen.swift
need ios/EglViewer/Settings/ExportScreen.swift
need ios/EglViewer/Settings/DecryptArchiveScreen.swift
need ios/EglViewer/Resources/InfoPlist.xcstrings
need ios/EglViewer/Screens/WelcomeScreen.swift
need ios/EglViewer/Screens/LibraryScreen.swift
need ios/EglViewer/Resources/Info.plist
need ios/EglViewer/Resources/PrivacyInfo.xcprivacy
need ios/EglViewer/Resources/Assets.xcassets/AppIcon.appiconset/AppIcon.png
need ios/EglViewer/Resources/Assets.xcassets/AppIcon.appiconset/Contents.json
need ios/EglViewerTests/PerformanceTests.swift
need ios/EglViewerTests/ReleaseTests.swift
need ios/EglViewerUITests/AccessibilityUITests.swift
need scripts/ios-release-version.sh
need ios/EglViewer/Resources/Localizable.xcstrings
need ios/EglViewerTests/LibraryLocationsTests.swift
need ios/EglViewerUITests/WelcomeUITests.swift
need ios/EglViewerTests/BrowseModelTests.swift
need ios/EglViewerUITests/BrowseUITests.swift
need ios/EglViewerTests/DetailsAndActionsTests.swift
need ios/EglViewerTests/AddingTests.swift
need ios/EglViewerTests/OrganizeTests.swift
need ios/EglViewerUITests/OrganizeUITests.swift
need ios/EglViewerTests/LocksTests.swift
need ios/EglViewerUITests/LocksUITests.swift
need ios/EglViewerTests/MaintenanceTests.swift
need ios/EglViewerUITests/SettingsUITests.swift
need ios/EglViewerUITests/PreviewUITests.swift
need ios/EglViewerUITests/UITestSupport.swift
need ios/EglCore/Package.swift
need ios/EglCore/Sources/CEglLibrary/egl_library.h
need ios/EglCore/Sources/CEglLibrary/module.modulemap
need ios/EglCore/Sources/EglCore/Library.swift
need ios/EglCore/Sources/EglCore/PlatformDecoder.swift
need ios/EglCore/Sources/EglCore/JSONValue.swift

# Android A1 foundation (D-101): eglcore + Welcome / places shell.
need android/README.md
need android/gradlew
need android/settings.gradle.kts
need android/app/build.gradle.kts
need android/eglcore/build.gradle.kts
need android/eglcore/src/main/cpp/egl_jni.cpp
need android/eglcore/src/main/cpp/egl_library.h
need android/eglcore/src/main/cpp/CMakeLists.txt
need android/eglcore/src/main/java/app/eglviewer/core/Library.kt
need android/eglcore/src/main/java/app/eglviewer/core/EglError.kt
need android/eglcore/src/main/java/app/eglviewer/core/Models.kt
need android/eglcore/src/main/java/app/eglviewer/core/EglNative.kt
need android/eglcore/src/main/java/app/eglviewer/core/ItemModels.kt
need android/eglcore/src/test/java/app/eglviewer/core/EglCoreUnitTest.kt
need android/eglcore/src/test/java/app/eglviewer/core/ItemModelsTest.kt
need android/app/src/main/java/app/eglviewer/MainActivity.kt
need android/app/src/main/java/app/eglviewer/app/AppModel.kt
need android/app/src/main/java/app/eglviewer/app/LibraryLocations.kt
need android/app/src/main/java/app/eglviewer/app/OpenLibrary.kt
need android/app/src/main/java/app/eglviewer/browse/BrowseModel.kt
need android/app/src/main/java/app/eglviewer/browse/BrowseScreen.kt
need android/app/src/main/java/app/eglviewer/browse/SelectionBar.kt
need android/app/src/main/java/app/eglviewer/preview/PreviewScreen.kt
need android/app/src/main/java/app/eglviewer/preview/ZoomingImage.kt
need android/app/src/main/java/app/eglviewer/details/DetailsSheet.kt
need android/app/src/main/java/app/eglviewer/details/ItemDetailsModel.kt
need android/app/src/main/java/app/eglviewer/details/ShareFiles.kt
need android/app/src/main/java/app/eglviewer/screens/WelcomeScreen.kt
need android/app/src/main/java/app/eglviewer/screens/LibraryScreen.kt
need android/app/src/main/java/app/eglviewer/theme/Tokens.kt
need android/app/src/main/java/app/eglviewer/components/Chrome.kt
need android/app/src/main/java/app/eglviewer/components/UndoSnackbar.kt
need android/app/src/main/res/values/strings.xml
need android/app/src/main/res/values-ru/strings.xml
need android/app/src/main/res/values-zh-rCN/strings.xml
need android/app/src/test/java/app/eglviewer/app/LibraryLocationsTest.kt
need android/app/src/test/java/app/eglviewer/browse/DetailsAndActionsTest.kt
need android/app/src/test/java/app/eglviewer/browse/PerformanceTest.kt
need android/app/src/test/java/app/eglviewer/release/ReleaseTest.kt
need android/app/src/test/java/app/eglviewer/a11y/AccessibilityLabelsTest.kt
need android/app/src/main/java/app/eglviewer/a11y/AccessibilityLabels.kt
need android/app/src/main/java/app/eglviewer/a11y/StarsText.kt
need android/app/src/main/java/app/eglviewer/settings/PrivacyScreen.kt
need android/PRIVACY.md
need android/app/src/main/res/xml/data_extraction_rules.xml
need android/app/src/main/res/xml/backup_rules.xml
need android/app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml
need android/app/src/main/res/mipmap-xxxhdpi/ic_launcher.png
need android/app/ic_launcher-playstore.png
need scripts/android-release-version.sh
need scripts/check-android-jni.sh
need android/eglcore/src/hostTest/java/app/eglviewer/core/HostJniTest.java
need android/eglcore/src/hostTest/java/runner/Runner.java

# Release tags (D-100): ios-vX.Y.Z gives X.Y.Z; anything else publishes nothing.
version="$root/scripts/ios-release-version.sh"
if [[ "$(bash "$version" ios-v1.20.3)" != "1.20.3" ]]; then
  echo "scripts/ios-release-version.sh: ios-v1.20.3 should give 1.20.3" >&2
  fail=1
fi
for bad in v1.2.3 ios-v1.2 ios-v01.2.3 ios-v1.2.3-beta ios-latest ""; do
  if bash "$version" "$bad" >/dev/null 2>&1; then
    echo "scripts/ios-release-version.sh: '$bad' should be refused" >&2
    fail=1
  fi
done
# Release tags (D-108): android-vX.Y.Z gives X.Y.Z; anything else publishes nothing.
android_version="$root/scripts/android-release-version.sh"
if [[ "$(bash "$android_version" android-v1.20.3)" != "1.20.3" ]]; then
  echo "scripts/android-release-version.sh: android-v1.20.3 should give 1.20.3" >&2
  fail=1
fi
for bad in v1.2.3 android-v1.2 android-v01.2.3 android-v1.2.3-beta android-latest ios-v1.2.3 ""; do
  if bash "$android_version" "$bad" >/dev/null 2>&1; then
    echo "scripts/android-release-version.sh: '$bad' should be refused" >&2
    fail=1
  fi
done
# The iPhone icon: one 1024 px PNG without transparency (iOS rounds the corners itself).
python3 - "$root/ios/EglViewer/Resources/Assets.xcassets/AppIcon.appiconset/AppIcon.png" <<'PY' || fail=1
import struct, sys
data = open(sys.argv[1], "rb").read(33)
width, height, depth, colour = struct.unpack(">IIBB", data[16:26])
if data[:8] != b"\x89PNG\r\n\x1a\n" or (width, height) != (1024, 1024) or colour not in (0, 2):
    sys.exit(f"AppIcon.png: want a 1024×1024 PNG without alpha, got {width}×{height}, colour type {colour}")
PY
# The Android Play Store icon: 512×512 PNG (RGB or RGBA).
python3 - "$root/android/app/ic_launcher-playstore.png" <<'PY' || fail=1
import struct, sys
data = open(sys.argv[1], "rb").read(33)
width, height, depth, colour = struct.unpack(">IIBB", data[16:26])
if data[:8] != b"\x89PNG\r\n\x1a\n" or (width, height) != (512, 512) or colour not in (0, 2, 6):
    sys.exit(f"ic_launcher-playstore.png: want a 512×512 PNG, got {width}×{height}, colour type {colour}")
PY

canon="$root/crates/library-ffi/include/egl_library.h"
for copy in ios/EglCore/Sources/CEglLibrary/egl_library.h android/eglcore/src/main/cpp/egl_library.h; do
  if ! cmp -s "$canon" "$root/$copy"; then
    echo "header drift: $copy differs from crates/library-ffi/include/egl_library.h" >&2
    fail=1
  fi
done

# CMake stays gated on libegl_library.so / -Pegl.ffi so a Kotlin-only assemble needs no NDK;
# Mobile CI builds the .so with cargo-ndk first (D-101).
gradle="$root/android/eglcore/build.gradle.kts"
if ! grep -q 'libegl_library.so' "$gradle"; then
  echo "android/eglcore/build.gradle.kts: expected libegl_library.so gate for native FFI" >&2
  fail=1
fi
if ! grep -q 'enableNativeFfi' "$gradle"; then
  echo "android/eglcore/build.gradle.kts: expected enableNativeFfi guard around externalNativeBuild" >&2
  fail=1
fi
if grep -E '^[[:space:]]*externalNativeBuild[[:space:]]*\{' "$gradle" >/dev/null \
  && ! grep -q 'if (enableNativeFfi)' "$gradle"; then
  echo "android/eglcore/build.gradle.kts: ungated externalNativeBuild — wrap with enableNativeFfi" >&2
  fail=1
fi

# No mock-as-default: the draft MockLibraryStore must stay gone.
if grep -R --include='*.kt' -n 'MockLibraryStore\|usingMockData' "$root/android" >/dev/null 2>&1; then
  echo "android: mock library fallback must stay removed (D-101)" >&2
  fail=1
fi

if [[ "$fail" -ne 0 ]]; then
  echo "mobile shell check failed" >&2
  exit 1
fi
echo "mobile shell layout OK"
