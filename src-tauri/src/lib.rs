pub mod auth;
pub mod cache;
pub mod config;
pub mod discord;
pub mod minecraft;
pub mod sync;
pub mod worlds;

use std::sync::{Arc, Mutex};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager, State};
use futures::StreamExt;

use crate::auth::microsoft::{complete_auth, exchange_auth_code, build_auth_url, poll_for_token, request_device_code};
use crate::auth::storage::{AccountStore, StoredAccount};
use crate::config::{Config, Instance, SkinPreset, instances_base_dir};
use crate::minecraft::launcher::{install_and_launch, LaunchEvent, LaunchOptions};
use crate::minecraft::version::{fetch_manifest, load_manifest_from_disk, save_manifest_to_disk, VersionManifest};

// ─── Modrinth Types ───────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ModrinthHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub featured_gallery: Option<String>,
    pub color: Option<i32>,
    pub downloads: u64,
    pub follows: u64,
    pub versions: Vec<String>,
    pub categories: Vec<String>,
    pub project_type: String,
    pub date_modified: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ModrinthSearchResult {
    pub hits: Vec<ModrinthHit>,
    pub total_hits: u64,
    pub offset: u64,
    pub limit: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ModrinthVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ModrinthFile>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
}

// ─── Modpack Install Types ─────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct MrpackIndex {
    name: String,
    files: Vec<MrpackIndexFile>,
    dependencies: std::collections::HashMap<String, String>,
}

#[derive(serde::Deserialize)]
struct MrpackIndexFile {
    path: String,
    downloads: Vec<String>,
    #[serde(default)]
    env: Option<MrpackEnv>,
}

#[derive(serde::Deserialize, Default)]
struct MrpackEnv {
    #[serde(default)]
    client: String,
}

// ─── Modrinth Project Detail Types ────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct ModrinthGalleryItem {
    pub url: String,
    #[serde(default)]
    pub featured: bool,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ModrinthProject {
    pub id: String,
    pub slug: String,
    pub project_type: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub body: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub followers: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub gallery: Vec<ModrinthGalleryItem>,
    pub source_url: Option<String>,
    pub issues_url: Option<String>,
    pub discord_url: Option<String>,
    pub updated: String,
    pub published: String,
    pub license: Option<serde_json::Value>,
}

// ─── Instance Details ─────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ModMeta {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub icon_url: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ModInfo {
    pub filename: String,
    pub size_bytes: u64,
    pub title: Option<String>,
    pub icon_url: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub enabled: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ModUpdateInfo {
    pub content_type: String, // "mod" | "resourcepack" | "shader" | "datapack"
    pub filename: String,
    pub title: String,
    pub project_id: String,
    pub installed_version_id: String,
    pub latest_version_id: String,
    pub latest_version_number: String,
    pub icon_url: Option<String>,
}

#[derive(serde::Serialize)]
pub struct WorldInfo {
    pub name: String,           // folder name (used for quickplay)
    pub display_name: Option<String>, // LevelName from level.dat
    pub last_played_ms: Option<u64>,
    pub icon: Option<String>,   // base64 PNG
}

#[derive(serde::Serialize)]
pub struct ScreenshotInfo {
    pub filename: String,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(serde::Serialize)]
pub struct InstanceDetails {
    pub instance: Instance,
    pub game_dir: String,
    pub mods: Vec<ModInfo>,
    pub resourcepacks: Vec<ModInfo>,
    pub shaderpacks: Vec<ModInfo>,
    pub datapacks: Vec<ModInfo>,
    pub worlds: Vec<WorldInfo>,
    pub servers: Vec<worlds::ServerNbt>,
    pub screenshots: Vec<ScreenshotInfo>,
    pub log_tail: String,
    pub total_mods: usize,
    pub total_worlds: usize,
}

// ─── App State ────────────────────────────────────────────────────────────────

pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub accounts: Arc<Mutex<AccountStore>>,
    pub http: reqwest::Client,
    pub manifest: Mutex<Option<VersionManifest>>,
    pub running_pids: Arc<Mutex<HashMap<String, u32>>>,
    pub instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
    pub instance_errors: Arc<Mutex<HashMap<String, String>>>,
    pub install_progress: Arc<Mutex<HashMap<String, InstallProgress>>>,
    pub instance_updates: Arc<Mutex<HashMap<String, Vec<ModUpdateInfo>>>>,
    /// Aktueller Online-Status (wird periodisch geprüft)
    pub online: Arc<Mutex<bool>>,
    pub stopping_ids: Arc<Mutex<std::collections::HashSet<String>>>,
    pub cache: Arc<cache::HttpCache>,
    /// Channel to send Discord RPC messages; None if RPC is disabled or client_id missing
    pub discord_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<discord::DiscordMsg>>>>,
}

