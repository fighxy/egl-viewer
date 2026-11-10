#!/usr/bin/env bash
# Version of a GPUI desktop release tag: `gpui-v1.2.3` → `1.2.3`.
# Distinct from `v*` (Windows updater / latest-gpui.json), `ios-v*`, and `android-v*`.
set -euo pipefail

tag="${1:-}"
if [[ "$tag" =~ ^gpui-v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  echo "${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}"
else
  echo "not a GPUI release tag (gpui-vX.Y.Z): '$tag'" >&2
  exit 1
fi
