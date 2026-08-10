#!/usr/bin/env bash
# Assembles CoreForge.app from already-built universal binaries.
#
# Usage: build-app-bundle.sh <version> <staging-dir>
#
# Expects universal (lipo'd) binaries to already exist at:
#   <staging-dir>/coreforge
#   <staging-dir>/coreforge-gui
# (produced by the release workflow via `lipo -create` over the
# x86_64-apple-darwin and aarch64-apple-darwin release builds).
#
# Produces: <staging-dir>/CoreForge.app
set -euo pipefail

VERSION="${1:?usage: build-app-bundle.sh <version> <staging-dir>}"
STAGING_DIR="${2:?usage: build-app-bundle.sh <version> <staging-dir>}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP_DIR="$STAGING_DIR/CoreForge.app"

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

# --- Info.plist ---------------------------------------------------------
sed "s/__VERSION__/$VERSION/g" \
    "$REPO_ROOT/packaging/macos/Info.plist.template" \
    > "$APP_DIR/Contents/Info.plist"

# --- Icon: PNG -> .icns via the macOS-builtin sips/iconutil --------------
ICONSET="$STAGING_DIR/coreforge-emblem.iconset"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
SRC_PNG="$REPO_ROOT/assets/images/coreforge-emblem.png"
for size in 16 32 64 128 256 512; do
    sips -z "$size" "$size" "$SRC_PNG" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" "$SRC_PNG" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP_DIR/Contents/Resources/coreforge-emblem.icns"
rm -rf "$ICONSET"

# --- Binaries -------------------------------------------------------------
cp "$STAGING_DIR/coreforge-gui" "$APP_DIR/Contents/MacOS/coreforge-gui"
cp "$STAGING_DIR/coreforge" "$APP_DIR/Contents/MacOS/coreforge"
chmod +x "$APP_DIR/Contents/MacOS/coreforge-gui" "$APP_DIR/Contents/MacOS/coreforge"

echo "Built $APP_DIR"