/// Fetch JSON via HTTP, checking cache first. TTL in seconds.
async fn cached_get<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    cache: &cache::HttpCache,
    url: &str,
    ttl_secs: i64,
) -> Result<T, String> {
    if let Some(cached) = cache.get(url) {
        return serde_json::from_value(cached).map_err(|e| e.to_string());
    }
    let value: serde_json::Value = http.get(url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    cache.set(url.to_string(), value.clone(), ttl_secs);
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Fetch plain text via HTTP, checking cache first. Stored as a JSON string.
async fn cached_get_text(
    http: &reqwest::Client,
    cache: &cache::HttpCache,
    url: &str,
    ttl_secs: i64,
) -> Result<String, String> {
    if let Some(cached) = cache.get(url) {
        if let Some(s) = cached.as_str() {
            return Ok(s.to_string());
        }
    }
    let text = http.get(url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .text().await.map_err(|e| e.to_string())?;
    cache.set(url.to_string(), serde_json::Value::String(text.clone()), ttl_secs);
    Ok(text)
}

// ─── Serialisierbare Typen ────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallProgress {
    pub step: String,
    pub percent: f32,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(serde::Serialize, Clone)]
struct DeviceCodeInfo {
    user_code: String,
    verification_uri: String,
    device_code: String,
    interval: u64,
    message: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LoginResult {
    Complete { account: StoredAccount },
    Error { message: String },
}

#[derive(Clone, serde::Serialize)]
struct LaunchEventPayload {
    event_type: String,
    step: Option<String>,
    percent: Option<f32>,
    error: Option<String>,
    instance_id: Option<String>,
    log: Option<String>,
}

impl Default for LaunchEventPayload {
    fn default() -> Self {
        Self {
            event_type: String::new(),
            step: None,
            percent: None,
            error: None,
            instance_id: None,
            log: None,
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn lock_config<'a>(state: &'a State<'a, AppState>) -> Result<std::sync::MutexGuard<'a, Config>, String> {
    state.config.lock().map_err(|e| format!("Config-Lock vergiftet: {e}"))
}

fn lock_accounts<'a>(state: &'a State<'a, AppState>) -> Result<std::sync::MutexGuard<'a, AccountStore>, String> {
    state.accounts.lock().map_err(|e| format!("Accounts-Lock vergiftet: {e}"))
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(lock_config(&state)?.clone())
}

#[tauri::command]
async fn save_config(config: Config, state: State<'_, AppState>) -> Result<(), String> {
    let mut c = lock_config(&state)?;
    *c = config;
    c.save().map_err(|e| e.to_string())?;
    sync::apply_syncs(&c);
    Ok(())
}

#[tauri::command]
async fn get_accounts(state: State<'_, AppState>) -> Result<Vec<StoredAccount>, String> {
    Ok(lock_accounts(&state)?.accounts.clone())
}

/// Konnektivitätstest über rohen TCP-Connect (kein HTTP, kein DNS-Cache, kein Connection-Pool).
/// Verbindet direkt zu Cloudflare 1.1.1.1:443 — schlägt sofort fehl wenn kein Internet vorhanden.
async fn check_internet() -> bool {
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::net::TcpStream::connect("1.1.1.1:443"),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

#[tauri::command]
async fn get_online_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(*state.online.lock().map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn get_server_play_history(state: State<'_, AppState>) -> Result<std::collections::HashMap<String, u64>, String> {
    Ok(lock_config(&state)?.server_play_history.clone())
}

#[tauri::command]
async fn get_manifest(state: State<'_, AppState>) -> Result<VersionManifest, String> {
    {
        let manifest = state.manifest.lock().map_err(|e| e.to_string())?;
        if let Some(m) = manifest.as_ref() {
            return Ok(m.clone());
        }
    }
    match fetch_manifest(&state.http).await {
        Ok(manifest) => {
            save_manifest_to_disk(&manifest);
            *state.manifest.lock().map_err(|e| e.to_string())? = Some(manifest.clone());
            Ok(manifest)
        }
        Err(_) => {
            if let Some(cached) = load_manifest_from_disk() {
                *state.manifest.lock().map_err(|e| e.to_string())? = Some(cached.clone());
                Ok(cached)
            } else {
                Err("Manifest konnte nicht geladen werden und es ist kein Cache vorhanden".to_string())
            }
        }
    }
}

#[tauri::command]
async fn refresh_manifest(state: State<'_, AppState>) -> Result<VersionManifest, String> {
    let manifest = fetch_manifest(&state.http).await.map_err(|e| e.to_string())?;
    save_manifest_to_disk(&manifest);
    *state.manifest.lock().map_err(|e| e.to_string())? = Some(manifest.clone());
    Ok(manifest)
}

#[tauri::command]
/// OAuth2 Authorization Code Flow with Tauri WebviewWindow.
/// Opens a login window, intercepts the redirect to oauth20_desktop.srf, exchanges the code,
/// and delivers the result via a Tauri Channel passed from JS.
async fn start_login_browser(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    on_event: tauri::ipc::Channel<LoginResult>,
) -> Result<(), String> {
    use tauri::{WebviewWindowBuilder, WebviewUrl};
    use tokio::sync::oneshot;

    const REDIRECT_URI: &str = "https://login.live.com/oauth20_desktop.srf";

    let auth_url = build_auth_url(REDIRECT_URI);
    let auth_url_parsed = auth_url.parse::<url::Url>().map_err(|e| e.to_string())?;

    let (tx, rx) = oneshot::channel::<String>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    WebviewWindowBuilder::new(&app, "ms-login", WebviewUrl::External(auth_url_parsed))
        .title("Microsoft Login – Nova Launcher")
        .inner_size(520.0, 680.0)
        .resizable(false)
        .on_navigation(move |url| {
            let url_str = url.as_str();
            // Only intercept the final redirect that contains code=.
            // Intermediate redirects to oauth20_desktop.srf (e.g. "Stay signed in?")
            // do not contain code= and should be allowed through.
            if url_str.starts_with("https://login.live.com/oauth20_desktop.srf")
                && url_str.contains("code=")
            {
                if let Ok(mut guard) = tx.lock() {
                    if let Some(sender) = guard.take() {
                        let _ = sender.send(url_str.to_string());
                    }
                }
                return false;
            }
            true
        })
        .build()
        .map_err(|e| e.to_string())?;

    let http = state.http.clone();
    let accounts_mutex = state.accounts.clone();
    let app_clone = app.clone();

    tokio::spawn(async move {
        let result: Result<StoredAccount, String> = async {
            // Wait up to 5 minutes for the browser login
            let redirect_url = tokio::time::timeout(
                std::time::Duration::from_secs(300),
                rx,
            )
            .await
            .map_err(|_| "Login timeout (5 minutes exceeded)".to_string())?
            .map_err(|_| "Login window was closed".to_string())?;

            // Close the login window
            if let Some(win) = app_clone.get_webview_window("ms-login") {
                let _ = win.close();
            }

            // Extract code from redirect URL
            let parsed = url::Url::parse(&redirect_url).map_err(|e| e.to_string())?;
            if let Some(err) = parsed.query_pairs().find(|(k, _)| k == "error").map(|(_, v)| v.into_owned()) {
                let desc = parsed.query_pairs().find(|(k, _)| k == "error_description")
                    .map(|(_, v)| v.into_owned()).unwrap_or_default();
                return Err(format!("OAuth error: {err} – {desc}"));
            }
            let code = parsed.query_pairs()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.into_owned())
                .ok_or("No 'code' parameter in redirect URL")?;

            // Exchange code for MS tokens (30s timeout)
            let (ms_access, ms_refresh, ms_expires) =
                tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    exchange_auth_code(&http, &code, REDIRECT_URI),
                )
                .await
                .map_err(|_| "Timeout during token exchange (30s)".to_string())?
                .map_err(|e| e.to_string())?;

            // Run XBL → XSTS → Minecraft auth chain (30s timeout)
            let auth = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                complete_auth(&http, &ms_access, &ms_refresh, ms_expires),
            )
            .await
            .map_err(|_| "Timeout during Xbox/Minecraft auth (30s)".to_string())?
            .map_err(|e| e.to_string())?;

            let account = StoredAccount {
                uuid: auth.uuid,
                username: auth.username,
                minecraft_token: auth.minecraft_token,
                refresh_token: auth.refresh_token,
                token_expires_at: auth.expires_at,
                offline: false,
            };

            {
                let snapshot = {
                    let mut accounts = accounts_mutex.lock().map_err(|e| e.to_string())?;
                    accounts.add_or_update(account.clone());
                    accounts.clone()
                };
                snapshot.save_async().await.map_err(|e| e.to_string())?;
            }

            Ok(account)
        }
        .await;

        match result {
            Ok(account) => { let _ = on_event.send(LoginResult::Complete { account }); }
            Err(e) => { let _ = on_event.send(LoginResult::Error { message: e }); }
        }
    });

    Ok(())
}

#[tauri::command]
async fn start_login(state: State<'_, AppState>) -> Result<DeviceCodeInfo, String> {
    let dc = request_device_code(&state.http).await.map_err(|e| e.to_string())?;
    Ok(DeviceCodeInfo {
        user_code: dc.user_code,
        verification_uri: dc.verification_uri,
        device_code: dc.device_code,
        interval: dc.interval,
        message: dc.message,
    })
}

#[tauri::command]
async fn poll_login(
    device_code: String,
    interval: u64,
    state: State<'_, AppState>,
) -> Result<StoredAccount, String> {
    let (ms_access, ms_refresh, ms_expires) =
        poll_for_token(&state.http, &device_code, interval).await.map_err(|e| e.to_string())?;
    let auth = complete_auth(&state.http, &ms_access, &ms_refresh, ms_expires)
        .await.map_err(|e| e.to_string())?;
    let account = StoredAccount {
        uuid: auth.uuid,
        username: auth.username,
        minecraft_token: auth.minecraft_token,
        refresh_token: auth.refresh_token,
        token_expires_at: auth.expires_at,
        offline: false,
    };
    {
        let snapshot = {
            let mut accounts = lock_accounts(&state)?;
            accounts.add_or_update(account.clone());
            accounts.clone()
        };
        snapshot.save_async().await.map_err(|e| e.to_string())?;
    }
    {
        let mut config = lock_config(&state)?;
        config.active_account_uuid = Some(account.uuid.clone());
        config.save().map_err(|e| e.to_string())?;
    }
    Ok(account)
}

#[tauri::command]
async fn remove_account(uuid: String, state: State<'_, AppState>) -> Result<(), String> {
    let snapshot = {
        let mut accounts = lock_accounts(&state)?;
        accounts.remove(&uuid);
        accounts.clone()
    };
    snapshot.save_async().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_active_account(uuid: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut config = lock_config(&state)?;
    config.active_account_uuid = Some(uuid);
    config.save().map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_offline_account(
    username: String,
    state: State<'_, AppState>,
) -> Result<StoredAccount, String> {
    let username = username.trim().to_string();
    if username.len() < 3 || username.len() > 16 {
        return Err("Benutzername muss 3–16 Zeichen lang sein".to_string());
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Benutzername darf nur A–Z, a–z, 0–9 und _ enthalten".to_string());
    }
    let account = StoredAccount::new_offline(&username);
    {
        let snapshot = {
            let mut accounts = lock_accounts(&state)?;
            accounts.add_or_update(account.clone());
            accounts.clone()
        };
        snapshot.save_async().await.map_err(|e| e.to_string())?;
    }
    {
        let mut config = lock_config(&state)?;
        config.active_account_uuid = Some(account.uuid.clone());
        config.save().map_err(|e| e.to_string())?;
    }
    Ok(account)
}

#[tauri::command]
async fn add_instance(state: State<'_, AppState>) -> Result<Config, String> {
    let mut config = lock_config(&state)?;
    let n = config.instances.len() + 1;
    let mut inst = Instance::new(format!("Instanz {}", n));
    // Set isolated directory
    let dir = instances_base_dir().join(&inst.id);
    inst.game_dir = Some(dir.display().to_string());
    let id = inst.id.clone();
    config.instances.push(inst);
    config.active_instance_id = Some(id);
    config.save().map_err(|e| e.to_string())?;
    sync::apply_syncs(&config);
    Ok(config.clone())
}

#[tauri::command]
async fn delete_instance(id: String, state: State<'_, AppState>) -> Result<Config, String> {
    // Resolve dir and update config — drop the lock before any async I/O
    let (instance_dir, config_clone) = {
        let mut config = lock_config(&state)?;
        let instance_dir = if let Some(inst) = config.instances.iter().find(|i| i.id == id) {
            inst.resolved_game_dir()
        } else {
            instances_base_dir().join(&id)
        };
        config.instances.retain(|i| i.id != id);
        if config.instances.is_empty() {
            config.active_instance_id = None;
        } else if config.active_instance_id.as_deref() == Some(&id) {
            config.active_instance_id = Some(config.instances[0].id.clone());
        }
        config.save().map_err(|e| e.to_string())?;
        (instance_dir, config.clone())
    };

    // Delete the instance folder from disk (lock already released)
    if instance_dir.exists() {
        tokio::fs::remove_dir_all(&instance_dir).await.map_err(|e| e.to_string())?;
    }

    Ok(config_clone)
}

#[tauri::command]
async fn select_instance(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut config = lock_config(&state)?;
    config.active_instance_id = Some(id);
    config.save().map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_instance(instance: Instance, state: State<'_, AppState>) -> Result<Config, String> {
    let mut config = lock_config(&state)?;
    if let Some(idx) = config.instances.iter().position(|i| i.id == instance.id) {
        config.instances[idx] = instance;
        config.save().map_err(|e| e.to_string())?;
    }
    Ok(config.clone())
}

#[tauri::command]
async fn duplicate_instance(instance_id: String, state: State<'_, AppState>) -> Result<Config, String> {
    use crate::config::instances_base_dir;
    let (src_dir, new_instance) = {
        let config = lock_config(&state)?;
        let src = config.instances.iter().find(|i| i.id == instance_id)
            .ok_or("Instanz nicht gefunden")?;
        let src_dir = src.resolved_game_dir();
        let mut new = src.clone();
        new.id = uuid::Uuid::new_v4().to_string();
        new.name = format!("{} (Kopie)", src.name);
        new.game_dir = Some(instances_base_dir().join(&new.id).display().to_string());
        new.last_played = None;
        (src_dir, new)
    };
    // Copy game directory
    let dst_dir = new_instance.resolved_game_dir();
    if src_dir.exists() {
        copy_dir_recursive(&src_dir, &dst_dir).map_err(|e| e.to_string())?;
    }
    let mut config = lock_config(&state)?;
    config.instances.push(new_instance);
    config.save().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn java_install_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")))
        .join("nova-launcher")
        .join("java")
}

fn system_java_search_paths(version: u32) -> Vec<String> {
    let java_exe = if cfg!(windows) { "java.exe" } else { "java" };
    let v = version.to_string();
    let mut paths = Vec::new();
    if cfg!(target_os = "linux") {
        paths.push(format!("/usr/lib/jvm/java-{}/bin/{}", v, java_exe));
        paths.push(format!("/usr/lib/jvm/java-{}-openjdk/bin/{}", v, java_exe));
        paths.push(format!("/usr/lib/jvm/java-{}-openjdk-amd64/bin/{}", v, java_exe));
        paths.push(format!("/usr/lib/jvm/java-{}-openjdk-arm64/bin/{}", v, java_exe));
        paths.push(format!("/usr/lib/jvm/temurin-{}/bin/{}", v, java_exe));
        paths.push(format!("/usr/lib/jvm/java-{}-amazon-corretto/bin/{}", v, java_exe));
        if let Some(home) = dirs::home_dir() {
            let sdkman = home.join(".sdkman").join("candidates").join("java");
            paths.push(sdkman.join(format!("{}", v)).join("bin").join(java_exe).to_string_lossy().into_owned());
        }
    } else if cfg!(target_os = "macos") {
        paths.push(format!("/Library/Java/JavaVirtualMachines/temurin-{}.jdk/Contents/Home/bin/{}", v, java_exe));
        paths.push(format!("/Library/Java/JavaVirtualMachines/jdk-{}.jdk/Contents/Home/bin/{}", v, java_exe));
        paths.push(format!("/usr/local/opt/openjdk@{}/bin/{}", v, java_exe));
        paths.push(format!("/opt/homebrew/opt/openjdk@{}/bin/{}", v, java_exe));
    } else {
        // Windows
        paths.push(format!("C:\\Program Files\\Eclipse Adoptium\\jre-{}.0.0+0-hotspot\\bin\\{}", v, java_exe));
        paths.push(format!("C:\\Program Files\\Java\\jre{}\\bin\\{}", v, java_exe));
    }
    paths
}

#[tauri::command]
fn get_system_ram_mb() -> u64 {
    // Read from /proc/meminfo on Linux, or use sysinfo crate if available
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb.parse::<u64>() {
                            return kb / 1024;
                        }
                    }
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("sysctl").arg("-n").arg("hw.memsize").output() {
            if let Ok(s) = String::from_utf8(out.stdout) {
                if let Ok(bytes) = s.trim().parse::<u64>() {
                    return bytes / (1024 * 1024);
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        // GlobalMemoryStatusEx via Windows API would need winapi crate; fallback
    }
    8192 // fallback
}

#[tauri::command]
async fn test_java(path: String) -> Result<String, String> {
    let out = tokio::process::Command::new(&path)
        .arg("-version")
        .output()
        .await
        .map_err(|e| format!("Java konnte nicht gestartet werden: {e}"))?;
    // java -version writes to stderr
    let text = String::from_utf8_lossy(&out.stderr).to_string();
    let text = if text.trim().is_empty() { String::from_utf8_lossy(&out.stdout).to_string() } else { text };
    if text.trim().is_empty() {
        return Err(format!("Kein Output (exit {})", out.status));
    }
    Ok(text.lines().next().unwrap_or("").to_string())
}

#[tauri::command]
async fn detect_java(version: u32) -> Result<Option<String>, String> {
    let java_exe = if cfg!(windows) { "java.exe" } else { "java" };
    // 1. Launcher-local first
    let local = java_install_dir().join(version.to_string()).join("bin").join(java_exe);
    if local.exists() {
        return Ok(Some(local.to_string_lossy().into_owned()));
    }
    // 2. System paths
    for p in system_java_search_paths(version) {
        if std::path::Path::new(&p).exists() {
            return Ok(Some(p));
        }
    }
    Ok(None)
}

#[tauri::command]
async fn install_java(version: u32, state: State<'_, AppState>) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;
    let java_exe = if cfg!(windows) { "java.exe" } else { "java" };
    let os_str = if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "mac" } else { "linux" };
    let arch_str = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x64" };

    #[derive(serde::Deserialize)]
    struct Asset { binary: Binary }
    #[derive(serde::Deserialize)]
    struct Binary { package: Package }
    #[derive(serde::Deserialize)]
    struct Package { link: String }

    let api_url = format!(
        "https://api.adoptium.net/v3/assets/latest/{}/hotspot?os={}&architecture={}&image_type=jre",
        version, os_str, arch_str
    );
    let assets: Vec<Asset> = state.http.get(&api_url).send().await
        .map_err(|e| format!("API-Anfrage fehlgeschlagen: {e}"))?
        .json().await
        .map_err(|e| format!("API-Antwort ungültig: {e}"))?;
    let link = assets.into_iter().next()
        .ok_or_else(|| format!("Keine Java {}-JRE für {}/{} gefunden", version, os_str, arch_str))?
        .binary.package.link;

    let install_dir = java_install_dir().join(version.to_string());
    tokio::fs::create_dir_all(&install_dir).await
        .map_err(|e| format!("Verzeichnis konnte nicht erstellt werden: {e}"))?;

    let tmp = java_install_dir().join(format!("jre-{}.tmp", version));
    let mut resp = state.http.get(&link).send().await
        .map_err(|e| format!("Download fehlgeschlagen: {e}"))?;
    {
        let mut f = tokio::fs::File::create(&tmp).await
            .map_err(|e| format!("Temp-Datei konnte nicht erstellt werden: {e}"))?;
        while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
            f.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
    }

    let tar_out = tokio::process::Command::new("tar")
        .arg("xzf").arg(&tmp)
        .arg("-C").arg(&install_dir)
        .arg("--strip-components=1")
        .output().await
        .map_err(|e| format!("tar konnte nicht gestartet werden: {e}"))?;
    let _ = tokio::fs::remove_file(&tmp).await;
    if !tar_out.status.success() {
        return Err(format!("Entpacken fehlgeschlagen: {}", String::from_utf8_lossy(&tar_out.stderr)));
    }

    let java_path = install_dir.join("bin").join(java_exe).to_string_lossy().into_owned();
    if let Ok(mut cfg) = state.config.lock() {
        cfg.java_paths.insert(version.to_string(), java_path.clone());
        let _ = cfg.save();
    }
    Ok(java_path)
}

#[tauri::command]
async fn launch_game(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    launch_instance_internal(app, state, None, vec![]).await
}

#[tauri::command]
async fn launch_instance(
    instance_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    launch_instance_internal(app, state, Some(instance_id), vec![]).await
}

/// Launch directly into a world or server.
/// target_type: "world" | "server"
/// target: world folder name OR "host:port"
#[tauri::command]
async fn launch_with_quickplay(
    instance_id: String,
    target_type: String,
    target: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let args = match target_type.as_str() {
        "world" => vec![
            "--quickPlaySingleplayer".to_string(),
            target,
        ],
        "server" => {
            let (host, port) = if let Some(idx) = target.rfind(':') {
                (&target[..idx], &target[idx+1..])
            } else {
                (target.as_str(), "25565")
            };
            vec![
                // 1.20+ Quick Play
                "--quickPlayMultiplayer".to_string(), format!("{}:{}", host, port),
                // pre-1.20 fallback
                "--server".to_string(), host.to_string(),
                "--port".to_string(), port.to_string(),
            ]
        }
        _ => vec![],
    };
    launch_instance_internal(app, state, Some(instance_id), args).await
}

/// Build a Discord "playing" activity (used both on game start and on world/server change).
fn make_playing_activity(
    instance_name: &str,
    instance_icon: &str,
    state: Option<String>,
    start_ts: u64,
    player_head: &str,
    small_text: &str,
) -> discord::Activity {
    discord::Activity {
        details: Some(format!("Spielt {}", instance_name)),
        state,
        start_timestamp: Some(start_ts),
        large_image: Some(instance_icon.to_string()),
        large_text: Some(instance_name.to_string()),
        small_image: Some(player_head.to_string()),
        small_text: Some(small_text.to_string()),
    }
}

/// Returns true if this log line signals the player left a world or server.
fn is_mc_disconnect(line: &str) -> bool {
    // Singleplayer server shutdown (1.16+)
    line.contains("Stopping singleplayer server") ||
    // Generic server stop — also covers Forge "Stopping server"
    line.contains("Stopping server") ||
    // Client-side disconnect message (multiplayer leave / kick)
    line.contains("Disconnecting") ||
    // Fabric/vanilla network layer
    line.contains("Lost connection")
}

/// Parse a Minecraft log line and return a new Discord RPC state string if the
/// player joined a server or loaded a singleplayer world.
fn parse_mc_location(line: &str) -> Option<String> {
    // Multiplayer — "[Render thread/INFO]: Connecting to hostname, 25565"
    if line.contains("Connecting to ") {
        if let Some(idx) = line.find("Connecting to ") {
            let rest = &line[idx + "Connecting to ".len()..];
            let host = rest.split(',').next().unwrap_or("").trim();
            if !host.is_empty() && !host.contains(' ') {
                return Some(format!("↳ {}", host));
            }
        }
    }
    // Singleplayer join — Minecraft 1.18+ / Fabric / Forge:
    // "[Server thread/INFO]: Starting integrated minecraft server version ..."
    if line.contains("Starting integrated minecraft server") ||
       line.contains("Starting integrated Minecraft server") {
        return Some("↳ Singleplayer".to_string());
    }
    // Singleplayer world name — older Minecraft (pre-1.18) logs "Preparing level"
    for &(needle, close) in &[
        ("Preparing level \"", '"'),
        ("Preparing level '",  '\''),
        ("Loading level \"",   '"'),
        ("Loading level '",    '\''),
    ] {
        if let Some(idx) = line.find(needle) {
            let rest = &line[idx + needle.len()..];
            if let Some(end) = rest.find(close) {
                let world = rest[..end].trim();
                if !world.is_empty() {
                    return Some(format!("↳ {}", world));
                }
            }
        }
    }
    None
}

async fn launch_instance_internal(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    instance_id: Option<String>,
    quickplay_args: Vec<String>,
) -> Result<(), String> {
    let (config, account, version_entry, inst_id) = {
        let mut cfg = lock_config(&state)?;

        // Select the target instance
        let inst = match &instance_id {
            Some(id) => cfg.instances.iter().find(|i| &i.id == id).cloned(),
            None => cfg.active_instance().cloned(),
        }.ok_or("Keine Instanz ausgewählt")?;

        let version_id = inst.version.clone().ok_or("Keine Version ausgewählt")?;
        let acc_uuid = cfg.active_account_uuid.clone().ok_or("Nicht eingeloggt")?;

        let account = {
            let accounts = state.accounts.lock().map_err(|e| e.to_string())?;
            accounts.get(&acc_uuid).cloned().ok_or("Account nicht gefunden")?
        };

        let manifest = {
            let guard = state.manifest.lock().map_err(|e| e.to_string())?;
            if let Some(m) = guard.as_ref() {
                m.clone()
            } else {
                drop(guard);
                load_manifest_from_disk()
                    .ok_or("Manifest nicht geladen – bitte erst online die Versionen laden")?
            }
        };

        let entry = manifest.versions.iter()
            .find(|v| v.id == version_id)
            .cloned()
            .ok_or("Version nicht im Manifest gefunden")?;

        let iid = inst.id.clone();

        // Ensure instance dir exists
        let game_dir = inst.resolved_game_dir();
        std::fs::create_dir_all(&game_dir).map_err(|e| format!("Instanz-Verzeichnis: {e}"))?;

        // Build single-instance config for launcher — skip skin_presets (large base64 blobs not needed by launcher)
        let launch_config = Config {
            active_instance_id: Some(inst.id.clone()),
            instances: vec![inst],
            active_account_uuid: Some(acc_uuid),
            skin_presets: vec![],
            ..cfg.clone()
        };

        // Record last_played
        if let Some(idx) = cfg.instances.iter().position(|i| i.id == iid) {
            cfg.instances[idx].last_played = Some(now_iso());
            let _ = cfg.save();
        }

        (launch_config, account, entry, iid)
    };

    let http = state.http.clone();
    let is_online = *state.online.lock().map_err(|e| e.to_string())?;
    let accounts_mutex = state.accounts.clone();
    let running_pids = state.running_pids.clone();
    let instance_logs = state.instance_logs.clone();
    let instance_errors = state.instance_errors.clone();
    let stopping_ids = state.stopping_ids.clone();
    let discord_tx = state.discord_tx.clone();
    let config_arc = state.config.clone();

    // Capture Discord RPC base settings + behaviour flags
    let minimize_on_launch = { let cfg = lock_config(&state)?; cfg.minimize_on_launch };
    let (rpc_enabled, rpc_instance_name, rpc_instance_version, local_icon_path, existing_icon_url) = {
        let cfg = lock_config(&state)?;
        let inst = cfg.instances.iter().find(|i| i.id == inst_id);
        let enabled = cfg.discord_rpc_enabled;
        let name = inst.map(|i| i.name.clone()).unwrap_or_default();
        let version = inst.and_then(|i| i.version.clone()).unwrap_or_default();
        let local_path = inst.and_then(|i| i.icon_path.clone());
        let icon_url = inst.and_then(|i| i.icon_url.clone()).filter(|u| u.starts_with("https://"));
        (enabled, name, version, local_path, icon_url)
    };

    // Player head via mc-heads.net (HTTPS, works with Discord's image proxy)
    let rpc_player_head = format!("https://mc-heads.net/avatar/{}/64", account.uuid);
    let rpc_player_name = account.username.clone();

    // Resolve instance icon HTTPS URL for Discord RPC.
    // If no URL is stored yet but a local icon file exists, upload it to catbox.moe now
    // and save for future launches.
    let rpc_instance_icon = if rpc_enabled {
        if let Some(url) = existing_icon_url {
            url
        } else if let Some(path) = local_icon_path {
            let ext = std::path::Path::new(&path)
                .extension().and_then(|e| e.to_str()).unwrap_or("png").to_string();
            match tokio::fs::read(&path).await {
                Ok(bytes) => {
                    match upload_icon_to_catbox(&state.http, bytes, &ext).await {
                        Some(url) => {
                            // Persist for future launches
                            if let Ok(mut cfg) = lock_config(&state) {
                                if let Some(inst) = cfg.instances.iter_mut().find(|i| i.id == inst_id) {
                                    inst.icon_url = Some(url.clone());
                                }
                                let _ = cfg.save();
                            }
                            url
                        }
                        None => "nova_playing_block".to_string(),
                    }
                }
                Err(_) => "nova_playing_block".to_string(),
            }
        } else {
            "nova_playing_block".to_string()
        }
    } else {
        String::new()
    };


    // Clear previous error on new launch
    if let Ok(mut errs) = instance_errors.lock() { errs.remove(&inst_id); }

    let (tx, mut rx) = futures::channel::mpsc::unbounded::<LaunchEvent>();

    let opts = LaunchOptions { client: http, config, version_entry, account, tx, install_only: false, online: is_online, quickplay_args };

    tokio::spawn(async move {
        install_and_launch(opts).await;
    });

    let app_clone = app.clone();
    let iid_clone = inst_id.clone();
    tokio::spawn(async move {
        // Track game state across events
        let mut game_start_ts: u64 = 0;
        let mut rpc_game_state: Option<String> =
            if rpc_instance_version.is_empty() { None } else { Some(rpc_instance_version.clone()) };
        let mut rpc_small_text = rpc_player_name.clone();
        // true while player is on an external (multiplayer) server
        let mut rpc_in_multiplayer = false;
        // countdown for detecting server leave via "Stopping worker threads"
        // without a following "Started N worker threads" (which would indicate reload)
        let mut rpc_worker_stop_countdown: i32 = 0;

        while let Some(event) = rx.next().await {
            let mut delay_before_emit = false;
            let payload = match event {
                LaunchEvent::Progress { step, percent } => LaunchEventPayload {
                    event_type: "progress".into(),
                    step: Some(step),
                    percent: Some(percent),
                    instance_id: Some(iid_clone.clone()),
                    ..Default::default()
                },
                LaunchEvent::Running(pid) => {
                    if let Ok(mut pids) = running_pids.lock() {
                        pids.insert(iid_clone.clone(), pid);
                    }
                    // Launcher-Fenster in die Taskleiste minimieren wenn das Spiel startet.
                    // minimize() statt hide() damit der Taskleisteneintrag sichtbar bleibt.
                    // Der 1s-Crash-Detection-Poll in App.svelte wurde als Workaround für
                    // das WebKit-JS-Freeze beim Minimieren eingebaut.
                    if minimize_on_launch {
                        if let Some(win) = app_clone.get_webview_window("main") {
                            let _ = win.minimize();
                        }
                    }
                    // Track game start time (always — used for crash detection and playtime)
                    game_start_ts = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    // Start Discord RPC — show playing activity
                    if rpc_enabled {
                        let activity = make_playing_activity(
                            &rpc_instance_name, &rpc_instance_icon,
                            rpc_game_state.clone(), game_start_ts,
                            &rpc_player_head, &rpc_small_text,
                        );
                        if let Ok(guard) = discord_tx.lock() {
                            if let Some(tx) = guard.as_ref() {
                                tx.try_send(discord::DiscordMsg::Set(activity)).ok();
                            }
                        }
                    }
                    LaunchEventPayload {
                        event_type: "running".into(),
                        instance_id: Some(iid_clone.clone()),
                        ..Default::default()
                    }
                },
                LaunchEvent::Exited(exit_code) => {
                    if let Ok(mut pids) = running_pids.lock() { pids.remove(&iid_clone); }
                    // Capture logs BEFORE removing — needed for log-based crash detection
                    let session_logs: Vec<String> = instance_logs.lock().ok()
                        .and_then(|mut l| l.remove(&iid_clone))
                        .map(|d| d.into_iter().collect())
                        .unwrap_or_default();
                    let was_stopping = stopping_ids.lock()
                        .map(|mut s| s.remove(&iid_clone))
                        .unwrap_or(false);
                    // Accumulate playtime
                    if game_start_ts > 0 {
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                        let session_secs = now.saturating_sub(game_start_ts);
                        if let Ok(mut cfg) = config_arc.lock() {
                            if let Some(inst) = cfg.instances.iter_mut().find(|i| i.id == iid_clone) {
                                inst.total_play_secs += session_secs;
                            }
                            let _ = cfg.save();
                        }
                    }
                    // Post-exit: auto-compress screenshots if enabled
                    {
                        let compress = config_arc.lock().ok()
                            .map(|c| c.auto_compress_screenshots)
                            .unwrap_or(false);
                        if compress {
                            let game_dir = config_arc.lock().ok()
                                .and_then(|c| c.instances.iter().find(|i| i.id == iid_clone).map(|i| i.resolved_game_dir()))
                                .unwrap_or_else(|| instances_base_dir().join(&iid_clone));
                            tokio::spawn(async move { sync::compress_instance_screenshots(game_dir).await });
                        }
                    }
                    // Return to idle Discord RPC
                    if rpc_enabled {
                        if let Ok(guard) = discord_tx.lock() {
                            if let Some(tx) = guard.as_ref() {
                                tx.try_send(discord::DiscordMsg::Set(discord::idle_activity())).ok();
                            }
                        }
                    }
                    // Detect crash — runs for any exit code (0 or non-zero).
                    // Priority: crash-reports/ file → latest.log scan → in-memory log scan → exit code fallback
                    let crash_msg = if !was_stopping && game_start_ts > 0 {
                        let game_dir = config_arc.lock().ok()
                            .and_then(|c| c.instances.iter().find(|i| i.id == iid_clone).map(|i| i.resolved_game_dir()))
                            .unwrap_or_else(|| instances_base_dir().join(&iid_clone));
                        find_crash_report_after(&game_dir.join("crash-reports"), game_start_ts)
                        .or_else(|| find_jvm_crash_after(&game_dir, game_start_ts))
                        .or_else(|| {
                            let log_file = game_dir.join("logs").join("latest.log");
                            std::fs::read_to_string(&log_file).ok()
                                .map(|s| s.lines().map(String::from).collect::<Vec<_>>())
                                .as_deref()
                                .and_then(find_crash_in_logs)
                        })
                        .or_else(|| find_crash_in_logs(&session_logs))
                        .or_else(|| exit_code.filter(|&c| c != 0).map(|c| format!("Minecraft crashed (exit code {c})")))
                    } else {
                        None
                    };
                    // Launcher-Fenster nach Spielende wiederherstellen.
                    if minimize_on_launch {
                        if let Some(win) = app_clone.get_webview_window("main") {
                            let _ = win.unminimize();
                            let _ = win.set_focus();
                        }
                    }
                    if let Some(err) = crash_msg {
                        if let Ok(mut errs) = instance_errors.lock() { errs.insert(iid_clone.clone(), err.clone()); }
                        delay_before_emit = true;
                        LaunchEventPayload {
                            event_type: "error".into(),
                            error: Some(err),
                            instance_id: Some(iid_clone.clone()),
                            ..Default::default()
                        }
                    } else {
                        if let Ok(mut errs) = instance_errors.lock() { errs.remove(&iid_clone); }
                        LaunchEventPayload {
                            event_type: "done".into(),
                            instance_id: Some(iid_clone.clone()),
                            ..Default::default()
                        }
                    }
                },
                LaunchEvent::Failed(e) => {
                    // Launch-time failure: installation error, Java not found, etc.
                    // The game process never started — no playtime, no crash detection needed.
                    if let Ok(mut pids) = running_pids.lock() { pids.remove(&iid_clone); }
                    if let Ok(mut logs) = instance_logs.lock() { logs.remove(&iid_clone); }
                    // Clear stopping_ids so stale was_stopping can't suppress crashes on the next launch
                    stopping_ids.lock().ok().map(|mut s| s.remove(&iid_clone));
                    if let Ok(mut errs) = instance_errors.lock() { errs.insert(iid_clone.clone(), e.clone()); }
                    if let Some(win) = app_clone.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                    delay_before_emit = true;
                    if rpc_enabled {
                        if let Ok(guard) = discord_tx.lock() {
                            if let Some(tx) = guard.as_ref() {
                                tx.try_send(discord::DiscordMsg::Set(discord::idle_activity())).ok();
                            }
                        }
                    }
                    LaunchEventPayload {
                        event_type: "error".into(),
                        error: Some(e),
                        instance_id: Some(iid_clone.clone()),
                        ..Default::default()
                    }
                },
                LaunchEvent::Log(line) => {
                    if let Ok(mut logs) = instance_logs.lock() {
                        let v = logs.entry(iid_clone.clone()).or_default();
                        v.push_back(line.clone());
                        if v.len() > 500 { v.pop_front(); }
                    }
                    // Track which server was connected to (for "Weiterspielen" history)
                    // Only update in-memory — config is saved by the Exited handler
                    if line.contains("Connecting to ") {
                        if let Some(idx) = line.find("Connecting to ") {
                            let rest = &line[idx + "Connecting to ".len()..];
                            let host = rest.split(',').next().unwrap_or("").trim().to_string();
                            if !host.is_empty() && !host.contains(' ') {
                                let now_ms = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0);
                                let key = format!("{}:{}", iid_clone, host);
                                if let Ok(mut cfg) = config_arc.lock() {
                                    cfg.server_play_history.insert(key, now_ms);
                                }
                            }
                        }
                    }
                    if rpc_enabled && game_start_ts > 0 {
                        // Worker-thread countdown: detect server leave when
                        // "Stopping worker threads" is NOT immediately followed by
                        // "Started N worker threads" (which only happens on in-session reloads).
                        let mut worker_disconnect = false;
                        if line.contains("Started") && line.contains("worker threads") {
                            // Confirmed in-session resource reload — cancel any pending countdown
                            rpc_worker_stop_countdown = 0;
                        } else if rpc_worker_stop_countdown > 0 {
                            rpc_worker_stop_countdown -= 1;
                            if rpc_worker_stop_countdown == 0 {
                                worker_disconnect = true;
                            }
                        }
                        // Start countdown when worker threads stop during a multiplayer session
                        if line.contains("Stopping worker threads") && rpc_in_multiplayer {
                            rpc_worker_stop_countdown = 5;
                        }

                        let disconnect = is_mc_disconnect(&line) || worker_disconnect;
                        if disconnect {
                            let reset_state = if rpc_instance_version.is_empty() { None } else { Some(rpc_instance_version.clone()) };
                            if rpc_game_state != reset_state || rpc_small_text != rpc_player_name {
                                rpc_game_state = reset_state;
                                rpc_small_text = rpc_player_name.clone();
                                let activity = make_playing_activity(
                                    &rpc_instance_name, &rpc_instance_icon,
                                    rpc_game_state.clone(), game_start_ts,
                                    &rpc_player_head, &rpc_small_text,
                                );
                                if let Ok(guard) = discord_tx.lock() {
                                    if let Some(tx) = guard.as_ref() {
                                        tx.try_send(discord::DiscordMsg::Set(activity)).ok();
                                    }
                                }
                            }
                            rpc_in_multiplayer = false;
                            rpc_worker_stop_countdown = 0;
                        } else if let Some(new_state) = parse_mc_location(&line) {
                            // Detect if switching to multiplayer or singleplayer
                            rpc_in_multiplayer = !new_state.contains("Singleplayer");
                            rpc_worker_stop_countdown = 0; // new session, clear any stale countdown
                            if rpc_game_state.as_deref() != Some(new_state.as_str()) {
                                rpc_game_state = Some(new_state);
                                rpc_small_text = format!("{} · In-Game", rpc_player_name);
                                let activity = make_playing_activity(
                                    &rpc_instance_name, &rpc_instance_icon,
                                    rpc_game_state.clone(), game_start_ts,
                                    &rpc_player_head, &rpc_small_text,
                                );
                                if let Ok(guard) = discord_tx.lock() {
                                    if let Some(tx) = guard.as_ref() {
                                        tx.try_send(discord::DiscordMsg::Set(activity)).ok();
                                    }
                                }
                            }
                        }
                    }
                    LaunchEventPayload {
                        event_type: "log".into(),
                        log: Some(line),
                        instance_id: Some(iid_clone.clone()),
                        ..Default::default()
                    }
                },
                LaunchEvent::TokenRefreshed(acc) => {
                    let snapshot_opt = accounts_mutex.lock().ok().map(|mut a| {
                        a.add_or_update(acc);
                        a.clone()
                    });
                    if let Some(snapshot) = snapshot_opt {
                        let _ = snapshot.save_async().await;
                    }
                    LaunchEventPayload {
                        event_type: "token_refreshed".into(),
                        instance_id: Some(iid_clone.clone()),
                        ..Default::default()
                    }
                }
            };
            // For crash events: wait for WebKitGTK to unfreeze JS after unminimize()
            if delay_before_emit {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            app_clone.emit("launch_event", payload).ok();
        }
    });

    Ok(())
}

/// Returns which instance IDs are currently running (non-empty = running)
#[tauri::command]
fn get_running_instances(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let pids = state.running_pids.lock().map_err(|e| e.to_string())?;
    Ok(pids.keys().cloned().collect())
}

/// Returns all buffered log lines for an instance (cumulative, not consumed)
#[tauri::command]
fn get_instance_logs(instance_id: String, state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let logs = state.instance_logs.lock().map_err(|e| e.to_string())?;
    Ok(logs.get(&instance_id).map(|d| d.iter().cloned().collect()).unwrap_or_default())
}

/// Returns the last error/crash message for an instance, if any
#[tauri::command]
fn get_instance_error(instance_id: String, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let errs = state.instance_errors.lock().map_err(|e| e.to_string())?;
    Ok(errs.get(&instance_id).cloned())
}

/// Returns the first pending crash across ALL instances — no instance list needed.
/// The frontend polls this unconditionally to avoid relying on config being loaded.
#[tauri::command]
fn get_pending_crash(state: State<'_, AppState>) -> Result<Option<(String, String)>, String> {
    let errs = state.instance_errors.lock().map_err(|e| e.to_string())?;
    if let Some((id, err)) = errs.iter().next() {
        return Ok(Some((id.clone(), err.clone())));
    }
    Ok(None)
}

/// Clears the stored crash error for an instance (called when user dismisses the crash modal)
#[tauri::command]
fn clear_instance_error(instance_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut errs = state.instance_errors.lock().map_err(|e| e.to_string())?;
    errs.remove(&instance_id);
    Ok(())
}

#[tauri::command]
async fn kill_instance(instance_id: String, state: State<'_, AppState>) -> Result<(), String> {
    // Mark as intentionally stopped so the non-zero exit doesn't show as a crash
    if let Ok(mut s) = state.stopping_ids.lock() { s.insert(instance_id.clone()); }

    let pid = {
        let pids = state.running_pids.lock().map_err(|e| e.to_string())?;
        pids.get(&instance_id).copied()
    };
    if let Some(pid) = pid {
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
        }
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .status();
        }
    }
    Ok(())
}

#[tauri::command]
async fn read_icon(path: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = tokio::fs::read(&path).await.map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let ext = std::path::Path::new(&path)
        .extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif"          => "image/gif",
        "webp"         => "image/webp",
        _              => "image/png",
    };
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
async fn read_screenshot(path: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = tokio::fs::read(&path).await.map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let ext = std::path::Path::new(&path)
        .extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
    let mime = if matches!(ext.as_str(), "jpg" | "jpeg") { "image/jpeg" } else { "image/png" };
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
async fn read_screenshot_thumb(path: String) -> Result<String, String> {
    use base64::Engine;

    // Cache key: djb2 hash of path + modification time
    let mtime = tokio::fs::metadata(&path).await
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
        .unwrap_or(0);
    let hash: u64 = path.bytes().fold(5381u64, |a, b| a.wrapping_mul(33).wrapping_add(b as u64));
    let cache_dir = std::env::temp_dir().join("nova-launcher-thumbs");
    let cache_path = cache_dir.join(format!("{:016x}_{}.jpg", hash, mtime));

    // Return cached thumbnail if available
    if let Ok(cached) = tokio::fs::read(&cache_path).await {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&cached);
        return Ok(format!("data:image/jpeg;base64,{}", b64));
    }

    // Generate thumbnail, save to disk cache, return
    let b64 = tokio::task::spawn_blocking(move || -> Result<String, String> {
        use image::codecs::jpeg::JpegEncoder;
        let img = image::open(&path).map_err(|e| e.to_string())?;
        let thumb = img.thumbnail(320, 180);
        let mut buf = Vec::new();
        let encoder = JpegEncoder::new_with_quality(&mut buf, 60);
        thumb.write_with_encoder(encoder).map_err(|e| e.to_string())?;
        let _ = std::fs::create_dir_all(&cache_dir);
        let _ = std::fs::write(&cache_path, &buf);
        Ok(base64::engine::general_purpose::STANDARD.encode(&buf))
    }).await.map_err(|e| e.to_string())??;
    Ok(format!("data:image/jpeg;base64,{}", b64))
}

#[tauri::command]
async fn delete_screenshot(path: String) -> Result<(), String> {
    tokio::fs::remove_file(&path).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn compress_screenshot(path: String) -> Result<String, String> {
    use image::codecs::jpeg::JpegEncoder;
    let new_path_str = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let img = image::open(&path).map_err(|e| e.to_string())?;
        let p = std::path::Path::new(&path);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("screenshot");
        let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from("."));
        let new_path = dir.join(format!("{}.jpg", stem));
        let file = std::fs::File::create(&new_path).map_err(|e| e.to_string())?;
        let mut writer = std::io::BufWriter::new(file);
        let encoder = JpegEncoder::new_with_quality(&mut writer, 85);
        img.write_with_encoder(encoder).map_err(|e| e.to_string())?;
        drop(writer);
        // Remove original PNG if path differs
        if new_path.to_string_lossy() != path.as_str() {
            std::fs::remove_file(&path).ok();
        }
        Ok(new_path.display().to_string())
    }).await.map_err(|e| e.to_string())??;
    Ok(new_path_str)
}

/// Upload image bytes to catbox.moe (free, anonymous) and return the HTTPS URL.
/// Returns None if the upload fails — callers should handle gracefully.
async fn upload_icon_to_catbox(http: &reqwest::Client, bytes: Vec<u8>, ext: &str) -> Option<String> {
    let mime = match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "gif"          => "image/gif",
        "webp"         => "image/webp",
        _              => "image/png",
    };
    let filename = format!("icon.{}", ext);
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename)
        .mime_str(mime).ok()?;
    let form = reqwest::multipart::Form::new()
        .text("reqtype", "fileupload")
        .part("fileToUpload", part);
    let resp = http
        .post("https://catbox.moe/user/api.php")
        .header("User-Agent", "NovaLauncher/0.1.0")
        .multipart(form)
        .send().await.ok()?;
    let url = resp.text().await.ok()?;
    let url = url.trim().to_string();
    if url.starts_with("https://") { Some(url) } else { None }
}

#[tauri::command]
async fn save_instance_icon(
    instance_id: String,
    base64_data: String,
    extension: String,
    source_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<Config, String> {
    use base64::Engine;
    let (instance_dir, http) = {
        let cfg = lock_config(&state)?;
        let inst = cfg.instances.iter().find(|i| i.id == instance_id)
            .ok_or("Instanz nicht gefunden")?;
        (inst.resolved_game_dir(), state.http.clone())
    };
    tokio::fs::create_dir_all(&instance_dir).await.map_err(|e| e.to_string())?;

    let ext = if extension.is_empty() { "png".to_string() } else { extension };
    let icon_filename = format!("icon.{}", ext);
    let icon_path = instance_dir.join(&icon_filename);

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&base64_data)
        .map_err(|e| e.to_string())?;
    tokio::fs::write(&icon_path, &bytes).await.map_err(|e| e.to_string())?;

    // Resolve which HTTPS URL to store for Discord RPC:
    // 1. Use caller-provided source URL if it's already HTTPS (e.g. Modrinth CDN)
    // 2. Otherwise upload the image bytes to catbox.moe
    let icon_url = if let Some(url) = source_url.filter(|u| u.starts_with("https://")) {
        Some(url)
    } else {
        upload_icon_to_catbox(&http, bytes, &ext).await
    };

    let mut cfg = lock_config(&state)?;
    if let Some(inst) = cfg.instances.iter_mut().find(|i| i.id == instance_id) {
        inst.icon_path = Some(icon_path.display().to_string());
        inst.icon_url = icon_url;
    }
    cfg.save().map_err(|e| e.to_string())?;
    Ok(cfg.clone())
}

#[tauri::command]
async fn create_instance(
    name: String,
    mc_version: String,
    loader: String,
    loader_version: Option<String>,
    icon_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<Config, String> {
    use crate::config::ModLoader;
    let mut config = lock_config(&state)?;
    let n = config.instances.len() + 1;
    let mut inst = Instance::new(if name.is_empty() { format!("Instanz {}", n) } else { name });
    inst.version = Some(mc_version);
    inst.loader = match loader.as_str() {
        "fabric" => ModLoader::Fabric,
        "forge" => ModLoader::Forge,
        "neoforge" => ModLoader::Neoforge,
        "quilt" => ModLoader::Quilt,
        "paper" => ModLoader::Paper,
        _ => ModLoader::Vanilla,
    };
    inst.loader_version = loader_version;

    // Apply global defaults from config
    inst.ram_min_mb = config.global_ram_min_mb;
    inst.ram_max_mb = config.global_ram_max_mb;
    inst.game_width = config.default_game_width;
    inst.game_height = config.default_game_height;
    inst.fullscreen = config.default_fullscreen;
    inst.custom_jvm_args = config.default_custom_jvm_args.clone();
    inst.env_vars = config.default_env_vars.clone();
    inst.pre_launch_hook = config.default_pre_launch_hook.clone();
    inst.wrapper_command = config.default_wrapper_command.clone();
    inst.post_exit_hook = config.default_post_exit_hook.clone();

    // Set isolated game directory
    let game_dir = instances_base_dir().join(&inst.id);
    inst.game_dir = Some(game_dir.display().to_string());

    // Store icon URL if provided (e.g. from Modrinth modpack browser)
    inst.icon_url = icon_url.filter(|u| u.starts_with("https://"));

    let id = inst.id.clone();
    config.instances.push(inst);
    config.active_instance_id = Some(id);
    config.save().map_err(|e| e.to_string())?;
    sync::apply_syncs(&config);
    Ok(config.clone())
}

#[tauri::command]
async fn prepare_instance(
    instance_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (config, version_entry) = {
        let cfg = lock_config(&state)?;
        let inst = cfg.instances.iter()
            .find(|i| i.id == instance_id)
            .ok_or("Instanz nicht gefunden")?
            .clone();
        let version_id = inst.version.clone().ok_or("Keine Version ausgewählt")?;
        let manifest_guard = state.manifest.lock().map_err(|e| e.to_string())?;
        let manifest = manifest_guard.as_ref().ok_or("Manifest nicht geladen")?;
        let entry = manifest.versions.iter()
            .find(|v| v.id == version_id)
            .cloned()
            .ok_or("Version nicht im Manifest gefunden")?;
        drop(manifest_guard);
        let game_dir = inst.resolved_game_dir();
        std::fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
        let launch_config = Config {
            active_instance_id: Some(inst.id.clone()),
            instances: vec![inst],
            ..cfg.clone()
        };
        (launch_config, entry)
    };

    let http = state.http.clone();
    let ip = state.install_progress.clone();

    {
        let mut p = ip.lock().map_err(|e| e.to_string())?;
        p.insert(instance_id.clone(), InstallProgress {
            step: "Starte Installation…".into(),
            percent: 0.0,
            done: false,
            error: None,
        });
    }

    let (tx, mut rx) = futures::channel::mpsc::unbounded::<LaunchEvent>();
    let opts = LaunchOptions {
        client: http,
        config,
        version_entry,
        account: StoredAccount::default(),
        tx,
        install_only: true,
        online: true, // kein Token-Refresh bei install_only
        quickplay_args: vec![],
    };

    tokio::spawn(async move {
        install_and_launch(opts).await;
    });

    let ip_clone = ip.clone();
    let iid = instance_id.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.next().await {
            let mut p = match ip_clone.lock() { Ok(l) => l, Err(_) => break };
            match event {
                LaunchEvent::Progress { step, percent } => {
                    p.insert(iid.clone(), InstallProgress { step, percent, done: false, error: None });
                }
                LaunchEvent::Exited(_) => {
                    p.insert(iid.clone(), InstallProgress {
                        step: "Installation abgeschlossen".into(),
                        percent: 1.0,
                        done: true,
                        error: None,
                    });
                    break;
                }
                LaunchEvent::Failed(e) => {
                    p.insert(iid.clone(), InstallProgress {
                        step: "Fehler".into(),
                        percent: 0.0,
                        done: true,
                        error: Some(e),
                    });
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn get_install_progress(
    instance_id: String,
    state: State<'_, AppState>,
) -> Result<Option<InstallProgress>, String> {
    let mut p = state.install_progress.lock().map_err(|e| e.to_string())?;
    let progress = p.get(&instance_id).cloned();
    // Remove the entry once the frontend has seen the completed result
    if progress.as_ref().map(|v| v.done).unwrap_or(false) {
        p.remove(&instance_id);
    }
    Ok(progress)
}

async fn read_content_dir(dir: &std::path::Path, _exts: &[&str]) -> Vec<ModInfo> {
    let mut items = Vec::new();
    if !dir.exists() { return items; }
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else { return items };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let is_dir = entry.metadata().await.map(|m| m.is_dir()).unwrap_or(false);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if !is_dir && ext != "zip" && ext != "jar" { continue; }
        let filename = entry.file_name().to_string_lossy().to_string();
        let size_bytes = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
        let meta_path = dir.join(format!("{}.nova.json", filename));
        let meta: Option<ModMeta> = if meta_path.exists() {
            tokio::fs::read_to_string(&meta_path).await.ok()
                .and_then(|s| serde_json::from_str(&s).ok())
        } else { None };
        items.push(ModInfo {
            filename,
            size_bytes,
            title: meta.as_ref().map(|m| m.title.clone()),
            icon_url: meta.as_ref().and_then(|m| m.icon_url.clone()),
            project_id: meta.as_ref().map(|m| m.project_id.clone()),
            version_id: meta.as_ref().map(|m| m.version_id.clone()),
            enabled: true,
        });
    }
    items.sort_by(|a, b| a.filename.cmp(&b.filename));
    items
}

#[tauri::command]
async fn get_instance_details(
    instance_id: String,
    state: State<'_, AppState>,
) -> Result<InstanceDetails, String> {
    let instance = {
        let config = lock_config(&state)?;
        config.instances.iter().find(|i| i.id == instance_id).cloned()
            .ok_or_else(|| format!("Instanz {} nicht gefunden", instance_id))?
    };

    let game_dir = instance.resolved_game_dir();
    let game_dir_str = game_dir.display().to_string();

    // Create dir if missing
    let _ = tokio::fs::create_dir_all(&game_dir).await;

    // ── Mods ──────────────────────────────────────────────────────────────
    let mut mods = Vec::new();
    let mods_dir = game_dir.join("mods");
    if mods_dir.exists() {
        if let Ok(mut entries) = tokio::fs::read_dir(&mods_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let filename = entry.file_name().to_string_lossy().to_string();
                // Accept .jar (enabled) and .jar.disabled (disabled)
                let (is_mod, enabled) = if filename.ends_with(".jar") {
                    (true, true)
                } else if filename.ends_with(".jar.disabled") {
                    (true, false)
                } else {
                    (false, false)
                };
                if is_mod {
                    let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
                    // Meta file uses the base .jar name (strip .disabled suffix)
                    let base = if enabled { filename.clone() } else {
                        filename.strip_suffix(".disabled").unwrap_or(&filename).to_string()
                    };
                    let meta_path = mods_dir.join(format!("{}.nova.json", base));
                    let meta: Option<ModMeta> = if meta_path.exists() {
                        tokio::fs::read_to_string(&meta_path).await.ok()
                            .and_then(|s| serde_json::from_str(&s).ok())
                    } else {
                        None
                    };
                    mods.push(ModInfo {
                        filename,
                        size_bytes: size,
                        title: meta.as_ref().map(|m| m.title.clone()),
                        icon_url: meta.as_ref().and_then(|m| m.icon_url.clone()),
                        project_id: meta.as_ref().map(|m| m.project_id.clone()),
                        version_id: meta.as_ref().map(|m| m.version_id.clone()),
                        enabled,
                    });
                }
            }
        }
    }
    mods.sort_by(|a, b| a.filename.cmp(&b.filename));

    // ── Resource Packs ────────────────────────────────────────────────────
    let resourcepacks = read_content_dir(&game_dir.join("resourcepacks"), &["zip", "folder"]).await;

    // ── Shader Packs ──────────────────────────────────────────────────────
    let shaderpacks = read_content_dir(&game_dir.join("shaderpacks"), &["zip"]).await;

    // ── Data Packs ────────────────────────────────────────────────────────
    // Primary: instance-level datapacks dir (where install_content writes)
    let mut datapacks = read_content_dir(&game_dir.join("datapacks"), &["zip", "folder"]).await;
    // Also collect from per-world datapacks (vanilla location)
    let saves_dir_dp = game_dir.join("saves");
    if saves_dir_dp.exists() {
        if let Ok(mut world_entries) = tokio::fs::read_dir(&saves_dir_dp).await {
            while let Ok(Some(world_entry)) = world_entries.next_entry().await {
                if world_entry.metadata().await.map(|m| m.is_dir()).unwrap_or(false) {
                    let dp_dir = world_entry.path().join("datapacks");
                    let mut found = read_content_dir(&dp_dir, &["zip", "folder"]).await;
                    datapacks.append(&mut found);
                }
            }
        }
    }
    datapacks.dedup_by(|a, b| a.filename == b.filename);
    datapacks.sort_by(|a, b| a.filename.cmp(&b.filename));

    // ── Worlds ────────────────────────────────────────────────────────────
    let mut worlds = Vec::new();
    let saves_dir = game_dir.join("saves");
    if saves_dir.exists() {
        if let Ok(mut entries) = tokio::fs::read_dir(&saves_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.metadata().await.map(|m| m.is_dir()).unwrap_or(false) {
                    let last_played_ms = entry.metadata().await.ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64);
                    let world_path = entry.path();
                    let icon = worlds::read_world_icon(&world_path);
                    let display_name = worlds::read_level_name(&world_path);
                    worlds.push(WorldInfo {
                        name: entry.file_name().to_string_lossy().to_string(),
                        display_name,
                        last_played_ms,
                        icon,
                    });
                }
            }
        }
    }
    worlds.sort_by(|a, b| b.last_played_ms.cmp(&a.last_played_ms));

    // ── Servers ───────────────────────────────────────────────────────────
    let servers = worlds::read_servers(&game_dir);

    // ── Screenshots ───────────────────────────────────────────────────────
    let mut screenshots = Vec::new();
    let screenshots_dir = game_dir.join("screenshots");
    if screenshots_dir.exists() {
        if let Ok(mut entries) = tokio::fs::read_dir(&screenshots_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
                    let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
                    screenshots.push(ScreenshotInfo {
                        filename: entry.file_name().to_string_lossy().to_string(),
                        path: path.display().to_string(),
                        size_bytes: size,
                    });
                }
            }
        }
    }
    screenshots.sort_by(|a, b| b.filename.cmp(&a.filename)); // newest first (by name usually)

    // ── Log tail ──────────────────────────────────────────────────────────
    let log_path = game_dir.join("logs").join("latest.log");
    let log_tail = if log_path.exists() {
        tokio::task::spawn_blocking(move || -> String {
            use std::io::{Read, Seek, SeekFrom};
            let mut file = match std::fs::File::open(&log_path) {
                Ok(f) => f,
                Err(_) => return String::new(),
            };
            let size = file.seek(SeekFrom::End(0)).unwrap_or(0);
            file.seek(SeekFrom::Start(size.saturating_sub(64 * 1024))).ok();
            let mut buf = String::new();
            file.read_to_string(&mut buf).ok();
            let lines: Vec<&str> = buf.lines().collect();
            let start = lines.len().saturating_sub(200);
            lines[start..].join("\n")
        }).await.unwrap_or_default()
    } else {
        String::new()
    };

    let total_mods = mods.len();
    let total_worlds = worlds.len();

    Ok(InstanceDetails {
        instance,
        game_dir: game_dir_str,
        mods,
        resourcepacks,
        shaderpacks,
        datapacks,
        worlds,
        servers,
        screenshots,
        log_tail,
        total_mods,
        total_worlds,
    })
}

#[derive(serde::Serialize)]
pub struct ServerStatus {
    pub online: bool,
    pub motd_html: Option<String>,
    pub players_online: u32,
    pub players_max: u32,
    pub version: Option<String>,
    pub icon: Option<String>, // base64, no data: prefix
}

#[tauri::command]
async fn ping_server(address: String, state: State<'_, AppState>) -> Result<ServerStatus, String> {
    // Parse host and port from address
    let (host, port) = if let Some(colon) = address.rfind(':') {
        let h = &address[..colon];
        let p = address[colon+1..].parse::<u16>().unwrap_or(25565);
        (h.to_string(), p)
    } else {
        (address.clone(), 25565u16)
    };

    // Try mcsrvstat.us first (works for public servers)
    let url = format!("https://api.mcsrvstat.us/3/{}", address);
    if let Ok(resp) = state.http.get(&url)
        .header("User-Agent", "Nova-Launcher/1.0 (Minecraft Launcher)")
        .send().await
    {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            let online = json.get("online").and_then(|v| v.as_bool()).unwrap_or(false);
            if online {
                let motd_html = json.pointer("/motd/html")
                    .and_then(|v| v.as_array())
                    .map(|lines| lines.iter().filter_map(|l| l.as_str()).collect::<Vec<_>>().join("<br>"));
                let players_online = json.pointer("/players/online").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let players_max   = json.pointer("/players/max").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let version       = json.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
                let icon          = json.get("icon").and_then(|v| v.as_str())
                    .map(|s| s.strip_prefix("data:image/png;base64,").unwrap_or(s).to_string());
                return Ok(ServerStatus { online: true, motd_html, players_online, players_max, version, icon });
            }
        }
    }

    // Fallback: direct SLP (for private/LAN servers not reachable by mcsrvstat.us)
    match worlds::ping_server(&host, port).await {
        Ok(r) => Ok(ServerStatus {
            online: true,
            motd_html: Some(r.motd_html),
            players_online: r.online,
            players_max: r.max,
            version: r.version,
            icon: r.favicon,
        }),
        Err(_) => Ok(ServerStatus { online: false, motd_html: None, players_online: 0, players_max: 0, version: None, icon: None }),
    }
}

