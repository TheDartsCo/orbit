#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPIMAGE_DIR="$ROOT_DIR/src-tauri/target/release/bundle/appimage"

if [[ -d "$APPIMAGE_DIR" ]]; then
  find "$APPIMAGE_DIR" -maxdepth 1 -type f -name '*.AppImage' -delete
fi

cd "$ROOT_DIR"
npm run tauri -- build --bundles appimage

bash "$ROOT_DIR/scripts/verify-linux-appimage.sh" "$APPIMAGE_DIR"
