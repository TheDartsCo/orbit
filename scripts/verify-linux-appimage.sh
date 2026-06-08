#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="${1:-$ROOT_DIR/src-tauri/target/release/bundle/appimage}"

if [[ ! -d "$ARTIFACT_DIR" ]]; then
  echo "No AppImage artifact directory found: $ARTIFACT_DIR" >&2
  exit 1
fi

mapfile -t artifacts < <(find "$ARTIFACT_DIR" -maxdepth 1 -type f -name '*.AppImage' | sort)

if [[ "${#artifacts[@]}" -eq 0 ]]; then
  echo "No AppImage artifact found in: $ARTIFACT_DIR" >&2
  exit 1
fi

echo "Created AppImage artifact(s):"
for artifact in "${artifacts[@]}"; do
  echo "  $artifact"
done