// ─── Skin Preset Commands ─────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct SkinPresetsResponse {
    presets: Vec<SkinPreset>,
    active_id: Option<String>,
}

#[tauri::command]
fn get_skin_presets(state: State<'_, AppState>) -> Result<SkinPresetsResponse, String> {
    let config = lock_config(&state)?;
    Ok(SkinPresetsResponse {
        presets: config.skin_presets.clone(),
        active_id: config.active_skin_preset_id.clone(),
    })
}

#[tauri::command]
fn save_skin_preset(preset: SkinPreset, state: State<'_, AppState>) -> Result<(), String> {
    let mut config = lock_config(&state)?;
    if let Some(existing) = config.skin_presets.iter_mut().find(|p| p.id == preset.id) {
        *existing = preset;
    } else {
        config.skin_presets.push(preset);
    }
    config.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_skin_preset(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut config = lock_config(&state)?;
    config.skin_presets.retain(|p| p.id != id);
    if config.active_skin_preset_id.as_deref() == Some(id.as_str()) {
        config.active_skin_preset_id = None;
    }
    config.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_active_skin_preset(id: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut config = lock_config(&state)?;
    config.active_skin_preset_id = id;
    config.save().map_err(|e| e.to_string())
}

/// Helper: load account from keyring, refresh token if expired, return valid StoredAccount.
async fn get_valid_account(state: &AppState) -> Result<crate::auth::storage::StoredAccount, String> {
    use crate::auth::microsoft::{refresh_ms_token, complete_auth};
    use crate::auth::storage::{AccountStore, StoredAccount};

    let uuid = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.active_account_uuid.clone().ok_or("Kein aktiver Account")?
    };

    let account = tokio::task::spawn_blocking(move || {
        let store = AccountStore::load().map_err(|e| e.to_string())?;
        store.get(&uuid).cloned().ok_or_else(|| "Account nicht gefunden".to_string())
    }).await.map_err(|e| e.to_string())??;

    if !account.is_token_expired() {
        return Ok(account);
    }

    // Refresh
    let (ms_access, ms_refresh, ms_expires) = refresh_ms_token(&state.http, &account.refresh_token)
        .await.map_err(|e| e.to_string())?;
    let auth = complete_auth(&state.http, &ms_access, &ms_refresh, ms_expires)
        .await.map_err(|e| e.to_string())?;

    let updated = StoredAccount {
        uuid: auth.uuid,
        username: auth.username,
        minecraft_token: auth.minecraft_token,
        refresh_token: auth.refresh_token,
        token_expires_at: auth.expires_at,
        offline: false,
    };

    // Save updated account to keyring
    tokio::task::spawn_blocking({
        let updated = updated.clone();
        move || -> Result<(), String> {
            let mut store = AccountStore::load().map_err(|e| e.to_string())?;
            store.add_or_update(updated);
            store.save().map_err(|e| e.to_string())?;
            Ok(())
        }
    }).await.map_err(|e| e.to_string())??;

    Ok(updated)
}

#[tauri::command]
async fn import_current_skin_as_preset(state: State<'_, AppState>) -> Result<SkinPreset, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let account = get_valid_account(&state).await?;
    let token = &account.minecraft_token;
    let username = &account.username;

    // Fetch /minecraft/profile for cape UUID
    let mc_resp = state.http
        .get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "Nova-Launcher/1.0")
        .send().await.map_err(|e| e.to_string())?;
    if mc_resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err("RATE_LIMIT".to_string());
    }
    let mc_profile: serde_json::Value = mc_resp.json().await.map_err(|e| e.to_string())?;

    let active_cape_id = mc_profile["capes"]
        .as_array()
        .and_then(|capes| capes.iter().find(|c| c["state"] == "ACTIVE"))
        .and_then(|c| c["id"].as_str())
        .map(|s| s.to_string());

    // Fetch session server for texture URLs
    let uuid_nodash = account.uuid.replace('-', "");
    let session_url = format!(
        "https://sessionserver.mojang.com/session/minecraft/profile/{}",
        uuid_nodash
    );

    let sess_resp = state.http
        .get(&session_url)
        .header("User-Agent", "Nova-Launcher/1.0")
        .send().await.map_err(|e| e.to_string())?;
    if sess_resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err("RATE_LIMIT".to_string());
    }
    let profile: serde_json::Value = sess_resp.json().await.map_err(|e| e.to_string())?;

    let tex_b64 = profile["properties"]
        .as_array()
        .and_then(|arr| arr.iter().find(|p| p["name"] == "textures"))
        .and_then(|p| p["value"].as_str())
        .ok_or("Keine Texture-Daten im Profil")?;

    let tex_json = STANDARD.decode(tex_b64).map_err(|e| e.to_string())?;
    let textures: serde_json::Value = serde_json::from_slice(&tex_json)
        .map_err(|e| e.to_string())?;

    let skin_url = textures["textures"]["SKIN"]["url"]
        .as_str()
        .ok_or("Kein Skin in Profil gefunden")?
        .to_string();

    let is_slim = textures["textures"]["SKIN"]["metadata"]["model"]
        .as_str() == Some("slim");

    let cape_url = textures["textures"]["CAPE"]["url"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| {
            active_cape_id.as_ref().and_then(|cid| {
                mc_profile["capes"].as_array()?.iter()
                    .find(|c| c["id"].as_str() == Some(cid.as_str()))
                    .and_then(|c| c["url"].as_str())
                    .map(|s| s.to_string())
            })
        });

    // Download skin PNG
    let skin_bytes = state.http.get(&skin_url)
        .send().await.map_err(|e| e.to_string())?
        .bytes().await.map_err(|e| e.to_string())?;
    let skin_data = STANDARD.encode(&skin_bytes);

    // Download cape PNG (optional)
    let cape_data = if let Some(ref curl) = cape_url {
        if let Ok(resp) = state.http.get(curl).send().await {
            resp.bytes().await.ok().map(|b| STANDARD.encode(&b))
        } else { None }
    } else { None };

    let model = if is_slim { "slim".to_string() } else { "classic".to_string() };

    // Check if an existing preset already has the same skin pixels — update it instead of
    // creating a duplicate (handles manually-added presets that lack skin_url).
    let existing_id = {
        let cfg = lock_config(&state)?;
        cfg.skin_presets.iter()
            .find(|p| p.skin_data == skin_data)
            .map(|p| p.id.clone())
    };

    if let Some(eid) = existing_id {
        let updated = {
            let mut cfg = lock_config(&state)?;
            if let Some(p) = cfg.skin_presets.iter_mut().find(|p| p.id == eid) {
                p.skin_url = Some(skin_url.clone());
                p.model = model.clone();
                p.cape_id = active_cape_id.clone();
                // Clear cape if no longer equipped; update if download succeeded
                if active_cape_id.is_none() {
                    p.cape_data = None;
                } else if cape_data.is_some() {
                    p.cape_data = cape_data.clone();
                }
            }
            cfg.active_skin_preset_id = Some(eid.clone());
            cfg.save().map_err(|e| e.to_string())?;
            cfg.skin_presets.iter().find(|p| p.id == eid).cloned()
        };
        if let Some(preset) = updated {
            return Ok(preset);
        }
    }

    let preset = SkinPreset {
        id: uuid::Uuid::new_v4().to_string(),
        name: format!("{}s Skin", username),
        model,
        skin_data,
        cape_data,
        skin_url: Some(skin_url),
        cape_id: active_cape_id,
    };

    // Persist preset + mark as active
    {
        let mut cfg = lock_config(&state)?;
        cfg.skin_presets.push(preset.clone());
        cfg.active_skin_preset_id = Some(preset.id.clone());
        cfg.save().map_err(|e| e.to_string())?;
    }

    Ok(preset)
}

