#!/usr/bin/env bash
set -euo pipefail

echo "=== Nova Launcher – macOS Universal Build ==="

# Beide Rust-Targets installieren
rustup target add aarch64-apple-darwin x86_64-apple-darwin

npm ci

# Universal Binary: Tauri baut automatisch beide Architekturen und
# kombiniert sie mit lipo zu einem Universal Binary
npm run tauri build -- --target universal-apple-darwin

# Artefakte sammeln
mkdir -p dist
BUNDLE_DIR="src-tauri/target/universal-apple-darwin/release/bundle"

cp "$BUNDLE_DIR"/dmg/*.dmg      dist/ 2>/dev/null && echo "DMG kopiert"     || true
cp "$BUNDLE_DIR"/macos/*.app    dist/ 2>/dev/null && echo ".app kopiert"    || true

# .app als Tarball für einfacheren Upload
if ls dist/*.app &>/dev/null; then
    cd dist
    for app in *.app; do
        tar -czf "${app%.app}.app.tar.gz" "$app"
        rm -rf "$app"
    done
    cd ..
fi

echo "=== Fertig. Artefakte in dist/ ==="
ls -lh dist/
