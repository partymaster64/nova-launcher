use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─── ModLoader ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    #[default]
    Vanilla,
    Fabric,
    Forge,
    Neoforge,
    Quilt,
    Paper,
}

impl ModLoader {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Vanilla => "Vanilla",
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::Neoforge => "NeoForge",
            Self::Quilt => "Quilt",
            Self::Paper => "Paper",
        }
    }
    pub fn color(&self) -> &'static str {
        match self {
            Self::Vanilla => "#34d399",
            Self::Fabric => "#dba96e",
            Self::Forge => "#c084fc",
            Self::Neoforge => "#f97316",
            Self::Quilt => "#60a5fa",
            Self::Paper => "#f87171",
        }
    }
}

// ─── Instance ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub ram_min_mb: u32,
    pub ram_max_mb: u32,
    /// Absolute path to this instance's directory (None = isolated default)
    #[serde(default)]
    pub game_dir: Option<String>,
    #[serde(default)]
    pub java_path: Option<String>,
    pub game_width: u32,
    pub game_height: u32,
    #[serde(default)]
    pub loader: ModLoader,
    #[serde(default)]
    pub loader_version: Option<String>,
    /// ISO 8601 timestamp of last play session
    #[serde(default)]
    pub last_played: Option<String>,
    /// Modpack source info (modrinth project id)
    #[serde(default)]
    pub modpack_id: Option<String>,
    /// Path to custom instance icon (absolute local file path)
    #[serde(default)]
    pub icon_path: Option<String>,
    /// Original HTTPS URL of the instance icon (used for Discord RPC, since local files can't be used there)
    #[serde(default)]
    pub icon_url: Option<String>,
    /// Instance group / category label
    #[serde(default)]
    pub group: Option<String>,
    /// Launch in fullscreen mode
    #[serde(default)]
    pub fullscreen: bool,
    /// Extra JVM arguments (space-separated string)
    #[serde(default)]
    pub custom_jvm_args: Option<String>,
    /// Extra environment variables [[key, value], ...]
    #[serde(default)]
    pub env_vars: Option<Vec<Vec<String>>>,
    /// Command to run before launching (pre-launch hook)
    #[serde(default)]
    pub pre_launch_hook: Option<String>,
    /// Wrapper command (e.g. gamescope, mangohud) prepended before java
    #[serde(default)]
    pub wrapper_command: Option<String>,
    /// Command to run after game exits (post-exit hook)
    #[serde(default)]
    pub post_exit_hook: Option<String>,
    /// Accumulated playtime in seconds across all sessions
    #[serde(default)]
    pub total_play_secs: u64,
}

impl Instance {
    pub fn new(name: impl Into<String>) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self {
            id,
            name: name.into(),
            version: None,
            ram_min_mb: 512,
            ram_max_mb: 2048,
            game_dir: None, // will be set by create_instance command
            java_path: None,
            game_width: 854,
            game_height: 480,
            loader: ModLoader::Vanilla,
            loader_version: None,
            last_played: None,
            modpack_id: None,
            icon_path: None,
            icon_url: None,
            group: None,
            fullscreen: false,
            custom_jvm_args: None,
            env_vars: None,
            pre_launch_hook: None,
            wrapper_command: None,
            post_exit_hook: None,
            total_play_secs: 0,
        }
    }

    /// Returns the resolved game directory for this instance.
    /// Uses instance-specific dir or falls back to global instances base.
    pub fn resolved_game_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.game_dir {
            if !dir.is_empty() {
                return PathBuf::from(dir);
            }
        }
        // Fallback: isolated dir under nova-launcher data dir
        instances_base_dir().join(&self.id)
    }

    pub fn java_executable(&self) -> String {
        self.java_path.clone().unwrap_or_else(|| "java".to_string())
    }
}

/// Base directory for all instance data
pub fn instances_base_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("nova-launcher")
        .join("instances")
}

// ─── Skin Preset ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinPreset {
    pub id: String,
    pub name: String,
    /// "classic" or "slim"
    #[serde(default = "default_classic")]
    pub model: String,
    /// base64-encoded PNG (no data: prefix)
    pub skin_data: String,
    /// base64-encoded PNG cape (no data: prefix)
    #[serde(default)]
    pub cape_data: Option<String>,
    /// original Mojang texture URL
    #[serde(default)]
    pub skin_url: Option<String>,
    /// Mojang cape UUID
    #[serde(default)]
    pub cape_id: Option<String>,
}

fn default_classic() -> String { "classic".to_string() }

// ─── Accent Color ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccentColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl AccentColor {
    pub fn default_purple() -> Self {
        Self { r: 0.659, g: 0.333, b: 0.969 } // #a855f7
    }

    pub fn to_hex(&self) -> String {
        let r = (self.r * 255.0) as u8;
        let g = (self.g * 255.0) as u8;
        let b = (self.b * 255.0) as u8;
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    }
}

