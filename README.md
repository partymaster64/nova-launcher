<p align="center">
  <img src="assets/nova-icon.svg" width="120" alt="Nova Launcher Logo" />
</p>

<h1 align="center">Nova Launcher</h1>

<p align="center">
  A fast, beautiful Minecraft launcher built with Tauri 2 and Svelte.
  <br />
  <a href="https://partymaster64.github.io/nova-launcher">Website</a> ·
  <a href="https://github.com/partymaster64/nova-launcher/releases">Download</a> ·
  <a href="https://github.com/partymaster64/nova-launcher/issues">Report Bug</a>
</p>

<p align="center">
  <img src="https://img.shields.io/github/v/release/partymaster64/nova-launcher?style=flat-square&color=a855f7&label=release" alt="Release" />
  <img src="https://img.shields.io/github/actions/workflow/status/partymaster64/nova-launcher/build.yml?style=flat-square&color=a855f7&label=CI" alt="CI" />
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-a855f7?style=flat-square" alt="Platforms" />
  <img src="https://img.shields.io/badge/built%20with-Tauri%202%20%2B%20Svelte-a855f7?style=flat-square" alt="Stack" />
  <img src="https://img.shields.io/github/license/partymaster64/nova-launcher?style=flat-square&color=a855f7" alt="License" />
</p>

---

## Features

| | |
|---|---|
| 🔐 **Microsoft Login** | OAuth2 device-code flow, credentials stored in the OS native keystore |
| 📦 **Instance Management** | Multiple isolated instances with their own versions, mods and saves |
| 🌍 **Modrinth Integration** | Browse and install mods, modpacks and resource packs in-launcher |
| 🧑‍🎤 **Skin Manager** | Live 3D skin preview powered by skinview3d |
| 🎮 **Discord Rich Presence** | Shows your current session in Discord automatically |
| 🔄 **World & Save Sync** | Sync saves, resource packs and screenshots across all instances |
| ☕ **Java Management** | Auto-detects Java and downloads the right JRE per version |
| 🌐 **Multilingual** | German, English, French — more via JSON locale files |
| ⚡ **Native Performance** | Tauri 2 + Rust: minimal memory, instant startup, no Electron |

## Downloads

Grab the latest release from [Releases](https://github.com/partymaster64/nova-launcher/releases).

| Platform | File |
|----------|------|
| Linux    | `.AppImage` (no install) or `.deb` |
| Windows  | `-Setup.exe` or `.msi` |
| macOS    | `.dmg` (Universal Binary — Intel + Apple Silicon) |

## Build from Source

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+

**Linux (Fedora/Nobara):**
```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel libappindicator-gtk3-devel librsvg2-devel
```

**Linux (Debian/Ubuntu):**
```bash
sudo apt-get install libwebkit2gtk-4.1-dev libssl-dev libdbus-1-dev \
  libxdo-dev libayatana-appindicator3-dev librsvg2-dev libgtk-3-dev pkg-config
```

### Dev mode

```bash
git clone https://github.com/partymaster64/nova-launcher.git
cd nova-launcher
npm install
npm run tauri dev
```

### Release builds

```bash
# Linux
./scripts/build_linux.sh

# macOS (Universal Binary: Intel + Apple Silicon)
./scripts/build_mac.sh

# Windows (PowerShell)
.\scripts\build_windows.ps1
```

Output goes to `dist/`.

## Stack

| Layer | Technology |
|-------|-----------|
| UI | Svelte 4 + Vite 5 |
| Backend | Tauri 2 (Rust) |
| HTTP | reqwest 0.12 |
| Auth | Microsoft OAuth2 (Device-Code) |
| Keystore | keyring (native per platform) |
| 3D Skin | skinview3d + three.js |
| Async | tokio 1.x |

## License

MIT — see [LICENSE](LICENSE)
