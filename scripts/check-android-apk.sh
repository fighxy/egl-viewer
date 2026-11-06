#!/usr/bin/env bash
# Checks the native libraries inside an Android APK (D-115), the way the phone's loader sees
# them: every DT_NEEDED is a bare file name (not a path on the build machine),
# libegl_library.so has its SONAME, every egl library is aligned for 16 KB pages, and no
# stray Cargo artifact (`lib<crate>-<hash>.so`) rides along.
#   scripts/check-android-apk.sh app.apk
set -euo pipefail

apk="${1:?usage: check-android-apk.sh <app.apk>}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
unzip -q -o "$apk" 'lib/*' -d "$work" 2>/dev/null || true

failed=0
fail() {
  echo "FAIL $*" >&2
  failed=1
}

abis=0
for dir in "$work"/lib/*/; do
  abi="$(basename "$dir")"
  [[ -f "$dir/libegl_jni.so" || -f "$dir/libegl_library.so" ]] || continue
  abis=$((abis + 1))
  for name in libegl_jni.so libegl_library.so; do
    [[ -f "$dir/$name" ]] || fail "$abi: $name is missing"
  done
  for so in "$dir"*.so; do
    name="$(basename "$so")"
    if [[ "$name" =~ -[0-9a-f]{16}\.so$ ]]; then
      fail "$abi: stray Cargo artifact $name"
      continue
    fi
    [[ "$name" == libegl_* ]] || continue
    while read -r needed; do
      if [[ "$needed" == */* ]]; then
        fail "$abi/$name needs a path from the build machine: $needed"
      fi
    done < <(readelf -dW "$so" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p')
    # The last column of a LOAD header is its alignment, in hex.
    while read -r align; do
      if (( align < 16384 )); then
        fail "$abi/$name is aligned to $((align)) bytes, not 16 KB"
        break
      fi
    done < <(readelf -lW "$so" | awk '$1 == "LOAD" { print $NF }')
  done
  soname="$(readelf -dW "$dir/libegl_library.so" 2>/dev/null | sed -n 's/.*(SONAME).*\[\(.*\)\]/\1/p')"
  [[ "$soname" == "libegl_library.so" ]] || fail "$abi: libegl_library.so has SONAME '${soname}'"
done

(( abis > 0 )) || fail "no egl native libraries in $apk"
if (( failed )); then
  exit 1
fi
echo "android APK native libraries OK ($abis ABI)"