#[derive(serde::Serialize)]
struct CurrentSkinInfo {
    skin_data: String,
    cape_data: Option<String>,
    model: String,
    skin_url: String,
    cape_url: Option<String>,
    cape_id: Option<String>,
    matched_preset_id: Option<String>,
}

#[tauri::command]
async fn get_current_mojang_skin(state: State<'_, AppState>) -> Result<CurrentSkinInfo, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let account = get_valid_account(&state).await?;
    let token = &account.minecraft_token;

    // Fetch /minecraft/profile for cape UUID
    let mc_resp = state.http
        .get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "Nova-Launcher/1.0")
        .send().await.map_err(|e| e.to_string())?;
    if mc_resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err("RATE_LIMIT".to_string());
    }
    let mc_profile: serde_json::Value = mc_resp.json().await.map_err(|e| e.to_string())?;

    let active_cape_id = mc_profile["capes"]
        .as_array()
        .and_then(|capes| capes.iter().find(|c| c["state"] == "ACTIVE"))
        .and_then(|c| c["id"].as_str())
        .map(|s| s.to_string());

    // Fetch session server for texture URLs
    let uuid_nodash = account.uuid.replace('-', "");
    let session_url = format!(
        "https://sessionserver.mojang.com/session/minecraft/profile/{}",
        uuid_nodash
    );

    let sess_resp = state.http
        .get(&session_url)
        .header("User-Agent", "Nova-Launcher/1.0")
        .send().await.map_err(|e| e.to_string())?;
    if sess_resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err("RATE_LIMIT".to_string());
    }
    let profile: serde_json::Value = sess_resp.json().await.map_err(|e| e.to_string())?;

    let tex_b64 = profile["properties"]
        .as_array()
        .and_then(|arr| arr.iter().find(|p| p["name"] == "textures"))
        .and_then(|p| p["value"].as_str())
        .ok_or("Keine Texture-Daten im Profil")?;

    let tex_json = STANDARD.decode(tex_b64).map_err(|e| e.to_string())?;
    let textures: serde_json::Value = serde_json::from_slice(&tex_json)
        .map_err(|e| e.to_string())?;

    let skin_url = textures["textures"]["SKIN"]["url"]
        .as_str()
        .ok_or("Kein Skin in Profil gefunden")?
        .to_string();

    let is_slim = textures["textures"]["SKIN"]["metadata"]["model"]
        .as_str() == Some("slim");

    let cape_url = textures["textures"]["CAPE"]["url"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| {
            active_cape_id.as_ref().and_then(|cid| {
                mc_profile["capes"].as_array()?.iter()
                    .find(|c| c["id"].as_str() == Some(cid.as_str()))
                    .and_then(|c| c["url"].as_str())
                    .map(|s| s.to_string())
            })
        });

    let model = if is_slim { "slim".to_string() } else { "classic".to_string() };

    // Download skin PNG
    let skin_bytes = state.http.get(&skin_url)
        .send().await.map_err(|e| e.to_string())?
        .bytes().await.map_err(|e| e.to_string())?;
    let skin_data = STANDARD.encode(&skin_bytes);

    // Download cape PNG (optional)
    let cape_data = if let Some(ref curl) = cape_url {
        if let Ok(resp) = state.http.get(curl).send().await {
            resp.bytes().await.ok().map(|b| STANDARD.encode(&b))
        } else { None }
    } else { None };

    // Match against existing presets
    let matched_preset_id = {
        let cfg = lock_config(&state)?;
        cfg.skin_presets.iter().find(|p| {
            p.skin_url.as_deref() == Some(&skin_url)
                && match &active_cape_id {
                    Some(cid) => p.cape_id.as_deref() == Some(cid.as_str()),
                    None => p.cape_id.is_none() && p.cape_data.is_none(),
                }
        }).map(|p| p.id.clone())
    };

    Ok(CurrentSkinInfo {
        skin_data,
        cape_data,
        model,
        skin_url,
        cape_url,
        cape_id: active_cape_id,
        matched_preset_id,
    })
}

