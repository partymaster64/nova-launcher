use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

// ─── Manifest ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: VersionType,
    pub url: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    #[serde(rename = "old_alpha")]
    OldAlpha,
    #[serde(rename = "old_beta")]
    OldBeta,
}

impl VersionType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Release => "Release",
            Self::Snapshot => "Snapshot",
            Self::OldAlpha => "Alpha",
            Self::OldBeta => "Beta",
        }
    }
}

fn manifest_cache_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("nova-launcher").join("manifest_cache.json"))
}

pub fn save_manifest_to_disk(manifest: &VersionManifest) {
    if let Some(path) = manifest_cache_path() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Ok(json) = serde_json::to_string(manifest) {
            std::fs::write(path, json).ok();
        }
    }
}

pub fn load_manifest_from_disk() -> Option<VersionManifest> {
    let path = manifest_cache_path()?;
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

pub async fn fetch_manifest(client: &reqwest::Client) -> Result<VersionManifest> {
    let manifest: VersionManifest = client
        .get(MANIFEST_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(manifest)
}

// ─── Version-Metadaten ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionMeta {
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "minecraftArguments", default)]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "arguments", default)]
    pub arguments: Option<ArgumentsBlock>,
    pub downloads: VersionDownloads,
    pub libraries: Vec<Library>,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndex,
    pub assets: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ArgumentsBlock {
    #[serde(default)]
    pub game: Vec<serde_json::Value>,
    #[serde(default)]
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionDownloads {
    pub client: Download,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Download {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<LibraryRule>,
    /// OS → Classifier-Schlüssel (z.B. "linux" → "natives-linux")
    #[serde(default)]
    pub natives: HashMap<String, String>,
    #[serde(default)]
    pub extract: Option<ExtractRules>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibraryDownloads {
    pub artifact: Option<LibraryArtifact>,
    /// Natives-Classifiers (Schlüssel = Classifier-Name)
    #[serde(default)]
    pub classifiers: HashMap<String, LibraryArtifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibraryArtifact {
    pub url: String,
    pub path: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibraryRule {
    pub action: String,
    pub os: Option<OsRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OsRule {
    pub name: Option<String>,
}

impl Library {
    /// Prüft ob die Library für das aktuelle OS gilt
    pub fn applies_to_current_os(&self) -> bool {
        if self.rules.is_empty() {
            return true;
        }
        let current_os = current_os_name();
        let mut allow = false;
        for rule in &self.rules {
            let matches_os = rule
                .os
                .as_ref()
                .and_then(|o| o.name.as_deref())
                .map(|name| name == current_os)
                .unwrap_or(true);
            if matches_os {
                allow = rule.action == "allow";
            }
        }
        allow
    }

    /// Gibt das Natives-Artefakt für das aktuelle OS zurück (falls vorhanden)
    pub fn native_artifact(&self) -> Option<&LibraryArtifact> {
        let os_key = current_os_name();
        let classifier_key = self.natives.get(os_key)?;
        // ${arch} Platzhalter auflösen (z.B. "natives-linux-${arch}")
        let arch = if cfg!(target_arch = "x86_64") { "64" } else { "32" };
        let classifier_key = classifier_key.replace("${arch}", arch);
        self.downloads.as_ref()?.classifiers.get(&classifier_key)
    }

    /// Gibt die beim Natives-Extrahieren auszuschließenden Pfade zurück
    pub fn extract_excludes(&self) -> Vec<String> {
        self.extract
            .as_ref()
            .map(|e| e.exclude.clone())
            .unwrap_or_default()
    }
}

pub fn current_os_name() -> &'static str {
    #[cfg(target_os = "linux")]
    { "linux" }
    #[cfg(target_os = "macos")]
    { "osx" }
    #[cfg(target_os = "windows")]
    { "windows" }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    { "unknown" }
}

fn version_meta_cache_path(version_id: &str) -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("nova-launcher").join("version_meta").join(format!("{}.json", version_id)))
}

pub fn save_version_meta_to_disk(version_id: &str, meta: &VersionMeta) {
    if let Some(path) = version_meta_cache_path(version_id) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Ok(json) = serde_json::to_string(meta) {
            std::fs::write(path, json).ok();
        }
    }
}

pub fn load_version_meta_from_disk(version_id: &str) -> Option<VersionMeta> {
    let path = version_meta_cache_path(version_id)?;
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

pub async fn fetch_version_meta(
    client: &reqwest::Client,
    url: &str,
    version_id: &str,
) -> Result<VersionMeta> {
    match client.get(url).send().await.and_then(|r| r.error_for_status()) {
        Ok(resp) => {
            let meta: VersionMeta = resp.json().await?;
            save_version_meta_to_disk(version_id, &meta);
            Ok(meta)
        }
        Err(e) => {
            // Netzwerkfehler → Disk-Cache versuchen
            load_version_meta_from_disk(version_id)
                .ok_or_else(|| anyhow::anyhow!("{e} (kein Cache vorhanden)"))
        }
    }
}
