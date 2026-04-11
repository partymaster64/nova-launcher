use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─── Instance ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub ram_min_mb: u32,
    pub ram_max_mb: u32,
    pub game_dir: Option<PathBuf>,
    pub java_path: Option<String>,
    pub game_width: u32,
    pub game_height: u32,
}

impl Instance {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            version: None,
            ram_min_mb: 512,
            ram_max_mb: 2048,
            game_dir: None,
            java_path: None,
            game_width: 854,
            game_height: 480,
        }
    }

    pub fn game_dir(&self) -> PathBuf {
        self.game_dir.clone().unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".minecraft")
        })
    }

    pub fn java_executable(&self) -> String {
        self.java_path.clone().unwrap_or_else(|| "java".to_string())
    }
}

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

    pub fn to_iced(&self) -> iced::Color {
        iced::Color::from_rgb(self.r, self.g, self.b)
    }
}

impl Default for AccentColor {
    fn default() -> Self {
        Self::default_purple()
    }
}

// ─── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Aktiver Account (UUID)
    pub active_account_uuid: Option<String>,
    /// Aktive Instanz (ID)
    pub active_instance_id: Option<String>,
    /// Alle Instanzen
    #[serde(default)]
    pub instances: Vec<Instance>,
    /// Versionsfilter
    pub show_snapshots: bool,
    pub show_old_alpha: bool,
    pub show_old_beta: bool,
    /// Akzentfarbe
    #[serde(default)]
    pub accent_color: AccentColor,
}

impl Default for Config {
    fn default() -> Self {
        let default_instance = Instance::new("Standard");
        let id = default_instance.id.clone();
        Self {
            active_account_uuid: None,
            active_instance_id: Some(id),
            instances: vec![default_instance],
            show_snapshots: false,
            show_old_alpha: false,
            show_old_beta: false,
            accent_color: AccentColor::default_purple(),
        }
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
        let cfg: Self = serde_json::from_str(&json).unwrap_or_default();
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
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

    /// Compatibility: game_dir from active instance
    pub fn game_dir(&self) -> PathBuf {
        self.active_instance()
            .map(|i| i.game_dir())
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".minecraft")
            })
    }

    /// Compatibility: java_executable from active instance
    pub fn java_executable(&self) -> String {
        self.active_instance()
            .map(|i| i.java_executable())
            .unwrap_or_else(|| "java".to_string())
    }

    /// Compatibility: selected_version from active instance
    pub fn selected_version(&self) -> Option<&str> {
        self.active_instance()?.version.as_deref()
    }

    pub fn ram_min_mb(&self) -> u32 {
        self.active_instance().map(|i| i.ram_min_mb).unwrap_or(512)
    }

    pub fn ram_max_mb(&self) -> u32 {
        self.active_instance().map(|i| i.ram_max_mb).unwrap_or(2048)
    }

    pub fn game_width(&self) -> u32 {
        self.active_instance().map(|i| i.game_width).unwrap_or(854)
    }

    pub fn game_height(&self) -> u32 {
        self.active_instance().map(|i| i.game_height).unwrap_or(480)
    }
}