#[derive(serde::Serialize)]
struct OwnedCape {
    id: String,
    alias: String,
    state: String,   // "ACTIVE" | "INACTIVE"
    cape_data: String, // base64 PNG
}

#[tauri::command]
async fn get_owned_capes(state: State<'_, AppState>) -> Result<Vec<OwnedCape>, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let account = get_valid_account(&state).await?;

    let mc_resp = state.http
        .get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {}", account.minecraft_token))
        .header("User-Agent", "Nova-Launcher/1.0")
        .send().await.map_err(|e| e.to_string())?;
    if mc_resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err("RATE_LIMIT".to_string());
    }
    let mc_profile: serde_json::Value = mc_resp.json().await.map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    let capes_array: &[serde_json::Value] = mc_profile["capes"].as_array().map(|v| v.as_slice()).unwrap_or_default();

    for cape in capes_array {
        let id = cape["id"].as_str().unwrap_or("");
        let url = cape["url"].as_str().unwrap_or("");
        if id.is_empty() || url.is_empty() { continue; }
        let id = id.to_string();
        let alias = cape["alias"].as_str().unwrap_or("Cape").to_string();
        let cape_state = cape["state"].as_str().unwrap_or("INACTIVE").to_string();
        let url = url.to_string();

        let cape_data = match state.http.get(&url).send().await {
            Ok(resp) => match resp.bytes().await {
                Ok(b) => STANDARD.encode(&b),
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        result.push(OwnedCape { id, alias, state: cape_state, cape_data });
    }

    Ok(result)
}

#[tauri::command]
async fn apply_skin_to_mojang(id: String, state: State<'_, AppState>) -> Result<(), String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    // Get preset from config
    let preset = {
        let cfg = lock_config(&state)?;
        cfg.skin_presets.iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| format!("Preset {} nicht gefunden", id))?
    };

    let account = get_valid_account(&state).await?;
    let token = &account.minecraft_token;

    // Decode skin PNG bytes
    let skin_bytes = STANDARD.decode(&preset.skin_data).map_err(|e| e.to_string())?;

    // Upload skin via multipart POST
    let part = reqwest::multipart::Part::bytes(skin_bytes)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text("variant", preset.model.clone())
        .part("file", part);

    let resp = state.http
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send().await.map_err(|e| e.to_string())?;

    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err("RATE_LIMIT".to_string());
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Skin-Upload fehlgeschlagen: {} – {}", status, body));
    }

    // Capture the new skin URL from upload response so future matching works
    let new_skin_url: Option<String> = resp.json::<serde_json::Value>().await.ok()
        .and_then(|body| {
            body["skins"].as_array()?.iter()
                .find(|s| s["state"] == "ACTIVE")
                .and_then(|s| s["url"].as_str())
                .map(|s| s.to_string())
        });

    // Handle cape
    if let Some(ref cape_id) = preset.cape_id {
        let body = serde_json::json!({ "capeId": cape_id });
        let resp = state.http
            .put("https://api.minecraftservices.com/minecraft/profile/capes/active")
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Cape-Aktivierung fehlgeschlagen: {} – {}", status, body));
        }
    } else {
        // Hide all capes
        let _ = state.http
            .delete("https://api.minecraftservices.com/minecraft/profile/capes/active")
            .header("Authorization", format!("Bearer {}", token))
            .send().await;
    }

    // Update active preset + persist skin_url so future matching works
    {
        let mut cfg = lock_config(&state)?;
        if let Some(ref url) = new_skin_url {
            if let Some(p) = cfg.skin_presets.iter_mut().find(|p| p.id == id) {
                p.skin_url = Some(url.clone());
            }
        }
        cfg.active_skin_preset_id = Some(id);
        cfg.save().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
async fn open_instance_folder(instance_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let game_dir = {
        let config = lock_config(&state)?;
        config.instances.iter().find(|i| i.id == instance_id)
            .map(|i| i.resolved_game_dir())
            .ok_or_else(|| format!("Instanz {} nicht gefunden", instance_id))?
    };
    std::fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
    open::that(&game_dir).map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_modrinth(
    query: String,
    game_version: Option<String>,
    loader: Option<String>,
    offset: u64,
    state: State<'_, AppState>,
) -> Result<ModrinthSearchResult, String> {
    let encoded_query: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let mut url = format!(
        "https://api.modrinth.com/v2/search?query={}&limit=20&offset={}&index=relevance",
        encoded_query, offset
    );
    let mut facets: Vec<String> = vec!["[\"project_type:mod\"]".to_string()];
    if let Some(gv) = &game_version {
        if !gv.is_empty() { facets.push(format!("[\"versions:{}\"]", gv)); }
    }
    if let Some(l) = &loader {
        if !l.is_empty() && l != "vanilla" { facets.push(format!("[\"categories:{}\"]", l)); }
    }
    url.push_str(&format!("&facets=[{}]", facets.join(",")));
    cached_get::<ModrinthSearchResult>(&state.http, &state.cache, &url, 300).await
}

#[tauri::command]
async fn search_modpacks(
    query: String,
    game_version: Option<String>,
    offset: u64,
    state: State<'_, AppState>,
) -> Result<ModrinthSearchResult, String> {
    let encoded_query: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let mut url = format!(
        "https://api.modrinth.com/v2/search?query={}&limit=20&offset={}&index=downloads",
        encoded_query, offset
    );
    let mut facets: Vec<String> = vec!["[\"project_type:modpack\"]".to_string()];
    if let Some(gv) = &game_version {
        if !gv.is_empty() { facets.push(format!("[\"versions:{}\"]", gv)); }
    }
    url.push_str(&format!("&facets=[{}]", facets.join(",")));
    cached_get::<ModrinthSearchResult>(&state.http, &state.cache, &url, 300).await
}

#[tauri::command]
async fn get_modrinth_versions(
    project_id: String,
    game_version: Option<String>,
    loader: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ModrinthVersion>, String> {
    let mut url = format!("https://api.modrinth.com/v2/project/{}/version?", project_id);
    if let Some(gv) = &game_version { if !gv.is_empty() { url.push_str(&format!("game_versions=[\"{}\"]&", gv)); } }
    if let Some(l) = &loader { if !l.is_empty() && l != "vanilla" { url.push_str(&format!("loaders=[\"{}\"]&", l)); } }
    cached_get::<Vec<ModrinthVersion>>(&state.http, &state.cache, &url, 1800).await
}

#[tauri::command]
async fn install_mod(
    project_id: String,
    version_id: String,
    instance_id: Option<String>,
    title: Option<String>,
    icon_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let url = format!("https://api.modrinth.com/v2/version/{}", version_id);
    let version: ModrinthVersion = state.http.get(&url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let file = version.files.iter().find(|f| f.primary).or_else(|| version.files.first())
        .ok_or("Keine Datei gefunden")?;

    let game_dir = {
        let config = lock_config(&state)?;
        if let Some(iid) = &instance_id {
            config.instances.iter().find(|i| &i.id == iid)
                .map(|i| i.resolved_game_dir())
                .unwrap_or_else(|| config.game_dir())
        } else {
            config.game_dir()
        }
    };

    let mods_dir = game_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir).await.map_err(|e| e.to_string())?;
    let dest = mods_dir.join(&file.filename);

    let bytes = state.http.get(&file.url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .bytes().await.map_err(|e| e.to_string())?;
    tokio::fs::write(&dest, &bytes).await.map_err(|e| e.to_string())?;

    // Save metadata alongside mod
    let meta = ModMeta {
        project_id: project_id.clone(),
        version_id: version_id.clone(),
        title: title.unwrap_or_else(|| file.filename.clone()),
        icon_url,
    };
    if let Ok(json) = serde_json::to_string(&meta) {
        let meta_path = mods_dir.join(format!("{}.nova.json", file.filename));
        let _ = tokio::fs::write(&meta_path, json).await;
    }

    Ok(file.filename.clone())
}

#[tauri::command]
async fn install_modpack(
    version_id: String,
    name: Option<String>,
    icon_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // 1. Fetch version metadata to get the .mrpack download URL
    let url = format!("https://api.modrinth.com/v2/version/{}", version_id);
    let version: ModrinthVersion = state.http.get(&url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let mrpack_file = version.files.iter()
        .find(|f| f.primary)
        .or_else(|| version.files.iter().find(|f| f.filename.ends_with(".mrpack")))
        .or_else(|| version.files.first())
        .ok_or("Keine .mrpack Datei gefunden")?;

    // 2. Download .mrpack (zip)
    let bytes = state.http.get(&mrpack_file.url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .bytes().await.map_err(|e| e.to_string())?
        .to_vec();

    // 3. Parse modrinth.index.json + collect overrides in spawn_blocking
    let (index, overrides) = tokio::task::spawn_blocking(move || -> Result<(MrpackIndex, Vec<(String, Vec<u8>)>), String> {
        use std::io::Read;
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;

        let index: MrpackIndex = {
            let mut f = archive.by_name("modrinth.index.json")
                .map_err(|_| "modrinth.index.json nicht gefunden".to_string())?;
            let mut s = String::new();
            f.read_to_string(&mut s).map_err(|e| e.to_string())?;
            serde_json::from_str(&s).map_err(|e| e.to_string())?
        };

        let mut overrides = Vec::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let entry_name = entry.name().to_string();
            let is_override = entry_name.starts_with("overrides/") || entry_name.starts_with("client-overrides/");
            if is_override && !entry_name.ends_with('/') {
                let rel = if entry_name.starts_with("overrides/") {
                    entry_name.trim_start_matches("overrides/").to_string()
                } else {
                    entry_name.trim_start_matches("client-overrides/").to_string()
                };
                if !rel.is_empty() {
                    let mut data = Vec::new();
                    entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
                    overrides.push((rel, data));
                }
            }
        }
        Ok((index, overrides))
    }).await.map_err(|e| e.to_string())??;

    // 4. Determine MC version + loader from dependencies
    let mc_version = index.dependencies.get("minecraft")
        .cloned()
        .ok_or("Keine Minecraft-Version im Modpack gefunden")?;

    let (loader_name, loader_ver) = if let Some(v) = index.dependencies.get("fabric-loader") {
        ("fabric", Some(v.clone()))
    } else if let Some(v) = index.dependencies.get("quilt-loader") {
        ("quilt", Some(v.clone()))
    } else if let Some(v) = index.dependencies.get("neoforge") {
        ("neoforge", Some(v.clone()))
    } else if let Some(v) = index.dependencies.get("forge") {
        ("forge", Some(v.clone()))
    } else {
        ("vanilla", None)
    };

    // 5. Create the instance
    let pack_name = name.unwrap_or_else(|| index.name.clone());
    let (instance_id, game_dir, icon_https) = {
        use crate::config::ModLoader;
        let mut config = lock_config(&state)?;
        let n = config.instances.len() + 1;
        let mut inst = Instance::new(if pack_name.is_empty() { format!("Modpack {}", n) } else { pack_name });
        inst.version = Some(mc_version);
        inst.loader = match loader_name {
            "fabric" => ModLoader::Fabric,
            "forge"  => ModLoader::Forge,
            "neoforge" => ModLoader::Neoforge,
            "quilt"  => ModLoader::Quilt,
            _        => ModLoader::Vanilla,
        };
        inst.loader_version = loader_ver;
        inst.ram_min_mb = config.global_ram_min_mb;
        inst.ram_max_mb = config.global_ram_max_mb;
        inst.game_width = config.default_game_width;
        inst.game_height = config.default_game_height;
        inst.fullscreen = config.default_fullscreen;
        inst.custom_jvm_args = config.default_custom_jvm_args.clone();
        inst.env_vars = config.default_env_vars.clone();
        inst.pre_launch_hook = config.default_pre_launch_hook.clone();
        inst.wrapper_command = config.default_wrapper_command.clone();
        inst.post_exit_hook = config.default_post_exit_hook.clone();
        let gdir = instances_base_dir().join(&inst.id);
        inst.game_dir = Some(gdir.display().to_string());
        let icon_https = icon_url.filter(|u| u.starts_with("https://"));
        inst.icon_url = icon_https.clone();
        let id = inst.id.clone();
        config.instances.push(inst);
        config.save().map_err(|e| e.to_string())?;
        (id, gdir, icon_https)
    };

    // 6. Set initial progress
    {
        let mut p = state.install_progress.lock().map_err(|e| e.to_string())?;
        p.insert(instance_id.clone(), InstallProgress {
            step: "Modpack wird installiert…".into(),
            percent: 0.0,
            done: false,
            error: None,
        });
    }

    // 7. Background: download all files + write overrides + icon
    let ip = state.install_progress.clone();
    let http = state.http.clone();
    let iid = instance_id.clone();
    let config_arc = state.config.clone();

    let dl_files: Vec<MrpackIndexFile> = index.files.into_iter()
        .filter(|f| f.env.as_ref().map(|e| e.client.as_str() != "unsupported").unwrap_or(true))
        .collect();

    tokio::spawn(async move {
        let total = (dl_files.len() + overrides.len()).max(1);
        let mut done = 0usize;

        // Download and save icon locally so InstanceCard can display it
        if let Some(ref icon_url) = icon_https {
            if let Ok(resp) = http.get(icon_url).header("User-Agent", "NovaLauncher/0.1.0").send().await {
                if let Ok(data) = resp.bytes().await {
                    let _ = tokio::fs::create_dir_all(&game_dir).await;
                    let ext = std::path::Path::new(icon_url.split('?').next().unwrap_or(icon_url))
                        .extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
                    let icon_file = game_dir.join(format!("icon.{}", ext));
                    if tokio::fs::write(&icon_file, &data).await.is_ok() {
                        if let Ok(mut cfg) = config_arc.lock() {
                            if let Some(inst) = cfg.instances.iter_mut().find(|i| i.id == iid) {
                                inst.icon_path = Some(icon_file.display().to_string());
                            }
                            cfg.save().ok();
                        }
                    }
                }
            }
        }

        // (nova_path, project_id) for batch metadata enrichment after downloads
        let mut mod_meta_map: Vec<(std::path::PathBuf, String)> = Vec::new();

        for file in &dl_files {
            let url = match file.downloads.first() {
                Some(u) => u.clone(),
                None => { done += 1; continue; }
            };

            let dest = game_dir.join(&file.path);
            if let Some(parent) = dest.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }

            if let Ok(resp) = http.get(&url).header("User-Agent", "NovaLauncher/0.1.0").send().await {
                if let Ok(data) = resp.bytes().await {
                    let _ = tokio::fs::write(&dest, &data).await;
                    let fname = dest.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    // Extract project_id from Modrinth CDN URL: .../data/{project_id}/versions/...
                    let project_id = if url.contains("cdn.modrinth.com/data/") {
                        url.split("/data/").nth(1)
                            .and_then(|s| s.split('/').next())
                            .unwrap_or("").to_string()
                    } else { String::new() };
                    let title = std::path::Path::new(&fname)
                        .file_stem().and_then(|s| s.to_str()).unwrap_or(&fname)
                        .replace('"', "\\\"");
                    let meta_json = format!(
                        r#"{{"project_id":"{}","version_id":"","title":"{}","icon_url":null}}"#,
                        project_id, title
                    );
                    if let Some(parent) = dest.parent() {
                        let nova_path = parent.join(format!("{}.nova.json", fname));
                        let _ = tokio::fs::write(&nova_path, &meta_json).await;
                        if !project_id.is_empty() {
                            mod_meta_map.push((nova_path, project_id));
                        }
                    }
                }
            }

            done += 1;
            let fname = std::path::Path::new(&file.path)
                .file_name().and_then(|n| n.to_str()).unwrap_or(&file.path).to_string();
            if let Ok(mut p) = ip.lock() {
                p.insert(iid.clone(), InstallProgress {
                    step: format!("Lade {}…", fname),
                    percent: done as f32 / total as f32,
                    done: false,
                    error: None,
                });
            }
        }

        // Batch-fetch project names + icons from Modrinth for all downloaded mods
        if !mod_meta_map.is_empty() {
            let mut seen = std::collections::HashSet::new();
            let unique_ids: Vec<&str> = mod_meta_map.iter()
                .filter_map(|(_, pid)| if seen.insert(pid.as_str()) { Some(pid.as_str()) } else { None })
                .collect();
            if let Ok(ids_json) = serde_json::to_string(&unique_ids) {
                let batch_url = format!(
                    "https://api.modrinth.com/v2/projects?ids={}",
                    urlencoding::encode(&ids_json)
                );
                if let Ok(resp) = http.get(&batch_url).header("User-Agent", "NovaLauncher/0.1.0").send().await {
                    if let Ok(projects) = resp.json::<Vec<serde_json::Value>>().await {
                        let proj_map: std::collections::HashMap<String, (String, Option<String>)> = projects.iter()
                            .filter_map(|p| {
                                let id = p["id"].as_str()?.to_string();
                                let title = p["title"].as_str()?.to_string();
                                let icon = p["icon_url"].as_str().map(|s| s.to_string());
                                Some((id, (title, icon)))
                            })
                            .collect();
                        for (nova_path, project_id) in &mod_meta_map {
                            if let Some((title, icon_url)) = proj_map.get(project_id) {
                                let title_esc = title.replace('"', "\\\"");
                                let icon_str = match icon_url {
                                    Some(u) => format!(r#""{}""#, u.replace('"', "\\\"")),
                                    None => "null".to_string(),
                                };
                                let meta = format!(
                                    r#"{{"project_id":"{}","version_id":"","title":"{}","icon_url":{}}}"#,
                                    project_id, title_esc, icon_str
                                );
                                let _ = tokio::fs::write(nova_path, meta).await;
                            }
                        }
                    }
                }
            }
        }

        for (rel_path, data) in &overrides {
            let dest = game_dir.join(rel_path);
            if let Some(parent) = dest.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            let _ = tokio::fs::write(&dest, data).await;
        }

        if let Ok(mut p) = ip.lock() {
            p.insert(iid.clone(), InstallProgress {
                step: "Installation abgeschlossen".into(),
                percent: 1.0,
                done: true,
                error: None,
            });
        }
    });

    Ok(instance_id)
}

#[tauri::command]
async fn delete_mod(
    filename: String,
    instance_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_content(filename, instance_id, "mod".into(), state).await
}

#[tauri::command]
async fn delete_content(
    filename: String,
    instance_id: String,
    content_type: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let game_dir = {
        let config = lock_config(&state)?;
        config.instances.iter().find(|i| i.id == instance_id)
            .map(|i| i.resolved_game_dir())
            .ok_or_else(|| format!("Instanz {} nicht gefunden", instance_id))?
    };
    let sub_dir = match content_type.as_str() {
        "mod"          => game_dir.join("mods"),
        "resourcepack" | "resourcepacks" => game_dir.join("resourcepacks"),
        "shader"       | "shaderpacks"   => game_dir.join("shaderpacks"),
        "datapack"     | "datapacks"     => {
            // Check instance-level datapacks dir first
            let direct = game_dir.join("datapacks").join(&filename);
            if direct.exists() {
                if direct.is_dir() {
                    tokio::fs::remove_dir_all(&direct).await.map_err(|e| e.to_string())?;
                } else {
                    tokio::fs::remove_file(&direct).await.map_err(|e| e.to_string())?;
                }
                let meta = game_dir.join("datapacks").join(format!("{}.nova.json", filename));
                if meta.exists() { let _ = tokio::fs::remove_file(&meta).await; }
            }
            // Also remove from any world saves
            let saves = game_dir.join("saves");
            if saves.exists() {
                if let Ok(mut worlds) = tokio::fs::read_dir(&saves).await {
                    while let Ok(Some(w)) = worlds.next_entry().await {
                        let dp_path = w.path().join("datapacks").join(&filename);
                        if dp_path.exists() {
                            if dp_path.is_dir() {
                                let _ = tokio::fs::remove_dir_all(&dp_path).await;
                            } else {
                                let _ = tokio::fs::remove_file(&dp_path).await;
                            }
                        }
                    }
                }
            }
            return Ok(());
        }
        _ => return Err(format!("Unbekannter Content-Typ: {}", content_type)),
    };
    let path = sub_dir.join(&filename);
    if path.exists() {
        if path.is_dir() {
            tokio::fs::remove_dir_all(&path).await.map_err(|e| e.to_string())?;
        } else {
            tokio::fs::remove_file(&path).await.map_err(|e| e.to_string())?;
        }
    }
    let meta_path = sub_dir.join(format!("{}.nova.json", filename));
    if meta_path.exists() {
        let _ = tokio::fs::remove_file(&meta_path).await;
    }
    Ok(())
}

#[tauri::command]
async fn get_loader_versions(
    loader: String,
    mc_version: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    match loader.as_str() {
        "fabric" => {
            #[derive(serde::Deserialize)]
            struct FabricLoader { loader: FabricLoaderInfo }
            #[derive(serde::Deserialize)]
            struct FabricLoaderInfo { version: String }
            let url = format!("https://meta.fabricmc.net/v2/versions/loader/{}", mc_version);
            let loaders: Vec<FabricLoader> = cached_get(&state.http, &state.cache, &url, 3600).await?;
            Ok(loaders.into_iter().map(|l| l.loader.version).take(20).collect())
        }
        "quilt" => {
            #[derive(serde::Deserialize)]
            struct QuiltLoader { loader: QuiltLoaderInfo }
            #[derive(serde::Deserialize)]
            struct QuiltLoaderInfo { version: String }
            let url = format!("https://meta.quiltmc.org/v3/versions/loader/{}", mc_version);
            let loaders: Vec<QuiltLoader> = cached_get(&state.http, &state.cache, &url, 3600).await?;
            Ok(loaders.into_iter().map(|l| l.loader.version).take(20).collect())
        }
        "forge" => {
            #[derive(serde::Deserialize)]
            struct Promos { promos: std::collections::HashMap<String, String> }
            let url = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
            let data: Promos = cached_get(&state.http, &state.cache, url, 3600).await?;
            let prefix = format!("{}-", mc_version);
            // Collect unique versions, recommended first
            let rec_key = format!("{}-recommended", mc_version);
            let lat_key = format!("{}-latest", mc_version);
            let mut versions = Vec::new();
            if let Some(v) = data.promos.get(&rec_key) {
                versions.push(v.clone());
            }
            if let Some(v) = data.promos.get(&lat_key) {
                if !versions.contains(v) { versions.push(v.clone()); }
            }
            // Add remaining versions for this MC version
            let mut others: Vec<String> = data.promos.iter()
                .filter(|(k, _)| k.starts_with(&prefix) && **k != rec_key && **k != lat_key)
                .map(|(_, v)| v.clone())
                .collect();
            others.sort_by(|a, b| b.cmp(a));
            others.dedup();
            versions.extend(others);
            Ok(versions)
        }
        "neoforge" => {
            let url = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
            let xml: String = cached_get_text(&state.http, &state.cache, url, 3600).await?;
            let parts: Vec<&str> = mc_version.splitn(3, '.').collect();
            let prefix = match parts.len() {
                3 => format!("{}.{}.", parts[1], parts[2]),
                2 => format!("{}.", parts[1]),
                _ => return Err(format!("Ungültige MC-Version: {}", mc_version)),
            };
            let mut versions: Vec<String> = xml.lines()
                .filter_map(|line| {
                    let t = line.trim();
                    t.strip_prefix("<version>")?.strip_suffix("</version>").map(|v| v.to_string())
                })
                .filter(|v| v.starts_with(&prefix))
                .collect();
            versions.reverse();
            Ok(versions.into_iter().take(30).collect())
        }
        _ => Ok(vec!["latest".to_string()])
    }
}

#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_modrinth_project(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ModrinthProject, String> {
    let url = format!("https://api.modrinth.com/v2/project/{}", project_id);
    cached_get::<ModrinthProject>(&state.http, &state.cache, &url, 1800).await
}

#[tauri::command]
async fn search_modrinth_projects(
    query: String,
    project_type: String,
    game_version: Option<String>,
    loader: Option<String>,
    offset: u64,
    state: State<'_, AppState>,
) -> Result<ModrinthSearchResult, String> {
    let encoded_query: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let index = if project_type == "modpack" { "downloads" } else { "relevance" };
    let mut url = format!(
        "https://api.modrinth.com/v2/search?query={}&limit=20&offset={}&index={}",
        encoded_query, offset, index
    );
    let mut facets: Vec<String> = vec![format!("[\"project_type:{}\"]", project_type)];
    if let Some(gv) = &game_version {
        if !gv.is_empty() { facets.push(format!("[\"versions:{}\"]", gv)); }
    }
    if let Some(l) = &loader {
        if !l.is_empty() && l != "vanilla" && l != "paper" {
            facets.push(format!("[\"categories:{}\"]", l));
        }
    }
    url.push_str(&format!("&facets=[{}]", facets.join(",")));
    cached_get::<ModrinthSearchResult>(&state.http, &state.cache, &url, 300).await
}

/// Check a content directory for available updates via Modrinth.
async fn check_updates_for_dir(
    dir: &std::path::Path,
    content_type: &str,
    game_version: &Option<String>,
    loader: &Option<String>, // only used for mods
    http: &reqwest::Client,
) -> Vec<ModUpdateInfo> {
    let mut updates = Vec::new();
    if !dir.exists() { return updates; }
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else { return updates };
    let mut installed = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext != "jar" && ext != "zip" { continue; }
        let filename = entry.file_name().to_string_lossy().to_string();
        let meta_path = dir.join(format!("{}.nova.json", filename));
        if let Some(meta) = tokio::fs::read_to_string(&meta_path).await.ok()
            .and_then(|s| serde_json::from_str::<ModMeta>(&s).ok())
        {
            if !meta.project_id.is_empty() && !meta.version_id.is_empty() {
                installed.push((filename, meta));
            }
        }
    }
    for (filename, meta) in installed {
        let mut url = format!("https://api.modrinth.com/v2/project/{}/version?", meta.project_id);
        if let Some(ref gv) = game_version {
            if !gv.is_empty() { url.push_str(&format!("game_versions=[\"{}\"]&", urlencoding::encode(gv))); }
        }
        if content_type == "mod" {
            if let Some(ref l) = loader {
                url.push_str(&format!("loaders=[\"{}\"]&", l));
            }
        }
        if let Ok(resp) = http.get(&url).header("User-Agent", "NovaLauncher/0.1.0").send().await {
            if let Ok(versions) = resp.json::<Vec<ModrinthVersion>>().await {
                if let Some(latest) = versions.first() {
                    if latest.id != meta.version_id {
                        updates.push(ModUpdateInfo {
                            content_type: content_type.to_string(),
                            filename,
                            title: meta.title.clone(),
                            project_id: meta.project_id.clone(),
                            installed_version_id: meta.version_id.clone(),
                            latest_version_id: latest.id.clone(),
                            latest_version_number: latest.version_number.clone(),
                            icon_url: meta.icon_url.clone(),
                        });
                    }
                }
            }
        }
    }
    updates
}

#[tauri::command]
async fn check_instance_updates(
    instance_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ModUpdateInfo>, String> {
    let instance = {
        let config = lock_config(&state)?;
        config.instances.iter().find(|i| i.id == instance_id).cloned()
            .ok_or_else(|| format!("Instanz {} nicht gefunden", instance_id))?
    };
    let game_dir = instance.resolved_game_dir();
    let game_version = instance.version.clone();
    let loader = match instance.loader {
        crate::config::ModLoader::Vanilla | crate::config::ModLoader::Paper => None,
        ref l => Some(format!("{:?}", l).to_lowercase()),
    };
    let http = state.http.clone();
    let mut all = Vec::new();
    all.extend(check_updates_for_dir(&game_dir.join("mods"),         "mod",         &game_version, &loader, &http).await);
    all.extend(check_updates_for_dir(&game_dir.join("resourcepacks"),"resourcepack",&game_version, &None,   &http).await);
    all.extend(check_updates_for_dir(&game_dir.join("shaderpacks"),  "shader",      &game_version, &None,   &http).await);
    all.extend(check_updates_for_dir(&game_dir.join("datapacks"),    "datapack",    &game_version, &None,   &http).await);
    if let Ok(mut cache) = state.instance_updates.lock() {
        cache.insert(instance_id, all.clone());
    }
    Ok(all)
}

#[tauri::command]
fn get_instance_updates(
    instance_id: String,
    state: State<'_, AppState>,
) -> Result<Option<Vec<ModUpdateInfo>>, String> {
    let cache = state.instance_updates.lock().map_err(|e| e.to_string())?;
    Ok(cache.get(&instance_id).cloned())
}

#[tauri::command]
async fn update_content(
    instance_id: String,
    content_type: String,
    old_filename: String,
    project_id: String,
    new_version_id: String,
    title: Option<String>,
    icon_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let game_dir = {
        let config = lock_config(&state)?;
        config.instances.iter().find(|i| i.id == instance_id)
            .map(|i| i.resolved_game_dir())
            .ok_or_else(|| format!("Instanz {} nicht gefunden", instance_id))?
    };
    let sub_dir = match content_type.as_str() {
        "mod"         => game_dir.join("mods"),
        "resourcepack"=> game_dir.join("resourcepacks"),
        "shader"      => game_dir.join("shaderpacks"),
        "datapack"    => game_dir.join("datapacks"),
        other         => return Err(format!("Unbekannter Content-Typ: {}", other)),
    };
    // Fetch version info
    let version: ModrinthVersion = state.http
        .get(format!("https://api.modrinth.com/v2/version/{}", new_version_id))
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let file = version.files.iter().find(|f| f.primary).or_else(|| version.files.first())
        .ok_or("Keine Datei gefunden")?;
    let bytes = state.http.get(&file.url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .bytes().await.map_err(|e| e.to_string())?;
    // Remove old
    let old_path = sub_dir.join(&old_filename);
    if old_path.exists() { let _ = tokio::fs::remove_file(&old_path).await; }
    let old_meta = sub_dir.join(format!("{}.nova.json", old_filename));
    if old_meta.exists() { let _ = tokio::fs::remove_file(&old_meta).await; }
    // Write new
    tokio::fs::write(sub_dir.join(&file.filename), &bytes).await.map_err(|e| e.to_string())?;
    let meta = ModMeta {
        project_id: project_id.clone(),
        version_id: new_version_id.clone(),
        title: title.unwrap_or_else(|| file.filename.clone()),
        icon_url,
    };
    if let Ok(json) = serde_json::to_string(&meta) {
        let _ = tokio::fs::write(sub_dir.join(format!("{}.nova.json", file.filename)), json).await;
    }
    // Remove from update cache
    if let Ok(mut cache) = state.instance_updates.lock() {
        if let Some(updates) = cache.get_mut(&instance_id) {
            updates.retain(|u| !(u.filename == old_filename && u.content_type == content_type));
        }
    }
    Ok(file.filename.clone())
}

#[tauri::command]
async fn install_content(
    project_id: String,
    version_id: String,
    instance_id: String,
    content_type: String, // "mod" | "resourcepack" | "shader" | "datapack"
    title: Option<String>,
    icon_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let url = format!("https://api.modrinth.com/v2/version/{}", version_id);
    let version: ModrinthVersion = state.http.get(&url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;

    let file = version.files.iter().find(|f| f.primary).or_else(|| version.files.first())
        .ok_or("Keine Datei gefunden")?;

    let game_dir = {
        let config = lock_config(&state)?;
        config.instances.iter().find(|i| i.id == instance_id)
            .map(|i| i.resolved_game_dir())
            .ok_or_else(|| format!("Instanz {} nicht gefunden", instance_id))?
    };

    let subdir = match content_type.as_str() {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        "datapack" => "datapacks",
        _ => "mods",
    };

    let content_dir = game_dir.join(subdir);
    tokio::fs::create_dir_all(&content_dir).await.map_err(|e| e.to_string())?;
    let dest = content_dir.join(&file.filename);

    let bytes = state.http.get(&file.url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.map_err(|e| e.to_string())?
        .bytes().await.map_err(|e| e.to_string())?;
    tokio::fs::write(&dest, &bytes).await.map_err(|e| e.to_string())?;

    let meta = ModMeta {
        project_id: project_id.clone(),
        version_id: version_id.clone(),
        title: title.unwrap_or_else(|| file.filename.clone()),
        icon_url,
    };
    if let Ok(json) = serde_json::to_string(&meta) {
        let meta_path = content_dir.join(format!("{}.nova.json", file.filename));
        let _ = tokio::fs::write(&meta_path, json).await;
    }

    Ok(file.filename.clone())
}

#[tauri::command]
async fn read_file_base64(path: String) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let bytes = tokio::fs::read(&path).await.map_err(|e| e.to_string())?;
    Ok(STANDARD.encode(&bytes))
}

// ─── Mod Toggle ───────────────────────────────────────────────────────────────

#[tauri::command]
async fn toggle_mod(
    instance_id: String,
    filename: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mods_dir = {
        let config = lock_config(&state)?;
        config.instances.iter().find(|i| i.id == instance_id)
            .map(|i| i.resolved_game_dir().join("mods"))
            .ok_or_else(|| format!("Instanz {} nicht gefunden", instance_id))?
    };
    let current = mods_dir.join(&filename);
    if filename.ends_with(".jar.disabled") {
        let new_name = filename.strip_suffix(".disabled").unwrap();
        tokio::fs::rename(&current, mods_dir.join(new_name)).await.map_err(|e| e.to_string())
    } else if filename.ends_with(".jar") {
        tokio::fs::rename(&current, mods_dir.join(format!("{}.disabled", filename))).await.map_err(|e| e.to_string())
    } else {
        Err(format!("Unbekannter Dateityp: {}", filename))
    }
}

// ─── Crash Log Analysis ───────────────────────────────────────────────────────

/// Scan log lines for fatal crash indicators that don't produce a crash-reports file.
/// Fabric's FormattedException exits with code 0 and only logs to stdout — no crash file.
/// Scan log lines (from memory or file) for fatal crash indicators.
fn find_crash_in_logs(logs: &[String]) -> Option<String> {
    if logs.is_empty() { return None; }
    // Patterns that indicate a fatal unrecoverable crash.
    // Note: Minecraft/Fabric log format is "[HH:MM:SS] [thread/LEVEL]: message"
    // so FATAL level appears as "/FATAL]" not "[FATAL]".
    const FATAL_PATTERNS: &[&str] = &[
        "net.fabricmc.loader.impl.FormattedException",
        "net.minecraftforge.fml.LoadingFailedException",
        "---- Minecraft Crash Report ----",
        // "Exception in thread" catches crashes on any thread (Render thread, Server thread, etc.)
        "Exception in thread \"",
        "/FATAL]",
        "A fatal error has been detected by the Java Runtime",
        "EXCEPTION_ACCESS_VIOLATION",
        "java.lang.OutOfMemoryError",
    ];
    // Search last 500 lines for the first (earliest) match that isn't a Java stack trace line.
    // Exception messages appear BEFORE their stack traces in the log, so we search forward.
    // Searching in reverse would land on "at pkg.Class.method(...)" stack entries instead.
    let search_start = logs.len().saturating_sub(500);
    let matched = logs[search_start..].iter().find(|l| {
        if !FATAL_PATTERNS.iter().any(|p| l.contains(p)) { return false; }
        // Strip "[HH:MM:SS] [thread/LEVEL]: " prefix to get the raw message
        let msg = if let Some(pos) = l.find("]: ") { &l[pos + 3..] } else { l.as_str() };
        let msg = msg.trim_start();
        // Skip Java stack trace lines: "at pkg.Class.method(...)" and "... N more"
        !msg.starts_with("at ") && !msg.starts_with("... ")
    })?;
    // Strip the "[HH:MM:SS] [thread/LEVEL]: " log prefix if present
    let msg = if let Some(pos) = matched.find("]: ") { &matched[pos + 3..] } else { matched.as_str() };
    let msg = msg.trim();
    if msg.is_empty() { return None; }
    Some(msg.to_string())
}

/// Check for JVM crash files (hs_err_pid*.log) in the game directory.
/// These are written by the JVM for native-level crashes (SIGSEGV, SIGABRT, OOM in native code).
/// Minecraft's crash-reports/ mechanism doesn't catch these.
fn find_jvm_crash_after(game_dir: &std::path::Path, after_secs: u64) -> Option<String> {
    let cutoff = std::time::UNIX_EPOCH + std::time::Duration::from_secs(after_secs);
    let newest = std::fs::read_dir(game_dir).ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("hs_err_pid") && name.ends_with(".log")
        })
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            if modified > cutoff { Some((modified, e.path())) } else { None }
        })
        .max_by_key(|(t, _)| *t);
    let (_, path) = newest?;
    // Extract the signal/error line from the JVM crash log
    let content = std::fs::read_to_string(&path).ok()?;
    let detail = content.lines()
        .find(|l| l.starts_with("# A fatal error") || l.starts_with("# SIGSEGV") || l.starts_with("# SIGABRT") || l.starts_with("# SIGBUS"))
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .unwrap_or_else(|| "JVM crashed (native crash)".to_string());
    Some(detail)
}

/// Check whether a crash report was written after `after_secs` (Unix timestamp).
/// Returns the crash description extracted from the report, or None if no new report found.
fn find_crash_report_after(crash_dir: &std::path::Path, after_secs: u64) -> Option<String> {
    let cutoff = std::time::UNIX_EPOCH + std::time::Duration::from_secs(after_secs);
    let newest = std::fs::read_dir(crash_dir).ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("txt"))
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            if modified > cutoff { Some((modified, e.path())) } else { None }
        })
        .max_by_key(|(t, _)| *t);

    let (_, path) = newest?;
    let content = std::fs::read_to_string(&path).ok()?;
    // Extract "Description: ..." line from crash report
    let description = content.lines()
        .find(|l| l.starts_with("Description:"))
        .map(|l| l.trim_start_matches("Description:").trim().to_string())
        .unwrap_or_else(|| "Minecraft crashed".to_string());
    // Also grab the first exception class line
    let exception = content.lines()
        .find(|l| l.contains("Exception") || l.contains("Error:"))
        .map(|l| l.trim().to_string())
        .unwrap_or_default();
    if exception.is_empty() {
        Some(description)
    } else {
        Some(format!("{}\n{}", description, exception))
    }
}

