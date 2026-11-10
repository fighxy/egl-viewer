#!/usr/bin/env bash
# Stages a portable Linux x86_64 layout for the GPUI desktop host and tars it.
# Ships the release binary, a small launcher, bundled libheif/libde265, licenses.
# Wayland/X11/Vulkan stay as system libraries (documented in README.txt).
# Usage (repo root):
#   bash scripts/package-gpui-linux.sh
#   bash scripts/package-gpui-linux.sh --skip-build
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

skip_build=0
out_dir="${OUT_DIR:-dist/gpui-linux}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) skip_build=1; shift ;;
    --out-dir) out_dir="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "package-gpui-linux.sh must run on Linux" >&2
  exit 1
fi

version="$(
  cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="gpui-desktop"))'
)"

if [[ "$skip_build" -eq 0 ]]; then
  echo "Building gpui-desktop (release)…"
  # $ORIGIN/lib so the bundled heif/de265 resolve next to the binary.
  export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,\$ORIGIN/lib"
  cargo build --locked -p gpui-desktop --release
fi

bin="$repo/target/release/gpui-desktop"
test -x "$bin"

stage_root="$repo/$out_dir"
folder_name="egl-viewer-gpui_${version}_linux-x86_64"
stage="$stage_root/$folder_name"
rm -rf "$stage_root"
mkdir -p "$stage/bin" "$stage/lib"

cp "$bin" "$stage/bin/gpui-desktop"
chmod +x "$stage/bin/gpui-desktop"

# Launcher: ensure bundled libs win even if RPATH was stripped.
cat > "$stage/egl-viewer" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
export LD_LIBRARY_PATH="$here/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$here/bin/gpui-desktop" "$@"
EOF
chmod +x "$stage/egl-viewer"

copy_lib() {
  local src="$1"
  [[ -e "$src" ]] || return 0
  local real
  real="$(readlink -f "$src")"
  local base
  base="$(basename "$real")"
  cp -L "$real" "$stage/lib/$base"
  # Keep the SONAME symlink name the binary looks up.
  local link_name
  link_name="$(basename "$src")"
  if [[ "$link_name" != "$base" ]]; then
    ln -sfn "$base" "$stage/lib/$link_name"
  fi
  # Also keep short .so.N names when present as separate links.
  echo "  bundled $link_name -> $base"
}

echo "Bundling libheif / libde265…"
# Resolve via ldd first, then fall back to known multiarch paths.
mapfile -t heif_lines < <(ldd "$stage/bin/gpui-desktop" | awk '/libheif\.so|libde265\.so|libsharpyuv\.so/ {print $3}')
for path in "${heif_lines[@]:-}"; do
  [[ -n "${path:-}" && "$path" != "not" ]] || continue
  copy_lib "$path"
done

for pattern in \
  /usr/lib/x86_64-linux-gnu/libheif.so* \
  /usr/lib/x86_64-linux-gnu/libde265.so* \
  /usr/lib/x86_64-linux-gnu/libsharpyuv.so* \
  /usr/local/lib/libheif.so* \
  /usr/local/lib/libde265.so*
do
  for path in $pattern; do
    [[ -e "$path" ]] || continue
    copy_lib "$path"
  done
done

# Plugin directory for libheif (libde265 decoder), when present.
plugin_src=""
for cand in \
  /usr/lib/x86_64-linux-gnu/libheif \
  /usr/lib/libheif \
  /usr/local/lib/libheif
do
  if [[ -d "$cand" ]]; then
    plugin_src="$cand"
    break
  fi
done
if [[ -n "$plugin_src" ]]; then
  mkdir -p "$stage/lib/libheif"
  cp -a "$plugin_src"/. "$stage/lib/libheif/" || true
  echo "  bundled libheif plugins from $plugin_src"
fi

licenses="$repo/crates/gpui-desktop/assets/THIRD_PARTY_LICENSES.txt"
[[ -f "$licenses" ]] && cp "$licenses" "$stage/THIRD_PARTY_LICENSES.txt"
printf '%s\n' "libheif — LGPL-3.0-only. See https://github.com/strukturag/libheif" \
  > "$stage/NOTICE-libheif.txt"
printf '%s\n' "libde265 — LGPL. See https://github.com/strukturag/libde265" \
  > "$stage/NOTICE-libde265.txt"

cat > "$stage/README.txt" <<EOF
egl-viewer GPUI desktop (Linux x86_64)

Run ./egl-viewer (or bin/gpui-desktop with lib/ on LD_LIBRARY_PATH).

Bundled next to the binary (LGPL):
  lib/libheif*, lib/libde265* (and libheif plugins when available)

Still needed from the OS (typical Debian/Ubuntu packages):
  libwayland-client0 libxkbcommon0 libx11-6 libxcb1 libvulkan1
  libfontconfig1 libfreetype6 libpcsclite1

This is a portable tarball, not an AppImage. Signing is deferred.
EOF

mkdir -p "$repo/dist"
tar_name="egl-viewer-gpui_${version}_linux-x86_64.tar.gz"
tar_path="$repo/dist/$tar_name"
rm -f "$tar_path"
tar -C "$stage_root" -czf "$tar_path" "$folder_name"

echo "Staged $stage"
echo "Wrote $tar_path"
ls -lh "$tar_path"
ldd "$stage/bin/gpui-desktop" | head -40 || true