impl Default for AccentColor {
    fn default() -> Self {
        Self::default_purple()
    }
}

fn default_global_ram_min() -> u32 { 512 }
fn default_global_ram_max() -> u32 { 2048 }
fn default_game_width() -> u32 { 854 }
fn default_game_height() -> u32 { 480 }
fn default_font_size() -> u32 { 14 }
fn default_true() -> bool { true }
fn default_sidebar_width() -> u32 { 220 }
fn default_ui_scale() -> f32 { 1.0 }
fn default_language() -> String { "de".to_string() }

// ─── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub active_account_uuid: Option<String>,
    pub active_instance_id: Option<String>,
    #[serde(default)]
    pub instances: Vec<Instance>,
    #[serde(default)]
    pub show_snapshots: bool,
    #[serde(default)]
    pub show_old_alpha: bool,
    #[serde(default)]
    pub show_old_beta: bool,
    #[serde(default)]
    pub accent_color: AccentColor,
    #[serde(default = "default_global_ram_min")]
    pub global_ram_min_mb: u32,
    #[serde(default = "default_global_ram_max")]
    pub global_ram_max_mb: u32,
    #[serde(default)]
    pub global_java_path: Option<String>,
    #[serde(default)]
    pub close_on_launch: bool,
    #[serde(default = "default_true")]
    pub minimize_on_launch: bool,
    #[serde(default = "default_game_width")]
    pub default_game_width: u32,
    #[serde(default = "default_game_height")]
    pub default_game_height: u32,
    /// UI theme: "" | "oled" | "soft" | "forest" | "ocean" | "warm" | "midnight" | "light"
    #[serde(default)]
    pub ui_theme: String,
    /// Border radius: "" | "compact" | "rounded" | "pill"
    #[serde(default)]
    pub ui_radius: String,
    #[serde(default = "default_font_size")]
    pub ui_font_size: u32,
    /// Font family: "" | "nunito" | "mono"
    #[serde(default)]
    pub ui_font: String,
    /// UI zoom scale factor (e.g. 0.85, 1.0, 1.15)
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    /// Layout density: "" | "compact" | "comfortable"
    #[serde(default)]
    pub ui_density: String,
    #[serde(default = "default_true")]
    pub ui_animations: bool,
    #[serde(default = "default_sidebar_width")]
    pub ui_sidebar_width: u32,

    /// Language code: "de" | "en" | "fr"
    #[serde(default = "default_language")]
    pub language: String,

    // ── Skin Presets ──────────────────────────────────────────────────────
    #[serde(default)]
    pub skin_presets: Vec<SkinPreset>,
    #[serde(default)]
    pub active_skin_preset_id: Option<String>,

    // ── Discord Rich Presence ─────────────────────────────────────────────
    #[serde(default)]
    pub discord_rpc_enabled: bool,

    // ── Setup Wizard ──────────────────────────────────────────────────────
    #[serde(default)]
    pub setup_complete: bool,

    // ── Improvements ─────────────────────────────────────────────────────
    /// Auto-compress new PNG screenshots to JPEG after the game exits
    #[serde(default)]
    pub auto_compress_screenshots: bool,
    /// Copy screenshots from all instances into one shared folder
    #[serde(default)]
    pub screenshot_sync: bool,
    /// Share the saves/ folder across all instances (symlink)
    #[serde(default)]
    pub saves_sync: bool,
    /// Share servers.dat across all instances
    #[serde(default)]
    pub servers_sync: bool,
    /// Sync options.txt and keybinds.txt across all instances
    #[serde(default)]
    pub config_sync: bool,
    /// Sync resourcepacks/ folder across all instances
    #[serde(default)]
    pub resource_pack_sync: bool,

    // ── Server play history ───────────────────────────────────────────────
    /// "instanceId:serverHost" -> epoch_ms of last connection (detected via log)
    #[serde(default)]
    pub server_play_history: std::collections::HashMap<String, u64>,

    /// Per-version Java paths: "8" | "17" | "21" | "25" → full path to java executable
    #[serde(default)]
    pub java_paths: std::collections::HashMap<String, String>,

    // ── Global instance defaults ──────────────────────────────────────────
    #[serde(default)]
    pub default_fullscreen: bool,
    #[serde(default)]
    pub default_custom_jvm_args: Option<String>,
    #[serde(default)]
    pub default_env_vars: Option<Vec<Vec<String>>>,
    #[serde(default)]
    pub default_pre_launch_hook: Option<String>,
    #[serde(default)]
    pub default_wrapper_command: Option<String>,
    #[serde(default)]
    pub default_post_exit_hook: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active_account_uuid: None,
            active_instance_id: None,
            instances: vec![],
            show_snapshots: false,
            show_old_alpha: false,
            show_old_beta: false,
            accent_color: AccentColor::default_purple(),
            global_ram_min_mb: 512,
            global_ram_max_mb: 2048,
            global_java_path: None,
            close_on_launch: false,
            minimize_on_launch: true,
            default_game_width: 854,
            default_game_height: 480,
            ui_theme: String::new(),
            ui_radius: String::new(),
            ui_font_size: 14,
            ui_font: String::new(),
            ui_scale: 1.0,
            ui_density: String::new(),
            ui_animations: true,
            ui_sidebar_width: 220,
            language: "de".to_string(),
            discord_rpc_enabled: false,
            setup_complete: false,
            default_fullscreen: false,
            default_custom_jvm_args: None,
            default_env_vars: None,
            default_pre_launch_hook: None,
            default_wrapper_command: None,
            default_post_exit_hook: None,
            skin_presets: vec![],
            active_skin_preset_id: None,
            auto_compress_screenshots: false,
            screenshot_sync: false,
            saves_sync: false,
            servers_sync: false,
            config_sync: false,
            resource_pack_sync: false,
            server_play_history: std::collections::HashMap::new(),
            java_paths: std::collections::HashMap::new(),
        }
    }
}