#[derive(serde::Serialize)]
pub struct CrashIssue {
    pub severity: String,
    pub code: String,
}

#[derive(serde::Serialize)]
pub struct CrashAnalysis {
    pub has_issues: bool,
    pub issues: Vec<CrashIssue>,
    pub crash_report_snippet: Option<String>,
}

#[tauri::command]
async fn analyze_crash_log(instance_id: String, state: State<'_, AppState>) -> Result<CrashAnalysis, String> {
    let game_dir = {
        let config = lock_config(&state)?;
        config.instances.iter().find(|i| i.id == instance_id)
            .map(|i| i.resolved_game_dir())
            .ok_or_else(|| format!("Instanz {} nicht gefunden", instance_id))?
    };

    let log_path = game_dir.join("logs").join("latest.log");
    let log_content = if log_path.exists() {
        tokio::task::spawn_blocking(move || -> String {
            use std::io::{Read, Seek, SeekFrom};
            let mut file = match std::fs::File::open(&log_path) {
                Ok(f) => f,
                Err(_) => return String::new(),
            };
            let size = file.seek(SeekFrom::End(0)).unwrap_or(0);
            file.seek(SeekFrom::Start(size.saturating_sub(128 * 1024))).ok();
            let mut buf = String::new();
            file.read_to_string(&mut buf).ok();
            buf
        }).await.unwrap_or_default()
    } else {
        String::new()
    };

    // Find latest crash report
    let crash_dir = game_dir.join("crash-reports");
    let crash_report_snippet = if crash_dir.exists() {
        tokio::task::spawn_blocking(move || -> Option<String> {
            let mut entries: Vec<_> = std::fs::read_dir(&crash_dir).ok()?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("txt"))
                .collect();
            entries.sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));
            let latest = entries.first()?;
            let content = std::fs::read_to_string(latest.path()).ok()?;
            Some(content.lines().take(60).collect::<Vec<_>>().join("\n"))
        }).await.unwrap_or(None)
    } else {
        None
    };

    let combined = format!("{}\n{}", log_content, crash_report_snippet.as_deref().unwrap_or(""));
    let mut issues: Vec<CrashIssue> = Vec::new();

    if combined.contains("java.lang.OutOfMemoryError") || combined.contains("OutOfMemoryError") || combined.contains("GC overhead limit exceeded") {
        issues.push(CrashIssue { severity: "error".into(), code: "oom".into() });
    }
    if combined.contains("UnsupportedClassVersionError") {
        issues.push(CrashIssue { severity: "error".into(), code: "java_version".into() });
    }
    if combined.contains("DuplicateModsFoundException") || combined.to_lowercase().contains("duplicate mods found") {
        issues.push(CrashIssue { severity: "error".into(), code: "duplicate_mods".into() });
    }
    if combined.contains("Mixin apply failed") || combined.contains("MixinApplyError") || combined.contains("mixin.injection.throwables") {
        issues.push(CrashIssue { severity: "error".into(), code: "mixin_error".into() });
    }
    if combined.contains("requires version") || combined.contains("IncompatibleModError") || combined.contains("incompatible with")
        || combined.contains("FormattedException") || combined.contains("net.fabricmc.loader.impl.gui")
    {
        issues.push(CrashIssue { severity: "error".into(), code: "mod_compat".into() });
    }
    if combined.contains("NoClassDefFoundError") || combined.contains("ClassNotFoundException") {
        if combined.contains("mod") || combined.contains("fabric") || combined.contains("forge") || combined.contains("quilt") {
            issues.push(CrashIssue { severity: "error".into(), code: "missing_dep".into() });
        }
    }
    if combined.contains("FileNotFoundException") && (combined.contains("assets") || combined.contains("libraries") || combined.contains("versions")) {
        issues.push(CrashIssue { severity: "warning".into(), code: "missing_files".into() });
    }
    if combined.contains("Failed to verify username") || combined.contains("Invalid session") {
        issues.push(CrashIssue { severity: "warning".into(), code: "auth".into() });
    }
    if combined.contains("Could not reserve enough space") || combined.contains("Unable to create new native thread") {
        issues.push(CrashIssue { severity: "error".into(), code: "system_resources".into() });
    }
    if combined.contains("EXCEPTION_ACCESS_VIOLATION") || combined.contains("SIGSEGV")
        || combined.contains("# A fatal error has been detected by the Java Runtime")
        || (combined.contains("OpenGL error") && combined.contains("1286"))
    {
        issues.push(CrashIssue { severity: "error".into(), code: "driver".into() });
    }
    if (combined.to_lowercase().contains("shader") && combined.to_lowercase().contains("compile"))
        || combined.contains("GLSL")
        || combined.contains("ShaderCompileError")
    {
        issues.push(CrashIssue { severity: "error".into(), code: "shader".into() });
    }
    if combined.contains("Failed to load chunk") || combined.contains("Failed to load region")
        || (combined.to_lowercase().contains("corrupt") && combined.to_lowercase().contains("chunk"))
    {
        issues.push(CrashIssue { severity: "error".into(), code: "corrupt_save".into() });
    }
    if combined.contains("LWJGL") || combined.contains("GLFW error")
        || combined.contains("No OpenGL context") || combined.contains("Could not create context")
        || combined.contains("Failed to create display")
    {
        issues.push(CrashIssue { severity: "error".into(), code: "lwjgl".into() });
    }
    if combined.contains("JsonSyntaxException") || combined.contains("JsonParseException")
        || (combined.contains("parse") && (combined.contains(".toml") || combined.contains("config")))
    {
        issues.push(CrashIssue { severity: "warning".into(), code: "config_parse".into() });
    }

    let has_issues = !issues.is_empty();
    Ok(CrashAnalysis { has_issues, issues, crash_report_snippet })
}

