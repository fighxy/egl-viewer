#!/usr/bin/env bash
# Lightweight validation for the experimental mobile shells (D-084).
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
need crates/library-ffi/include/egl_library.h
need crates/library-ffi/src/lib.rs
need ios/README.md
need ios/EglViewer.xcodeproj/project.pbxproj
need ios/EglViewer.xcodeproj/xcshareddata/xcschemes/EglViewer.xcscheme
need ios/EglViewer/EglViewerApp.swift
need ios/EglViewer/Screens/WelcomeScreen.swift
need ios/EglViewer/Screens/BrowseScreen.swift
need ios/EglViewer/Screens/PreviewScreen.swift
need ios/EglViewer/Screens/EditSheet.swift
need ios/EglViewer/Bridge/egl_library.h
need android/README.md
need android/gradlew
need android/app/build.gradle.kts
need android/app/src/main/java/app/eglviewer/MainActivity.kt
need android/app/src/main/java/app/eglviewer/screens/WelcomeScreen.kt
need android/app/src/main/java/app/eglviewer/screens/BrowseScreen.kt
need android/app/src/main/java/app/eglviewer/screens/PreviewScreen.kt
need android/app/src/main/java/app/eglviewer/screens/EditSheet.kt
need android/app/src/main/java/app/eglviewer/theme/Tokens.kt
need android/app/src/main/java/app/eglviewer/components/Chrome.kt
need android/app/src/main/cpp/egl_jni.cpp
need android/app/src/main/cpp/egl_library.h

canon="$root/crates/library-ffi/include/egl_library.h"
for copy in ios/EglViewer/Bridge/egl_library.h android/app/src/main/cpp/egl_library.h; do
  if ! cmp -s "$canon" "$root/$copy"; then
    echo "header drift: $copy differs from crates/library-ffi/include/egl_library.h" >&2
    fail=1
  fi
done

# CMake path must stay commented so assembleDebug needs no NDK/.so (D-084).
if grep -E '^[[:space:]]*externalNativeBuild[[:space:]]*\{' "$root/android/app/build.gradle.kts" >/dev/null; then
  echo "android/app/build.gradle.kts: active externalNativeBuild — keep it commented for CI mock builds" >&2
  fail=1
fi

if [[ "$fail" -ne 0 ]]; then
  echo "mobile shell check failed" >&2
  exit 1
fi
echo "mobile shell layout OK"