/// Returns the minimum Java major version required for the given Minecraft version string.
pub fn required_java_major(mc_version: &str) -> u32 {
    let parts: Vec<u32> = mc_version.split('.').filter_map(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        // New versioning scheme: 26.x, 27.x, … (major >= 26 means year-based, released 2026+)
        [major, ..] if *major >= 26 => 21,
        // Legacy 1.x scheme
        [1, minor, patch, ..] => {
            let (minor, patch) = (*minor, *patch);
            if minor >= 21 || (minor == 20 && patch >= 5) { 21 }
            else if minor >= 17 { 17 }
            else { 8 }
        }
        [1, minor] => {
            let minor = *minor;
            if minor >= 21 { 21 } else if minor >= 17 { 17 } else { 8 }
        }
        _ => 17,
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Kein Konfig-Verzeichnis gefunden"))?
            .join("nova-launcher");
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        // Use a lenient Value parse then re-deserialize so unknown/missing fields
        // fall back to serde defaults instead of silently wiping the whole config.
        let value: serde_json::Value = serde_json::from_str(&json)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        Ok(serde_json::from_value(value).unwrap_or_default())
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let json = serde_json::to_string_pretty(self)?;
        // Write to a temp file first, then rename — prevents data loss on crash during write
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn active_instance(&self) -> Option<&Instance> {
        let id = self.active_instance_id.as_deref()?;
        self.instances.iter().find(|i| i.id == id)
    }

    pub fn active_instance_mut(&mut self) -> Option<&mut Instance> {
        let id = self.active_instance_id.clone()?;
        self.instances.iter_mut().find(|i| i.id == id)
    }

    pub fn game_dir(&self) -> PathBuf {
        self.active_instance()
            .map(|i| i.resolved_game_dir())
            .unwrap_or_else(|| instances_base_dir().join("default"))
    }

    pub fn java_executable(&self) -> String {
        // Instance java_path > global java_path > "java"
        if let Some(inst) = self.active_instance() {
            if let Some(ref jp) = inst.java_path {
                if !jp.is_empty() { return jp.clone(); }
            }
        }
        if let Some(ref gj) = self.global_java_path {
            if !gj.is_empty() { return gj.clone(); }
        }
        "java".to_string()
    }

    pub fn java_for_mc_version(&self, mc_version: &str) -> String {
        // 1. Instance-specific path first
        if let Some(inst) = self.active_instance() {
            if let Some(ref jp) = inst.java_path {
                if !jp.is_empty() { return jp.clone(); }
            }
        }
        // 2. Per-version configured path
        let ver = crate::config::required_java_major(mc_version);
        if let Some(path) = self.java_paths.get(&ver.to_string()) {
            if !path.is_empty() { return path.clone(); }
        }
        // 3. Global fallback
        if let Some(ref gj) = self.global_java_path {
            if !gj.is_empty() { return gj.clone(); }
        }
        "java".to_string()
    }

    pub fn selected_version(&self) -> Option<&str> {
        self.active_instance()?.version.as_deref()
    }

    pub fn ram_min_mb(&self) -> u32 {
        self.active_instance().map(|i| i.ram_min_mb).unwrap_or(self.global_ram_min_mb)
    }

    pub fn ram_max_mb(&self) -> u32 {
        self.active_instance().map(|i| i.ram_max_mb).unwrap_or(self.global_ram_max_mb)
    }

    pub fn game_width(&self) -> u32 {
        self.active_instance().map(|i| i.game_width).unwrap_or(854)
    }

    pub fn game_height(&self) -> u32 {
        self.active_instance().map(|i| i.game_height).unwrap_or(480)
    }

    /// Returns instances sorted by last_played (most recent first)
    pub fn recent_instances(&self) -> Vec<&Instance> {
        let mut sorted: Vec<&Instance> = self.instances.iter().collect();
        sorted.sort_by(|a, b| b.last_played.cmp(&a.last_played));
        sorted
    }
}