// ─── JVM Suggestions ──────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct JvmSuggestion {
    pub label: String,
    pub args: String,
    pub description: String,
}

#[tauri::command]
fn get_jvm_suggestions(ram_mb: u32) -> Vec<JvmSuggestion> {
    let mut s = Vec::new();
    // Aikar's G1GC flags — the community gold standard for Minecraft
    let heap_region = if ram_mb >= 4096 { "8M" } else { "4M" };
    s.push(JvmSuggestion {
        label: "Aikar-Flags (G1GC, empfohlen)".into(),
        args: format!(
            "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 \
             -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:+AlwaysPreTouch \
             -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize={} \
             -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 \
             -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 \
             -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 \
             -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1", heap_region),
        description: "Optimierte GC-Flags für Minecraft (Aikar's Flags). Reduziert Lag-Spikes und verbessert Performance bei Modpacks.".into(),
    });
    if ram_mb >= 8192 {
        s.push(JvmSuggestion {
            label: "ZGC (Java 15+, sehr niedrige Latenz)".into(),
            args: "-XX:+UnlockExperimentalVMOptions -XX:+UseZGC -XX:+DisableExplicitGC \
                   -XX:+AlwaysPreTouch -XX:+ZUncommit -XX:ZUncommitDelay=60".into(),
            description: "Z Garbage Collector — extrem kurze GC-Pausen ideal für 8+ GB RAM. Benötigt Java 15 oder neuer.".into(),
        });
    }
    s
}

