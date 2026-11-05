#!/usr/bin/env bash
# The version of an Android release tag (D-108): `android-v1.2.3` → `1.2.3`. Anything else
# fails, so a mistyped tag publishes nothing.
set -euo pipefail

tag="${1:-}"
if [[ "$tag" =~ ^android-v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  echo "${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}"
else
  echo "not an Android release tag (android-vX.Y.Z): '$tag'" >&2
  exit 1
fi
