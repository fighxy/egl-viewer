#!/usr/bin/env bash
# Stages an unsigned macOS .app for the GPUI desktop host and zips it.
# Bundles Homebrew libheif (+ decode deps) into Contents/Frameworks.
# Usage (repo root, after `brew install libheif pkg-config`):
#   bash scripts/package-gpui-macos.sh
#   bash scripts/package-gpui-macos.sh --skip-build
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

skip_build=0
out_dir="${OUT_DIR:-dist/gpui-macos}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) skip_build=1; shift ;;
    --out-dir) out_dir="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "package-gpui-macos.sh must run on macOS" >&2
  exit 1
fi

version="$(
  cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="gpui-desktop"))'
)"
arch="$(uname -m)"
case "$arch" in
  arm64|aarch64) arch_label="aarch64" ;;
  x86_64) arch_label="x86_64" ;;
  *) arch_label="$arch" ;;
esac

if [[ "$skip_build" -eq 0 ]]; then
  echo "Building gpui-desktop (release)…"
  cargo build --locked -p gpui-desktop --release
fi

bin="$repo/target/release/gpui-desktop"
test -x "$bin"

stage="$repo/$out_dir"
rm -rf "$stage"
app="$stage/EglViewer.app"
macos_dir="$app/Contents/MacOS"
fw_dir="$app/Contents/Frameworks"
res_dir="$app/Contents/Resources"
mkdir -p "$macos_dir" "$fw_dir" "$res_dir"

cp "$bin" "$macos_dir/gpui-desktop"
chmod +x "$macos_dir/gpui-desktop"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>gpui-desktop</string>
  <key>CFBundleIdentifier</key>
  <string>com.eglviewer.gpui</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>egl-viewer</string>
  <key>CFBundleDisplayName</key>
  <string>egl-viewer</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${version}</string>
  <key>CFBundleVersion</key>
  <string>${version}</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

# Copy LGPL notices and third-party summary next to the binary (and into Resources).
licenses="$repo/crates/gpui-desktop/assets/THIRD_PARTY_LICENSES.txt"
if [[ -f "$licenses" ]]; then
  cp "$licenses" "$res_dir/THIRD_PARTY_LICENSES.txt"
  cp "$licenses" "$stage/THIRD_PARTY_LICENSES.txt"
fi
printf '%s\n' "libheif — LGPL-3.0-only. See https://github.com/strukturag/libheif" \
  > "$res_dir/NOTICE-libheif.txt"
printf '%s\n' "libde265 — LGPL. See https://github.com/strukturag/libde265" \
  > "$res_dir/NOTICE-libde265.txt"
cp "$res_dir/NOTICE-libheif.txt" "$stage/NOTICE-libheif.txt"
cp "$res_dir/NOTICE-libde265.txt" "$stage/NOTICE-libde265.txt"

# Bundle non-system dylibs that the binary (and those dylibs) need, rewriting
# install names to @executable_path/../Frameworks.
bundle_dylib() {
  local src="$1"
  local base
  base="$(basename "$src")"
  local dest="$fw_dir/$base"
  if [[ -f "$dest" ]]; then
    return 0
  fi
  if [[ ! -f "$src" ]]; then
    echo "missing dylib: $src" >&2
    return 1
  fi
  cp -f "$src" "$dest"
  chmod u+w "$dest"
  install_name_tool -id "@executable_path/../Frameworks/$base" "$dest"
  echo "  bundled $base"
}

is_system_lib() {
  case "$1" in
    /System/*|/usr/lib/*) return 0 ;;
    *) return 1 ;;
  esac
}

# Collect absolute paths of non-system dylibs referenced by a Mach-O file.
collect_deps() {
  local file="$1"
  otool -L "$file" | awk 'NR>1 {print $1}' | while read -r dep; do
    [[ -z "$dep" ]] && continue
    case "$dep" in
      @*|*) ;;
    esac
    if is_system_lib "$dep"; then
      continue
    fi
    if [[ "$dep" == @* ]]; then
      continue
    fi
    if [[ -f "$dep" ]]; then
      echo "$dep"
    fi
  done
}

echo "Bundling Homebrew / local dylibs into Frameworks…"
queue_file="$(mktemp)"
seen_file="$(mktemp)"
cleanup_tmp() { rm -f "$queue_file" "$seen_file"; }
trap cleanup_tmp EXIT

collect_deps "$macos_dir/gpui-desktop" >> "$queue_file"

# Prefer libheif from brew even if the binary already links it.
if command -v brew >/dev/null 2>&1; then
  heif_prefix="$(brew --prefix libheif 2>/dev/null || true)"
  if [[ -n "$heif_prefix" ]]; then
    for cand in "$heif_prefix"/lib/libheif*.dylib; do
      [[ -f "$cand" ]] && echo "$cand" >> "$queue_file"
    done
  fi
  de265_prefix="$(brew --prefix libde265 2>/dev/null || true)"
  if [[ -n "$de265_prefix" ]]; then
    for cand in "$de265_prefix"/lib/libde265*.dylib; do
      [[ -f "$cand" ]] && echo "$cand" >> "$queue_file"
    done
  fi
fi

# BFS over dylib deps (bash 3 compatible: no associative arrays).
queue_idx=1
while true; do
  src="$(sed -n "${queue_idx}p" "$queue_file" || true)"
  [[ -z "$src" ]] && break
  queue_idx=$((queue_idx + 1))
  base="$(basename "$src")"
  if grep -Fxq "$base" "$seen_file" 2>/dev/null; then
    continue
  fi
  echo "$base" >> "$seen_file"
  bundle_dylib "$src"
  collect_deps "$fw_dir/$base" >> "$queue_file"
done

# Point the executable at Frameworks for every bundled dylib it still names by abs path.
while IFS= read -r dep; do
  base="$(basename "$dep")"
  if [[ -f "$fw_dir/$base" ]]; then
    install_name_tool -change "$dep" "@executable_path/../Frameworks/$base" \
      "$macos_dir/gpui-desktop" || true
  fi
done < <(collect_deps "$macos_dir/gpui-desktop")

# Also rewrite absolute deps inside each Frameworks dylib to the bundled copies.
for dylib in "$fw_dir"/*.dylib; do
  [[ -f "$dylib" ]] || continue
  while IFS= read -r dep; do
    base="$(basename "$dep")"
    if [[ -f "$fw_dir/$base" ]]; then
      install_name_tool -change "$dep" "@executable_path/../Frameworks/$base" \
        "$dylib" || true
    fi
  done < <(collect_deps "$dylib")
done

cat > "$stage/README.txt" <<EOF
egl-viewer GPUI desktop (macOS, unsigned)

Open EglViewer.app. HEIC/HEIF decode libraries are in
EglViewer.app/Contents/Frameworks (libheif / libde265, LGPL).

This build is unsigned: Gatekeeper may require right-click → Open the first time.
A signed/notarized build is deferred.
EOF

zip_name="egl-viewer-gpui_${version}_macos-${arch_label}.zip"
zip_path="$repo/dist/$zip_name"
mkdir -p "$repo/dist"
rm -f "$zip_path"
(
  cd "$stage"
  ditto -c -k --sequesterRsrc --keepParent EglViewer.app "$zip_path"
)
# Also keep notices at the zip sibling level for the release job.
cp "$stage/README.txt" "$repo/dist/README-macos.txt" 2>/dev/null || true

echo "Staged $app"
echo "Wrote $zip_path"
ls -lh "$zip_path"
otool -L "$macos_dir/gpui-desktop" | head -40
