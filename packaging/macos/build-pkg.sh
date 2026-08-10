#!/usr/bin/env bash
# Wraps an already-built CoreForge.app into a distributable, unsigned
# CoreForge.pkg. Run on a macOS CI runner - pkgbuild/productbuild are
# macOS-only tools.
#
# Usage: build-pkg.sh <version> <staging-dir> <output-pkg-path>
#
# Expects <staging-dir>/CoreForge.app to already exist (see
# build-app-bundle.sh).
set -euo pipefail

VERSION="${1:?usage: build-pkg.sh <version> <staging-dir> <output-pkg-path>}"
STAGING_DIR="${2:?usage: build-pkg.sh <version> <staging-dir> <output-pkg-path>}"
OUTPUT_PKG="${3:?usage: build-pkg.sh <version> <staging-dir> <output-pkg-path>}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

PAYLOAD_ROOT="$STAGING_DIR/pkg-root"
rm -rf "$PAYLOAD_ROOT"
mkdir -p "$PAYLOAD_ROOT/Applications"
cp -R "$STAGING_DIR/CoreForge.app" "$PAYLOAD_ROOT/Applications/CoreForge.app"

pkgbuild \
    --root "$PAYLOAD_ROOT" \
    --identifier "com.coreverse.coreforge" \
    --version "$VERSION" \
    --install-location "/" \
    --scripts "$REPO_ROOT/packaging/macos" \
    "$OUTPUT_PKG"

echo "Built $OUTPUT_PKG (unsigned - not notarized)"