// ─── Discord RPC ──────────────────────────────────────────────────────────────

const DISCORD_CLIENT_ID: &str = "1482758920241025318";

/// Spawn a Discord RPC background task and return the sender.
/// Must be called from within an active Tokio runtime context.
fn start_discord() -> tokio::sync::mpsc::Sender<discord::DiscordMsg> {
    let (tx, rx) = discord::channel();
    tauri::async_runtime::spawn(discord::run(DISCORD_CLIENT_ID.to_string(), rx));
    tx
}

/// Call after saving config with changed Discord settings to (re)start the RPC task.
#[tauri::command]
async fn reload_discord_rpc(state: State<'_, AppState>) -> Result<(), String> {
    let enabled = lock_config(&state)?.discord_rpc_enabled;
    let (old_tx, new_tx) = {
        let mut guard = state.discord_tx.lock().map_err(|e| e.to_string())?;
        let old = guard.take();
        let new = if enabled {
            let tx = start_discord();
            *guard = Some(tx.clone());
            Some(tx)
        } else {
            None
        };
        (old, new)
    };
    if let Some(tx) = old_tx {
        tx.send(discord::DiscordMsg::Stop).await.ok();
    }
    // Show idle presence immediately after (re)enabling
    if let Some(tx) = new_tx {
        tx.try_send(discord::DiscordMsg::Set(discord::idle_activity())).ok();
    }
    Ok(())
}

// ─── Cache & App Dir ──────────────────────────────────────────────────────────

fn nova_cache_dir() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from(".")))
        .join("nova-launcher")
}


#[tauri::command]
fn get_cache_size(state: State<'_, AppState>) -> u64 {
    state.cache.size_bytes()
}

#[tauri::command]
async fn clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    state.cache.clear();
    Ok(())
}

#[tauri::command]
fn get_nova_dir() -> String {
    instances_base_dir()
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| instances_base_dir().to_string_lossy().to_string())
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog().file().blocking_pick_folder();
    Ok(path.map(|p| p.to_string()))
}

#[tauri::command]
fn get_system_locale() -> String {
    // Try LANG first (e.g. "de_DE.UTF-8"), then LANGUAGE, then LC_ALL
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LANGUAGE"))
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();
    // Extract the two-letter language code before '_' or '.'
    let code = lang.split(|c| c == '_' || c == '.' || c == '@').next().unwrap_or("en");
    let code = code.trim();
    if code.is_empty() { "en".to_string() } else { code.to_lowercase() }
}

#[tauri::command]
async fn complete_setup(
    language: String,
    accent_color: String,
    border_radius: u8,
    theme: String,
    discord_rpc: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut cfg = lock_config(&state)?;
    cfg.language = language;
    // Parse hex accent color into AccentColor struct
    let hex = accent_color.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            cfg.accent_color = crate::config::AccentColor {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
            };
        }
    }
    // Map border_radius to ui_radius preset
    cfg.ui_radius = if border_radius <= 4 {
        "compact".to_string()
    } else if border_radius <= 10 {
        String::new()
    } else if border_radius <= 16 {
        "rounded".to_string()
    } else {
        "pill".to_string()
    };
    cfg.ui_theme = if theme == "light" { "light".to_string() } else { String::new() };
    cfg.discord_rpc_enabled = discord_rpc;
    cfg.setup_complete = true;
    cfg.save().map_err(|e| e.to_string())
}

#[tauri::command]
async fn change_app_dir(new_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let new_base = std::path::PathBuf::from(&new_path).join("instances");
    let old_base = instances_base_dir();
    if new_base == old_base { return Ok(()); }
    std::fs::create_dir_all(&new_base).map_err(|e| e.to_string())?;
    if old_base.exists() {
        copy_dir_recursive(&old_base, &new_base).map_err(|e| e.to_string())?;
    }
    // Update instances that use the default location to point to new location
    let mut config = lock_config(&state)?;
    for instance in config.instances.iter_mut() {
        let uses_default = instance.game_dir.as_deref().map(|d| d.is_empty()).unwrap_or(true);
        if uses_default {
            instance.game_dir = Some(new_base.join(&instance.id).to_string_lossy().to_string());
        }
    }
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn close_splashscreen(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(splash) = app.get_webview_window("splashscreen") {
        splash.close().map_err(|e| e.to_string())?;
    }
    if let Some(main) = app.get_webview_window("main") {
        main.show().map_err(|e| e.to_string())?;
        main.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ─── App Entry ────────────────────────────────────────────────────────────────

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::load().unwrap_or_default();
    let accounts = AccountStore::load().unwrap_or_default();
    let http = reqwest::Client::builder()
        .user_agent("NovaLauncher/0.1.0")
        .build()
        .expect("HTTP Client konnte nicht erstellt werden");

    let cache_dir = nova_cache_dir();

    let state = AppState {
        config: Arc::new(Mutex::new(config)),
        accounts: Arc::new(Mutex::new(accounts)),
        http,
        manifest: Mutex::new(None),
        running_pids: Arc::new(Mutex::new(HashMap::new())),
        instance_logs: Arc::new(Mutex::new(HashMap::new())),
        instance_errors: Arc::new(Mutex::new(HashMap::new())),
        install_progress: Arc::new(Mutex::new(HashMap::new())),
        instance_updates: Arc::new(Mutex::new(HashMap::new())),
        stopping_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
        cache: Arc::new(cache::HttpCache::load(cache_dir)),
        discord_tx: Arc::new(Mutex::new(None)), // started in .setup() inside the tokio runtime
        online: Arc::new(Mutex::new(true)), // optimistisch starten; wird sofort geprüft
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(|app| {
            // .setup() runs inside the Tokio runtime — safe to spawn tasks here
            let state = app.state::<AppState>();
            // Apply sync settings (symlinks etc.) on every startup
            if let Ok(cfg) = state.config.lock() {
                sync::apply_syncs(&cfg);
            }
            let enabled = state.config.lock().expect("config lock").discord_rpc_enabled;
            if enabled {
                let tx = start_discord();
                // Show idle presence immediately on startup
                tx.try_send(discord::DiscordMsg::Set(discord::idle_activity())).ok();
                *state.discord_tx.lock().expect("discord_tx lock") = Some(tx);
            }
            // ── Konnektivitätsüberwachung ──────────────────────────────────────
            {
                let online_arc  = state.online.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        let now_online = check_internet().await;
                        {
                            let mut guard = online_arc.lock().unwrap();
                            *guard = now_online;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    }
                });
            }
            // Background update check for all instances on startup
            let cfg_arc    = state.config.clone();
            let http_clone = state.http.clone();
            let upd_arc    = state.instance_updates.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                let instances = if let Ok(cfg) = cfg_arc.lock() { cfg.instances.clone() } else { return };
                for instance in instances {
                    let game_dir = instance.resolved_game_dir();
                    let game_version = instance.version.clone();
                    let loader = match instance.loader {
                        crate::config::ModLoader::Vanilla | crate::config::ModLoader::Paper => None,
                        ref l => Some(format!("{:?}", l).to_lowercase()),
                    };
                    let mut all = Vec::new();
                    all.extend(check_updates_for_dir(&game_dir.join("mods"),         "mod",         &game_version, &loader,   &http_clone).await);
                    all.extend(check_updates_for_dir(&game_dir.join("resourcepacks"),"resourcepack",&game_version, &None,     &http_clone).await);
                    all.extend(check_updates_for_dir(&game_dir.join("shaderpacks"),  "shader",      &game_version, &None,     &http_clone).await);
                    all.extend(check_updates_for_dir(&game_dir.join("datapacks"),    "datapack",    &game_version, &None,     &http_clone).await);
                    if let Ok(mut cache) = upd_arc.lock() {
                        cache.insert(instance.id, all);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            get_accounts,
            get_online_status,
            get_server_play_history,
            test_java,
            detect_java,
            install_java,
            get_manifest,
            refresh_manifest,
            start_login,
            start_login_browser,
            poll_login,
            remove_account,
            set_active_account,
            add_offline_account,
            add_instance,
            delete_instance,
            select_instance,
            update_instance,
            launch_game,
            launch_instance,
            launch_with_quickplay,
            kill_instance,
            get_running_instances,
            get_instance_logs,
            get_instance_error,
            clear_instance_error,
            save_instance_icon,
            read_icon,
            create_instance,
            prepare_instance,
            get_install_progress,
            get_instance_details,
            ping_server,
            get_skin_presets,
            save_skin_preset,
            delete_skin_preset,
            set_active_skin_preset,
            import_current_skin_as_preset,
            get_current_mojang_skin,
            get_owned_capes,
            apply_skin_to_mojang,
            open_instance_folder,
            search_modrinth,
            search_modpacks,
            get_modrinth_versions,
            install_mod,
            install_modpack,
            delete_mod,
            delete_content,
            get_loader_versions,
            open_url,
            get_modrinth_project,
            search_modrinth_projects,
            install_content,
            check_instance_updates,
            get_instance_updates,
            update_content,
            read_screenshot,
            read_screenshot_thumb,
            delete_screenshot,
            compress_screenshot,
            read_file_base64,
            duplicate_instance,
            get_system_ram_mb,
            get_cache_size,
            clear_cache,
            get_nova_dir,
            change_app_dir,
            pick_folder,
            reload_discord_rpc,
            get_system_locale,
            complete_setup,
            toggle_mod,
            analyze_crash_log,
            get_jvm_suggestions,
            get_pending_crash,
            close_splashscreen,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Tauri-Anwendung");
}
