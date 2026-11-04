#!/usr/bin/env bash
# Lightweight validation for the experimental mobile shells (D-084 / D-085).
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
need ios/EglViewer/Resources/InfoPlist.xcstrings
need ios/EglViewer/Screens/WelcomeScreen.swift
need ios/EglViewer/Screens/LibraryScreen.swift
need ios/EglViewer/Resources/Info.plist
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
need ios/EglCore/Package.swift
need ios/EglCore/Sources/CEglLibrary/egl_library.h
need ios/EglCore/Sources/CEglLibrary/module.modulemap
need ios/EglCore/Sources/EglCore/Library.swift
need ios/EglCore/Sources/EglCore/PlatformDecoder.swift
need ios/EglCore/Sources/EglCore/JSONValue.swift
need android/README.md
need android/gradlew
need android/app/build.gradle.kts
need android/app/src/main/java/app/eglviewer/MainActivity.kt
need android/app/src/main/java/app/eglviewer/screens/WelcomeScreen.kt
need android/app/src/main/java/app/eglviewer/screens/BrowseScreen.kt
need android/app/src/main/java/app/eglviewer/screens/PreviewScreen.kt
need android/app/src/main/java/app/eglviewer/screens/EditSheet.kt
need android/app/src/main/java/app/eglviewer/bridge/LibraryAccess.kt
need android/app/src/main/java/app/eglviewer/theme/Tokens.kt
need android/app/src/main/java/app/eglviewer/components/Chrome.kt
need android/app/src/main/cpp/egl_jni.cpp
need android/app/src/main/cpp/egl_library.h

canon="$root/crates/library-ffi/include/egl_library.h"
for copy in ios/EglCore/Sources/CEglLibrary/egl_library.h android/app/src/main/cpp/egl_library.h; do
  if ! cmp -s "$canon" "$root/$copy"; then
    echo "header drift: $copy differs from crates/library-ffi/include/egl_library.h" >&2
    fail=1
  fi
done

# CMake must stay gated on libegl_library.so / -Pegl.ffi so CI mock builds need no NDK.
gradle="$root/android/app/build.gradle.kts"
if ! grep -q 'libegl_library.so' "$gradle"; then
  echo "android/app/build.gradle.kts: expected libegl_library.so gate for native FFI" >&2
  fail=1
fi
if ! grep -q 'enableNativeFfi' "$gradle"; then
  echo "android/app/build.gradle.kts: expected enableNativeFfi guard around externalNativeBuild" >&2
  fail=1
fi
# Reject always-on (ungated) externalNativeBuild at the android { } top level.
if grep -E '^[[:space:]]*externalNativeBuild[[:space:]]*\{' "$gradle" >/dev/null \
  && ! grep -q 'if (enableNativeFfi)' "$gradle"; then
  echo "android/app/build.gradle.kts: ungated externalNativeBuild — wrap with enableNativeFfi" >&2
  fail=1
fi

if [[ "$fail" -ne 0 ]]; then
  echo "mobile shell check failed" >&2
  exit 1
fi
echo "mobile shell layout OK"
