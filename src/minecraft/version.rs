use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

// ─── Manifest ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct ArgumentsBlock {
    #[serde(default)]
    pub game: Vec<serde_json::Value>,
    #[serde(default)]
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDownloads {
    pub client: Download,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Download {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<LibraryArtifact>,
    /// Natives-Classifiers (Schlüssel = Classifier-Name)
    #[serde(default)]
    pub classifiers: HashMap<String, LibraryArtifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryArtifact {
    pub url: String,
    pub path: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryRule {
    pub action: String,
    pub os: Option<OsRule>,
}

#[derive(Debug, Clone, Deserialize)]
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

pub async fn fetch_version_meta(
    client: &reqwest::Client,
    url: &str,
) -> Result<VersionMeta> {
    let meta: VersionMeta = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(meta)
}
