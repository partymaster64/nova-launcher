#!/usr/bin/env bash
set -euo pipefail

echo "=== Nova Launcher – Linux Build ==="

# Systemabhängigkeiten (Ubuntu/Debian – wird in CI verwendet)
if command -v apt-get &>/dev/null; then
    sudo apt-get update -q
    sudo apt-get install -y \
        libwebkit2gtk-4.1-dev \
        libssl-dev \
        libdbus-1-dev \
        libxdo-dev \
        libayatana-appindicator3-dev \
        librsvg2-dev \
        libgtk-3-dev \
        pkg-config
fi

npm ci
npm run tauri build -- --target x86_64-unknown-linux-gnu

# Artefakte sammeln
mkdir -p dist
BUNDLE_DIR="src-tauri/target/x86_64-unknown-linux-gnu/release/bundle"

cp "$BUNDLE_DIR"/appimage/*.AppImage dist/ 2>/dev/null && echo "AppImage kopiert" || true
cp "$BUNDLE_DIR"/deb/*.deb           dist/ 2>/dev/null && echo "DEB kopiert"      || true
cp "src-tauri/target/x86_64-unknown-linux-gnu/release/nova-launcher" \
   "dist/NovaLauncher-Linux-x86_64" 2>/dev/null && echo "Portable kopiert" || true

echo "=== Fertig. Artefakte in dist/ ==="
ls -lh dist/
