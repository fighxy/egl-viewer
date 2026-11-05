#!/usr/bin/env bash
# The Android JNI bridge (android/eglcore/src/main/cpp/egl_jni.cpp) against the real core on
# the Linux host, without an emulator (D-109): builds libegl_library for the host, compiles
# egl_jni.cpp with the JDK's jni.h, and runs android/eglcore/src/hostTest through its own
# class loader, as Android loads app classes. Checks non-ASCII and emoji strings both ways,
# events from the core's threads and the platform decoder callback.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
if [[ -n "${EGL_JNI_WORK:-}" ]]; then
  work="$EGL_JNI_WORK"
  mkdir -p "$work"
else
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
fi

# A debug build of the core: CI's library-ffi job has just compiled it for its tests.
if [[ -z "${EGL_SKIP_CARGO:-}" ]]; then
  cargo build -p library-ffi --manifest-path "$root/Cargo.toml"
fi
core="$root/target/${EGL_CORE_PROFILE:-debug}"
test -f "$core/libegl_library.so"

java_home="${JAVA_HOME:-$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")")}"
cxx="${CXX:-c++}"
"$cxx" -std=c++17 -shared -fPIC -Wall -Wextra -Werror -DEGL_HAS_RUST=1 \
  -I"$root/android/eglcore/src/main/cpp" -I"$java_home/include" -I"$java_home/include/linux" \
  "${EGL_JNI_SOURCE:-$root/android/eglcore/src/main/cpp/egl_jni.cpp}" \
  -L"$core" -legl_library -Wl,-rpath,"$core" -o "$work/libegl_jni.so"

src="$root/android/eglcore/src/hostTest/java"
mkdir -p "$work/app" "$work/runner"
javac -encoding UTF-8 -d "$work/app" "$src"/app/eglviewer/core/*.java
javac -encoding UTF-8 -d "$work/runner" "$src"/runner/Runner.java

# A 1×1 PNG.
python3 - "$work/one.png" <<'PY'
import base64, sys
open(sys.argv[1], "wb").write(base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="))
PY

rm -rf "$work/libraries"
mkdir -p "$work/libraries"
# The JVM encodes file names in the locale's charset: UTF-8, like Android.
LC_ALL=C.UTF-8 LANG=C.UTF-8 java -Dfile.encoding=UTF-8 -Dstdout.encoding=UTF-8 -Djava.library.path="$work" -Xcheck:jni \
  -cp "$work/runner" runner.Runner "$work/app" "$work/libraries" "$work/one.png"
