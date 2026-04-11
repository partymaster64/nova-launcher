<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" alt="Nova Launcher Logo" />
</p>

<h1 align="center">Nova Launcher</h1>

<p align="center">
  A fast, modern Minecraft launcher built with Tauri 2 and Svelte.
  <br />
  <a href="https://partymaster64.github.io/nova-launcher">Website</a> ·
  <a href="https://github.com/partymaster64/nova-launcher/releases">Download</a> ·
  <a href="https://github.com/partymaster64/nova-launcher/issues">Report Bug</a>
</p>

<p align="center">
  <img src="https://img.shields.io/github/v/release/partymaster64/nova-launcher?style=flat-square&color=a855f7" alt="Release" />
  <img src="https://img.shields.io/github/actions/workflow/status/partymaster64/nova-launcher/build.yml?style=flat-square&color=a855f7" alt="CI" />
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-a855f7?style=flat-square" alt="Platforms" />
  <img src="https://img.shields.io/badge/built%20with-Tauri%202%20%2B%20Svelte-a855f7?style=flat-square" alt="Stack" />
</p>

---

## Features

- **Microsoft-Login** — OAuth2 Device-Code-Flow, sicher gespeichert im nativen Keystore
- **Instanzverwaltung** — Mehrere Minecraft-Instanzen mit eigenen Versionen und Mods
- **Modrinth-Integration** — Mods, Modpacks und Updates direkt im Launcher durchsuchen
- **Cross-Platform** — Linux, Windows und macOS aus einem Codebase
- **Discord Rich Presence** — Zeigt die aktuelle Spielsitzung in Discord an
- **Skin-Manager** — 3D-Vorschau des eigenen Skins direkt im Launcher
- **Mehrsprachig** — Deutsch, Englisch, Französisch

## Downloads

Aktuelle Releases findest du unter [Releases](https://github.com/partymaster64/nova-launcher/releases).

| Plattform | Datei |
|-----------|-------|
| Linux     | `.AppImage` (kein Install nötig) oder `.deb` |
| Windows   | `-Setup.exe` (Installer) |
| macOS     | `.dmg` |

## Build from Source

### Voraussetzungen

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- Plattformabhängigkeiten (s. u.)

**Linux (Debian/Ubuntu):**
```bash
sudo apt-get install libwebkit2gtk-4.1-dev libssl-dev libdbus-1-dev \
  libxdo-dev libayatana-appindicator3-dev librsvg2-dev libgtk-3-dev pkg-config
```

**Linux (Fedora/Nobara):**
```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel libappindicator-gtk3-devel \
  librsvg2-devel
```

### Entwicklungsmodus

```bash
git clone https://github.com/partymaster64/nova-launcher.git
cd nova-launcher
npm install
npm run tauri dev
```

### Release-Build

```bash
# Linux
./scripts/build_linux.sh

# macOS (Universal Binary)
./scripts/build_mac.sh

# Windows (PowerShell)
.\scripts\build_windows.ps1
```

Fertige Artefakte landen in `dist/`.

## Stack

| Schicht | Technologie |
|---------|-------------|
| UI | Svelte 4 + Vite 5 |
| Backend | Tauri 2 (Rust) |
| HTTP | reqwest 0.12 |
| Auth | Microsoft OAuth2 (Device-Code) |
| Keystore | keyring (native pro Plattform) |
| 3D-Skin | skinview3d + three.js |

## License

MIT — see [LICENSE](LICENSE)
