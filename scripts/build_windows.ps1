# Nova Launcher – Windows Build
# Ausfuehren: .\scripts\build_windows.ps1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "=== Nova Launcher – Windows Build ===" -ForegroundColor Cyan

# Rust-Target sicherstellen
rustup target add x86_64-pc-windows-msvc

npm ci
npm run tauri build -- --target x86_64-pc-windows-msvc

# Artefakte sammeln
New-Item -ItemType Directory -Force -Path dist | Out-Null
$bundleDir = "src-tauri\target\x86_64-pc-windows-msvc\release\bundle"

# NSIS-Installer
$nsis = Get-ChildItem "$bundleDir\nsis\*.exe" -ErrorAction SilentlyContinue
foreach ($f in $nsis) {
    Copy-Item $f.FullName "dist\"
    Write-Host "Installer kopiert: $($f.Name)"
}

# MSI-Installer
$msi = Get-ChildItem "$bundleDir\msi\*.msi" -ErrorAction SilentlyContinue
foreach ($f in $msi) {
    Copy-Item $f.FullName "dist\"
    Write-Host "MSI kopiert: $($f.Name)"
}

# Portable EXE
$exe = "src-tauri\target\x86_64-pc-windows-msvc\release\nova-launcher.exe"
if (Test-Path $exe) {
    Copy-Item $exe "dist\NovaLauncher-Windows-x86_64.exe"
    Write-Host "Portable EXE kopiert"
}

Write-Host "=== Fertig. Artefakte in dist\ ===" -ForegroundColor Green
Get-ChildItem dist\
