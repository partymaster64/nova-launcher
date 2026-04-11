use anyhow::{Context, Result};
use futures::channel::mpsc;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::auth::microsoft::{complete_auth, refresh_ms_token};
use crate::auth::storage::StoredAccount;
use crate::config::{Config, ModLoader};
use super::version::{fetch_version_meta, Library, VersionEntry, VersionMeta};

// ─── Launch-Events ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LaunchEvent {
    TokenRefreshed(StoredAccount),
    Progress { step: String, percent: f32 },
    Running(u32),
    Log(String),
    /// Game process exited (exit code, or None if the OS didn't provide one).
    /// Used for both clean exits and crashes — crash detection happens in the handler.
    Exited(Option<i32>),
    /// Launch-time failure: installation error, Java not found, etc.
    /// The game process never ran (or couldn't be started).
    Failed(String),
}

// ─── Launch-Optionen ──────────────────────────────────────────────────────────

pub struct LaunchOptions {
    pub client: reqwest::Client,
    pub config: Config,
    pub version_entry: VersionEntry,
    pub account: StoredAccount,
    pub tx: mpsc::UnboundedSender<LaunchEvent>,
    /// If true: download & install files only, do not launch the game.
    pub install_only: bool,
    /// Ob Internet verfügbar ist — wenn false, Token-Refresh überspringen.
    pub online: bool,
    /// Extra args appended after all game args (used for --quickPlay*, --server, --port)
    pub quickplay_args: Vec<String>,
}

// ─── Haupt-Startfunktion ──────────────────────────────────────────────────────

pub async fn install_and_launch(opts: LaunchOptions) {
    let tx = opts.tx.clone();
    macro_rules! progress {
        ($step:expr, $pct:expr) => {
            let _ = tx.unbounded_send(LaunchEvent::Progress {
                step: $step.to_string(),
                percent: $pct,
            });
        };
    }
    macro_rules! fail {
        ($msg:expr) => {{
            let _ = tx.unbounded_send(LaunchEvent::Failed($msg.to_string()));
            return;
        }};
    }

    // ── Token ggf. erneuern ──────────────────────────────────────────────────
    let account = if !opts.install_only && opts.online && opts.account.is_token_expired() {
        progress!("Erneuere Minecraft-Token...", 0.0);
        match refresh_ms_token(&opts.client, &opts.account.refresh_token).await {
            Ok((ms_access, ms_refresh, ms_expires)) => {
                match complete_auth(&opts.client, &ms_access, &ms_refresh, ms_expires).await {
                    Ok(auth) => {
                        let updated = StoredAccount {
                            uuid: auth.uuid,
                            username: auth.username,
                            minecraft_token: auth.minecraft_token,
                            refresh_token: auth.refresh_token,
                            token_expires_at: auth.expires_at,
                            offline: false,
                        };
                        let _ = tx.unbounded_send(LaunchEvent::TokenRefreshed(updated.clone()));
                        updated
                    }
                    Err(e) => fail!(format!("Token-Erneuerung fehlgeschlagen: {e}")),
                }
            }
            Err(e) => fail!(format!("Token-Refresh fehlgeschlagen: {e}")),
        }
    } else {
        opts.account.clone()
    };

    // ── Versionsmetadaten ────────────────────────────────────────────────────
    progress!("Lade Versionsmetadaten...", 0.02);
    let meta = match fetch_version_meta(&opts.client, &opts.version_entry.url, &opts.version_entry.id).await {
        Ok(m) => m,
        Err(e) => fail!(format!("Versionsmetadaten: {e}")),
    };

    let version_id = &opts.version_entry.id;
    let game_dir = opts.config.game_dir();
    let versions_dir = game_dir.join("versions").join(version_id);
    let libraries_dir = game_dir.join("libraries");
    let assets_dir = game_dir.join("assets");
    let client_jar = versions_dir.join(format!("{}.jar", version_id));
    let natives_dir = versions_dir.join("natives");

    for dir in [&versions_dir, &libraries_dir, &assets_dir, &natives_dir] {
        if let Err(e) = tokio::fs::create_dir_all(dir).await {
            fail!(format!("Verzeichnis erstellen: {e}"));
        }
    }

    // ── Client-JAR ──────────────────────────────────────────────────────────
    progress!("Lade Client-JAR...", 0.05);
    let dl = &meta.downloads.client;
    if let Err(e) = download_file(&opts.client, &dl.url, &client_jar, Some(&dl.sha1)).await {
        fail!(format!("Client-JAR: {e}"));
    }

    // ── Libraries ───────────────────────────────────────────────────────────
    let applicable: Vec<_> = meta.libraries.iter().filter(|l| l.applies_to_current_os()).collect();
    let lib_total = applicable.len().max(1) as f32;

    for (i, lib) in applicable.iter().enumerate() {
        let pct = 0.1 + 0.4 * ((i as f32 + 1.0) / lib_total);
        progress!(format!("Libraries ({}/{})", i + 1, applicable.len()), pct);

        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let dest = libraries_dir.join(&artifact.path);
            if let Err(e) = download_file(&opts.client, &artifact.url, &dest, Some(&artifact.sha1)).await {
                fail!(format!("Library {}: {e}", lib.name));
            }
        }

        if let Some(native) = lib.native_artifact() {
            let native_jar = libraries_dir.join(&native.path);
            if let Err(e) = download_file(&opts.client, &native.url, &native_jar, Some(&native.sha1)).await {
                fail!(format!("Native-Download {}: {e}", lib.name));
            }
            if let Err(e) = extract_natives(&native_jar, &natives_dir, &lib.extract_excludes()).await {
                fail!(format!("Native-Extraktion {}: {e}", lib.name));
            }
        }
    }

    // ── Asset-Index ─────────────────────────────────────────────────────────
    progress!("Lade Asset-Index...", 0.55);
    let ai = &meta.asset_index;
    let index_path = assets_dir.join("indexes").join(format!("{}.json", ai.id));
    if let Err(e) = download_file(&opts.client, &ai.url, &index_path, Some(&ai.sha1)).await {
        fail!(format!("Asset-Index: {e}"));
    }

    // ── Assets ──────────────────────────────────────────────────────────────
    progress!("Lade Assets...", 0.6);
    if let Err(e) = download_assets(&opts.client, &index_path, &assets_dir).await {
        fail!(format!("Assets: {e}"));
    }

    // ── Loader installieren ──────────────────────────────────────────────────
    let instance = opts.config.active_instance();
    let loader = instance.map(|i| i.loader.clone()).unwrap_or_default();
    let loader_version = instance.and_then(|i| i.loader_version.clone());
    let java = opts.config.java_for_mc_version(version_id);
    // Collect instance-level overrides before instance reference is dropped
    let custom_jvm_args: Vec<String> = instance
        .and_then(|i| i.custom_jvm_args.as_deref())
        .map(|s| s.split_whitespace().map(|p| p.to_string()).collect())
        .unwrap_or_default();
    let env_vars: Vec<(String, String)> = instance
        .and_then(|i| i.env_vars.as_ref())
        .map(|vars| vars.iter()
            .filter(|v| v.len() == 2 && !v[0].is_empty())
            .map(|v| (v[0].clone(), v[1].clone()))
            .collect())
        .unwrap_or_default();
    let pre_launch_hook = instance.and_then(|i| i.pre_launch_hook.clone());
    let wrapper_command = instance.and_then(|i| i.wrapper_command.clone());
    let post_exit_hook = instance.and_then(|i| i.post_exit_hook.clone());
    let fullscreen = instance.map(|i| i.fullscreen).unwrap_or(false);
    let mut extra_jvm_args: Vec<String> = Vec::new();
    let mut extra_game_args: Vec<String> = Vec::new();

    let (main_class, extra_libs) = match loader {
        ModLoader::Fabric => {
            progress!("Lade Fabric-Metadaten...", 0.80);
            let lv = loader_version.as_deref().unwrap_or("latest");
            let tx_clone = tx.clone();
            let cb = move |step: String, pct: f32| {
                let _ = tx_clone.unbounded_send(LaunchEvent::Progress { step, percent: pct });
            };
            match install_fabric_loader(&opts.client, version_id, lv, &libraries_dir, &game_dir, opts.online, cb).await {
                Ok(result) => result,
                Err(e) => fail!(format!("Fabric-Installation fehlgeschlagen: {e}")),
            }
        }
        ModLoader::Quilt => {
            progress!("Lade Quilt-Metadaten...", 0.80);
            let lv = loader_version.as_deref().unwrap_or("latest");
            let tx_clone = tx.clone();
            let cb = move |step: String, pct: f32| {
                let _ = tx_clone.unbounded_send(LaunchEvent::Progress { step, percent: pct });
            };
            match install_quilt_loader(&opts.client, version_id, lv, &libraries_dir, &game_dir, opts.online, cb).await {
                Ok(result) => result,
                Err(e) => fail!(format!("Quilt-Installation fehlgeschlagen: {e}")),
            }
        }
        ModLoader::Forge => {
            progress!("Lade Forge-Installer...", 0.78);
            let lv = loader_version.as_deref().unwrap_or("latest");
            let tx_clone = tx.clone();
            let cb = move |step: String, pct: f32| {
                let _ = tx_clone.unbounded_send(LaunchEvent::Progress { step, percent: pct });
            };
            match install_forge_loader(&opts.client, version_id, lv, &libraries_dir, &game_dir, &java, opts.online, cb).await {
                Ok((mc, libs, jvm_args, game_args)) => {
                    extra_jvm_args = jvm_args;
                    extra_game_args = game_args;
                    (mc, libs)
                }
                Err(e) => fail!(format!("Forge-Installation fehlgeschlagen: {e}")),
            }
        }
        ModLoader::Neoforge => {
            progress!("Lade NeoForge-Installer...", 0.78);
            let lv = loader_version.as_deref().unwrap_or("latest");
            let tx_clone = tx.clone();
            let cb = move |step: String, pct: f32| {
                let _ = tx_clone.unbounded_send(LaunchEvent::Progress { step, percent: pct });
            };
            match install_neoforge_loader(&opts.client, version_id, lv, &libraries_dir, &game_dir, &java, opts.online, cb).await {
                Ok((mc, libs, jvm_args, game_args)) => {
                    extra_jvm_args = jvm_args;
                    extra_game_args = game_args;
                    (mc, libs)
                }
                Err(e) => fail!(format!("NeoForge-Installation fehlgeschlagen: {e}")),
            }
        }
        _ => (meta.main_class.clone(), vec![]),
    };

    // ── Install-only: fertig ─────────────────────────────────────────────────
    if opts.install_only {
        let _ = tx.unbounded_send(LaunchEvent::Exited(Some(0)));
        return;
    }

    // ── Starten ─────────────────────────────────────────────────────────────
    progress!("Starte Minecraft...", 0.95);
    let classpath = build_classpath_with_extras(&libraries_dir, &meta.libraries, &client_jar, &extra_libs);

    // Pre-launch hook
    if let Some(ref hook) = pre_launch_hook {
        if !hook.trim().is_empty() {
            let mut parts = hook.split_whitespace();
            if let Some(prog) = parts.next() {
                let _ = tokio::process::Command::new(prog)
                    .args(parts)
                    .current_dir(&game_dir)
                    .status().await;
            }
        }
    }

    // Build command; wrapper_command prepends the real executable
    let mut cmd = if let Some(ref wrapper) = wrapper_command {
        if !wrapper.trim().is_empty() {
            let mut parts = wrapper.split_whitespace();
            let prog = parts.next().unwrap();
            let mut c = tokio::process::Command::new(prog);
            c.args(parts).arg(&java);
            c
        } else {
            tokio::process::Command::new(&java)
        }
    } else {
        tokio::process::Command::new(&java)
    };

    cmd.arg(format!("-Xms{}m", opts.config.ram_min_mb()))
        .arg(format!("-Xmx{}m", opts.config.ram_max_mb()));

    // JVM-Argumente aus Version-Metadaten (Minecraft >=1.13)
    let mut meta_provides_cp = false;
    let mut meta_provides_natives = false;
    if let Some(args_block) = &meta.arguments {
        let natives_str = natives_dir.display().to_string();
        let cp_str = classpath.clone();
        for v in &args_block.jvm {
            if let Some(s) = v.as_str() {
                let s = s
                    .replace("${natives_directory}", &natives_str)
                    .replace("${launcher_name}", "nova-launcher")
                    .replace("${launcher_version}", env!("CARGO_PKG_VERSION"))
                    .replace("${classpath}", &cp_str);
                if s == "-cp" || s == "-classpath" {
                    meta_provides_cp = true;
                }
                if s.contains("java.library.path") {
                    meta_provides_natives = true;
                }
                cmd.arg(s);
            }
        }
    }
    if !meta_provides_natives {
        cmd.arg(format!("-Djava.library.path={}", natives_dir.display()));
    }
    if !meta_provides_cp {
        cmd.arg("-cp").arg(&classpath);
    }

    // Forge/NeoForge JVM args (module path, add-opens, -DlibraryDirectory, etc.)
    for arg in &extra_jvm_args {
        cmd.arg(arg);
    }

    // Custom user JVM args
    for arg in &custom_jvm_args {
        cmd.arg(arg);
    }

    // Environment variables
    for (k, v) in &env_vars {
        cmd.env(k, v);
    }

    cmd.arg(&main_class);

    let game_args = build_game_args(
        &meta,
        &game_dir,
        &assets_dir,
        &account.username,
        &account.uuid,
        &account.minecraft_token,
        version_id,
        opts.config.game_width(),
        opts.config.game_height(),
    );
    cmd.args(&game_args);

    // Forge/NeoForge game args (--launchTarget, --fml.forgeVersion, etc.)
    cmd.args(&extra_game_args);

    // Fullscreen
    if fullscreen {
        cmd.arg("--fullscreen");
    }

    // QuickPlay / direct connect args
    for arg in &opts.quickplay_args {
        cmd.arg(arg);
    }

    cmd.current_dir(&game_dir);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => fail!(format!("Java konnte nicht gestartet werden: {e}")),
    };

    let pid = child.id().unwrap_or(0);
    let _ = tx.unbounded_send(LaunchEvent::Running(pid));

    let stdout_task = if let Some(stdout) = child.stdout.take() {
        let tx_out = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_out.unbounded_send(LaunchEvent::Log(line));
            }
        })
    } else {
        tokio::spawn(async {})
    };

    let stderr_task = if let Some(stderr) = child.stderr.take() {
        let tx_err = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_err.unbounded_send(LaunchEvent::Log(line));
            }
        })
    } else {
        tokio::spawn(async {})
    };

    let wait_result = child.wait().await;
    // Wait for stdout/stderr readers to finish draining before sending Done/Failed.
    // This guarantees all LaunchEvent::Log lines are in the channel before Done arrives,
    // so crash detection in the Done handler sees the complete log output.
    let _ = tokio::join!(stdout_task, stderr_task);

    // Post-exit hook
    if let Some(ref hook) = post_exit_hook {
        if !hook.trim().is_empty() {
            let mut parts = hook.split_whitespace();
            if let Some(prog) = parts.next() {
                let _ = tokio::process::Command::new(prog)
                    .args(parts)
                    .current_dir(&game_dir)
                    .status().await;
            }
        }
    }

    match wait_result {
        Ok(status) => {
            let _ = tx.unbounded_send(LaunchEvent::Exited(status.code()));
        }
        Err(e) => {
            let _ = tx.unbounded_send(LaunchEvent::Failed(e.to_string()));
        }
    }
}

// ─── Fabric Loader Installation ───────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedLoaderProfile {
    main_class: String,
    libraries: Vec<CachedLoaderLib>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedLoaderLib {
    name: String,
    url: String,
}

async fn install_fabric_loader<F: Fn(String, f32)>(
    client: &reqwest::Client,
    mc_version: &str,
    loader_version: &str,
    libraries_dir: &Path,
    game_dir: &Path,
    online: bool,
    progress_cb: F,
) -> Result<(String, Vec<PathBuf>)> {
    #[derive(serde::Deserialize)]
    struct FabricProfile {
        #[serde(rename = "mainClass")]
        main_class: String,
        libraries: Vec<FabricLibEntry>,
    }
    #[derive(serde::Deserialize)]
    struct FabricLibEntry {
        name: String,
        url: String,
    }

    // Loader-Version auflösen; bei "latest" offline den Cache scannen
    let lv = if loader_version == "latest" {
        if online {
            let v = fetch_latest_fabric_loader_version(client, mc_version).await?;
            // Merke die aufgelöste Version für spätere Offline-Nutzung
            let _ = tokio::fs::write(
                game_dir.join(format!("fabric-latest-{}.txt", mc_version)),
                &v,
            ).await;
            v
        } else {
            // Zuletzt gespeicherte "latest"-Version laden
            let cached_ver_path = game_dir.join(format!("fabric-latest-{}.txt", mc_version));
            tokio::fs::read_to_string(&cached_ver_path).await
                .map(|s| s.trim().to_string())
                .map_err(|_| anyhow::anyhow!("Fabric nicht installiert und kein Internet vorhanden"))?
        }
    } else {
        loader_version.to_string()
    };

    let cache_path = game_dir.join(format!("fabric-profile-{}-{}.json", mc_version, lv));

    // Profil laden: online → von Netz (+ in Cache speichern), offline → aus Cache
    let profile: CachedLoaderProfile = if online {
        let url = format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
            mc_version, lv
        );
        let raw: FabricProfile = client.get(&url)
            .header("User-Agent", "NovaLauncher/0.1.0")
            .send().await
            .context("Fabric profile fetch")?
            .json().await
            .context("Fabric profile parse")?;
        let cached = CachedLoaderProfile {
            main_class: raw.main_class,
            libraries: raw.libraries.into_iter().map(|l| CachedLoaderLib { name: l.name, url: l.url }).collect(),
        };
        if let Ok(json) = serde_json::to_string(&cached) {
            let _ = tokio::fs::write(&cache_path, json).await;
        }
        cached
    } else {
        let json = tokio::fs::read_to_string(&cache_path).await
            .map_err(|_| anyhow::anyhow!("Fabric nicht installiert und kein Internet vorhanden"))?;
        serde_json::from_str(&json).context("Fabric profile cache parse")?
    };

    let total = profile.libraries.len().max(1) as f32;
    let mut downloaded = Vec::new();
    for (i, lib) in profile.libraries.iter().enumerate() {
        let pct = 0.82 + 0.10 * ((i as f32 + 1.0) / total);
        progress_cb(format!("Fabric libs ({}/{})", i + 1, profile.libraries.len()), pct);
        let rel_path = maven_local_path(&lib.name)?;
        let dest = libraries_dir.join(&rel_path);
        // Lib nur herunterladen wenn sie noch nicht existiert
        if !dest.exists() {
            if !online {
                return Err(anyhow::anyhow!(
                    "Fabric lib fehlt und kein Internet: {}", lib.name
                ));
            }
            let base = lib.url.trim_end_matches('/');
            let rel_str = rel_path.display().to_string().replace('\\', "/");
            let dl_url = format!("{}/{}", base, rel_str);
            download_file(client, &dl_url, &dest, None).await
                .with_context(|| format!("Fabric lib download: {}", lib.name))?;
        }
        downloaded.push(dest);
    }

    Ok((profile.main_class, downloaded))
}

async fn fetch_latest_fabric_loader_version(client: &reqwest::Client, mc_version: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Entry { loader: LoaderInfo }
    #[derive(serde::Deserialize)]
    struct LoaderInfo { version: String }

    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{}", mc_version);
    let entries: Vec<Entry> = client.get(&url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await?
        .json().await?;
    entries.into_iter().next()
        .map(|e| e.loader.version)
        .ok_or_else(|| anyhow::anyhow!("Keine Fabric Loader Versionen gefunden für {}", mc_version))
}

// ─── Quilt Loader Installation ────────────────────────────────────────────────

async fn install_quilt_loader<F: Fn(String, f32)>(
    client: &reqwest::Client,
    mc_version: &str,
    loader_version: &str,
    libraries_dir: &Path,
    game_dir: &Path,
    online: bool,
    progress_cb: F,
) -> Result<(String, Vec<PathBuf>)> {
    #[derive(serde::Deserialize)]
    struct QuiltProfile {
        #[serde(rename = "mainClass")]
        main_class: String,
        libraries: Vec<QuiltLibEntry>,
    }
    #[derive(serde::Deserialize)]
    struct QuiltLibEntry {
        name: String,
        url: String,
    }

    let lv = if loader_version == "latest" {
        if online {
            #[derive(serde::Deserialize)]
            struct QEntry { loader: QLoaderInfo }
            #[derive(serde::Deserialize)]
            struct QLoaderInfo { version: String }
            let url = format!("https://meta.quiltmc.org/v3/versions/loader/{}", mc_version);
            let entries: Vec<QEntry> = client.get(&url).send().await?.json().await?;
            let v = entries.into_iter().next()
                .map(|e| e.loader.version)
                .ok_or_else(|| anyhow::anyhow!("Keine Quilt Loader Versionen gefunden"))?;
            let _ = tokio::fs::write(
                game_dir.join(format!("quilt-latest-{}.txt", mc_version)),
                &v,
            ).await;
            v
        } else {
            let cached_ver_path = game_dir.join(format!("quilt-latest-{}.txt", mc_version));
            tokio::fs::read_to_string(&cached_ver_path).await
                .map(|s| s.trim().to_string())
                .map_err(|_| anyhow::anyhow!("Quilt nicht installiert und kein Internet vorhanden"))?
        }
    } else {
        loader_version.to_string()
    };

    let cache_path = game_dir.join(format!("quilt-profile-{}-{}.json", mc_version, lv));

    let profile: CachedLoaderProfile = if online {
        let url = format!(
            "https://meta.quiltmc.org/v3/versions/loader/{}/{}/profile/json",
            mc_version, lv
        );
        let raw: QuiltProfile = client.get(&url).send().await?.json().await
            .context("Quilt profile parse")?;
        let cached = CachedLoaderProfile {
            main_class: raw.main_class,
            libraries: raw.libraries.into_iter().map(|l| CachedLoaderLib { name: l.name, url: l.url }).collect(),
        };
        if let Ok(json) = serde_json::to_string(&cached) {
            let _ = tokio::fs::write(&cache_path, json).await;
        }
        cached
    } else {
        let json = tokio::fs::read_to_string(&cache_path).await
            .map_err(|_| anyhow::anyhow!("Quilt nicht installiert und kein Internet vorhanden"))?;
        serde_json::from_str(&json).context("Quilt profile cache parse")?
    };

    let total = profile.libraries.len().max(1) as f32;
    let mut downloaded = Vec::new();
    for (i, lib) in profile.libraries.iter().enumerate() {
        let pct = 0.82 + 0.10 * ((i as f32 + 1.0) / total);
        progress_cb(format!("Quilt libs ({}/{})", i + 1, profile.libraries.len()), pct);
        let rel_path = maven_local_path(&lib.name)?;
        let dest = libraries_dir.join(&rel_path);
        if !dest.exists() {
            if !online {
                return Err(anyhow::anyhow!(
                    "Quilt lib fehlt und kein Internet: {}", lib.name
                ));
            }
            let base = lib.url.trim_end_matches('/');
            let rel_str = rel_path.display().to_string().replace('\\', "/");
            let dl_url = format!("{}/{}", base, rel_str);
            download_file(client, &dl_url, &dest, None).await
                .with_context(|| format!("Quilt lib: {}", lib.name))?;
        }
        downloaded.push(dest);
    }

    Ok((profile.main_class, downloaded))
}

// ─── Forge / NeoForge Installer ───────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct ForgeResultCache {
    main_class: String,
    extra_libs: Vec<String>,
    jvm_args: Vec<String>,
    game_args: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ForgeInstallProfile {
    #[serde(default)]
    libraries: Vec<ForgeLibEntry>,
    #[serde(default)]
    processors: Vec<ForgeProcessor>,
    #[serde(default)]
    data: std::collections::HashMap<String, ForgeDataEntry>,
}

#[derive(serde::Deserialize)]
struct ForgeDataEntry {
    client: String,
}

#[derive(serde::Deserialize)]
struct ForgeProcessor {
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    sides: Vec<String>,
}

#[derive(serde::Deserialize, Default)]
struct ForgeArgBlock {
    #[serde(default)]
    jvm: Vec<serde_json::Value>,
    #[serde(default)]
    game: Vec<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct ForgeVersionJson {
    #[serde(rename = "mainClass")]
    main_class: String,
    #[serde(default)]
    libraries: Vec<ForgeLibEntry>,
    #[serde(default)]
    arguments: ForgeArgBlock,
}

#[derive(serde::Deserialize, Clone)]
struct ForgeLibEntry {
    name: String,
    #[serde(default)]
    downloads: Option<ForgeLibDownloads>,
}

#[derive(serde::Deserialize, Clone)]
struct ForgeLibDownloads {
    artifact: Option<ForgeArtifact>,
}

#[derive(serde::Deserialize, Clone)]
struct ForgeArtifact {
    #[serde(default)]
    path: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    sha1: String,
}

async fn install_forge_loader<F: Fn(String, f32)>(
    client: &reqwest::Client,
    mc_version: &str,
    forge_version: &str,
    libraries_dir: &Path,
    game_dir: &Path,
    java: &str,
    online: bool,
    progress_cb: F,
) -> Result<(String, Vec<PathBuf>, Vec<String>, Vec<String>)> {
    let fv = if forge_version == "latest" {
        if online {
            let v = fetch_latest_forge_version(client, mc_version).await?;
            let _ = tokio::fs::write(game_dir.join(format!("forge-latest-{}.txt", mc_version)), &v).await;
            v
        } else {
            tokio::fs::read_to_string(game_dir.join(format!("forge-latest-{}.txt", mc_version))).await
                .map(|s| s.trim().to_string())
                .map_err(|_| anyhow::anyhow!("Forge nicht installiert und kein Internet vorhanden"))?
        }
    } else {
        forge_version.to_string()
    };

    // Installationsergebnis aus Cache laden wenn vorhanden
    let cache_path = game_dir.join(format!("forge-result-{}-{}.json", mc_version, fv));
    if let Ok(json) = tokio::fs::read_to_string(&cache_path).await {
        if let Ok(cached) = serde_json::from_str::<ForgeResultCache>(&json) {
            let extra_libs: Vec<PathBuf> = cached.extra_libs.iter().map(PathBuf::from).collect();
            return Ok((cached.main_class, extra_libs, cached.jvm_args, cached.game_args));
        }
    }

    if !online {
        return Err(anyhow::anyhow!("Forge nicht installiert und kein Internet vorhanden"));
    }

    let url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{fv}/forge-{mc}-{fv}-installer.jar",
        mc = mc_version,
        fv = fv
    );
    let result = install_from_forge_installer(client, &url, mc_version, libraries_dir, game_dir, java, &progress_cb).await?;
    // Ergebnis cachen
    let cached = ForgeResultCache {
        main_class: result.0.clone(),
        extra_libs: result.1.iter().map(|p| p.display().to_string()).collect(),
        jvm_args: result.2.clone(),
        game_args: result.3.clone(),
    };
    if let Ok(json) = serde_json::to_string(&cached) {
        let _ = tokio::fs::write(&cache_path, json).await;
    }
    Ok(result)
}

async fn fetch_latest_forge_version(client: &reqwest::Client, mc_version: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Promos { promos: std::collections::HashMap<String, String> }
    let url = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
    let data: Promos = client.get(url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await?.json().await?;
    let rec = format!("{}-recommended", mc_version);
    let lat = format!("{}-latest", mc_version);
    data.promos.get(&rec)
        .or_else(|| data.promos.get(&lat))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Keine Forge-Version für MC {} gefunden", mc_version))
}

async fn install_neoforge_loader<F: Fn(String, f32)>(
    client: &reqwest::Client,
    mc_version: &str,
    neoforge_version: &str,
    libraries_dir: &Path,
    game_dir: &Path,
    java: &str,
    online: bool,
    progress_cb: F,
) -> Result<(String, Vec<PathBuf>, Vec<String>, Vec<String>)> {
    let nv = if neoforge_version == "latest" {
        if online {
            let v = fetch_latest_neoforge_version(client, mc_version).await?;
            let _ = tokio::fs::write(game_dir.join(format!("neoforge-latest-{}.txt", mc_version)), &v).await;
            v
        } else {
            tokio::fs::read_to_string(game_dir.join(format!("neoforge-latest-{}.txt", mc_version))).await
                .map(|s| s.trim().to_string())
                .map_err(|_| anyhow::anyhow!("NeoForge nicht installiert und kein Internet vorhanden"))?
        }
    } else {
        neoforge_version.to_string()
    };

    let cache_path = game_dir.join(format!("neoforge-result-{}-{}.json", mc_version, nv));
    if let Ok(json) = tokio::fs::read_to_string(&cache_path).await {
        if let Ok(cached) = serde_json::from_str::<ForgeResultCache>(&json) {
            let extra_libs: Vec<PathBuf> = cached.extra_libs.iter().map(PathBuf::from).collect();
            return Ok((cached.main_class, extra_libs, cached.jvm_args, cached.game_args));
        }
    }

    if !online {
        return Err(anyhow::anyhow!("NeoForge nicht installiert und kein Internet vorhanden"));
    }

    let url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar",
        v = nv
    );
    let result = install_from_forge_installer(client, &url, mc_version, libraries_dir, game_dir, java, &progress_cb).await?;
    let cached = ForgeResultCache {
        main_class: result.0.clone(),
        extra_libs: result.1.iter().map(|p| p.display().to_string()).collect(),
        jvm_args: result.2.clone(),
        game_args: result.3.clone(),
    };
    if let Ok(json) = serde_json::to_string(&cached) {
        let _ = tokio::fs::write(&cache_path, json).await;
    }
    Ok(result)
}

async fn fetch_latest_neoforge_version(client: &reqwest::Client, mc_version: &str) -> Result<String> {
    let url = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
    let xml = client.get(url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await?.text().await?;
    let parts: Vec<&str> = mc_version.splitn(3, '.').collect();
    let prefix = match parts.len() {
        3 => format!("{}.{}.", parts[1], parts[2]),
        2 => format!("{}.", parts[1]),
        _ => return Err(anyhow::anyhow!("Ungültige MC-Version: {}", mc_version)),
    };
    let mut versions: Vec<String> = xml.lines()
        .filter_map(|line| {
            let t = line.trim();
            t.strip_prefix("<version>")?.strip_suffix("</version>").map(|v| v.to_string())
        })
        .filter(|v| v.starts_with(&prefix))
        .collect();
    versions.reverse();
    versions.into_iter().next()
        .ok_or_else(|| anyhow::anyhow!("Keine NeoForge-Version für MC {} gefunden", mc_version))
}

async fn install_from_forge_installer<F: Fn(String, f32)>(
    client: &reqwest::Client,
    installer_url: &str,
    mc_version: &str,
    libraries_dir: &Path,
    game_dir: &Path,
    java: &str,
    progress_cb: &F,
) -> Result<(String, Vec<PathBuf>, Vec<String>, Vec<String>)> {
    // Download installer JAR
    progress_cb("Lade Forge-Installer...".into(), 0.80);
    let installer_bytes = client.get(installer_url)
        .header("User-Agent", "NovaLauncher/0.1.0")
        .send().await.context("Forge installer download")?
        .error_for_status().context("Forge installer HTTP")?
        .bytes().await.context("Forge installer bytes")?
        .to_vec();

    // Parse installer ZIP on a blocking thread
    let libs_dir = libraries_dir.to_path_buf();
    let (install_profile, version_json, data_files) =
        tokio::task::spawn_blocking(move || -> Result<_> {
            use std::io::{Cursor, Read};
            let cursor = Cursor::new(&installer_bytes);
            let mut archive = zip::ZipArchive::new(cursor)?;

            let mut version_json_str = String::new();
            archive.by_name("version.json")?.read_to_string(&mut version_json_str)?;
            let mut profile_str = String::new();
            archive.by_name("install_profile.json")?.read_to_string(&mut profile_str)?;

            let mut data_files: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i)?;
                let name = entry.name().to_string();
                if name.ends_with('/') { continue; }
                if name.starts_with("maven/") {
                    let rel = &name["maven/".len()..];
                    let dest = libs_dir.join(rel);
                    if let Some(p) = dest.parent() { std::fs::create_dir_all(p)?; }
                    if !dest.exists() {
                        let mut buf = Vec::new();
                        entry.read_to_end(&mut buf)?;
                        std::fs::write(&dest, &buf)?;
                    }
                } else if name.starts_with("data/") || name.starts_with("/data/") {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf)?;
                    data_files.insert(name, buf);
                }
            }

            let install_profile: ForgeInstallProfile = serde_json::from_str(&profile_str)
                .context("install_profile.json parsen")?;
            let version_json: ForgeVersionJson = serde_json::from_str(&version_json_str)
                .context("version.json parsen")?;
            Ok((install_profile, version_json, data_files))
        }).await??;

    // Write data files to temp dir
    let data_dir = game_dir.join(".forge-install-data");
    tokio::fs::create_dir_all(&data_dir).await?;
    for (name, bytes) in &data_files {
        let rel = name.trim_start_matches('/').trim_start_matches("data/");
        let dest = data_dir.join(rel);
        if let Some(p) = dest.parent() { tokio::fs::create_dir_all(p).await?; }
        tokio::fs::write(&dest, bytes).await?;
    }

    // Download processor libs
    progress_cb("Lade Forge-Prozessor-Bibliotheken...".into(), 0.83);
    for lib in &install_profile.libraries {
        download_forge_lib(client, lib, libraries_dir).await?;
    }

    // Download runtime libs from version.json; collect expected paths regardless of download success
    // (some libs like forge:...:client are generated by processors and won't be downloadable)
    progress_cb("Lade Forge-Laufzeitbibliotheken...".into(), 0.87);
    let mut extra_lib_paths: Vec<PathBuf> = Vec::new();
    for lib in &version_json.libraries {
        let expected = if let Some(path) = lib.downloads.as_ref()
            .and_then(|d| d.artifact.as_ref())
            .map(|a| a.path.as_str())
            .filter(|p| !p.is_empty())
        {
            libraries_dir.join(path)
        } else {
            libraries_dir.join(maven_local_path_ext(&lib.name)?)
        };
        extra_lib_paths.push(expected);
        download_forge_lib(client, lib, libraries_dir).await?;
    }

    // Copy MC client JAR to the library path expected by Forge processors
    let mc_client_src = game_dir.join("versions").join(mc_version).join(format!("{}.jar", mc_version));
    let mc_client_dest = libraries_dir.join(maven_local_path_ext(&format!("net.minecraft:client:{}", mc_version))?);
    if mc_client_src.exists() && !mc_client_dest.exists() {
        if let Some(p) = mc_client_dest.parent() { tokio::fs::create_dir_all(p).await?; }
        tokio::fs::copy(&mc_client_src, &mc_client_dest).await?;
    }

    // Run processors (client-side only)
    let processors: Vec<&ForgeProcessor> = install_profile.processors.iter()
        .filter(|p| p.sides.is_empty() || p.sides.iter().any(|s| s == "client"))
        .collect();
    let total = processors.len().max(1) as f32;
    for (i, processor) in processors.iter().enumerate() {
        let pct = 0.88 + 0.09 * ((i as f32 + 1.0) / total);
        progress_cb(format!("Forge-Prozessor {}/{}", i + 1, processors.len()), pct);
        run_forge_processor(processor, &install_profile.data, libraries_dir, &data_dir, game_dir, java, mc_version).await?;
    }

    let _ = tokio::fs::remove_dir_all(&data_dir).await;

    // Only include paths that exist (processor-generated JARs are now present)
    let extra_libs: Vec<PathBuf> = extra_lib_paths.into_iter().filter(|p| p.exists()).collect();

    // Extract JVM and game args from Forge's version.json, substituting placeholders
    let lib_dir_str = libraries_dir.display().to_string();
    let extra_jvm_args: Vec<String> = version_json.arguments.jvm.iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.replace("${library_directory}", &lib_dir_str))
        .collect();
    let extra_game_args: Vec<String> = version_json.arguments.game.iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect();

    Ok((version_json.main_class, extra_libs, extra_jvm_args, extra_game_args))
}

async fn run_forge_processor(
    processor: &ForgeProcessor,
    data: &std::collections::HashMap<String, ForgeDataEntry>,
    libraries_dir: &Path,
    data_dir: &Path,
    game_dir: &Path,
    java: &str,
    mc_version: &str,
) -> Result<()> {
    let jar_path = libraries_dir.join(maven_local_path_ext(&processor.jar)?);
    let jar_clone = jar_path.clone();
    let main_class = tokio::task::spawn_blocking(move || read_jar_main_class(&jar_clone)).await??;

    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut cp = vec![jar_path.display().to_string()];
    for dep in &processor.classpath {
        cp.push(libraries_dir.join(maven_local_path_ext(dep)?).display().to_string());
    }

    let mut args: Vec<String> = Vec::new();
    for arg in &processor.args {
        args.push(resolve_forge_arg(arg, data, libraries_dir, data_dir, game_dir, mc_version)?);
    }

    let output = tokio::process::Command::new(java)
        .arg("-cp").arg(cp.join(sep))
        .arg(&main_class)
        .args(&args)
        .current_dir(game_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output().await
        .with_context(|| format!("Forge-Prozessor: {}", processor.jar))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Forge-Prozessor fehlgeschlagen (Code {:?}): {}\nStderr: {}",
            output.status.code(),
            processor.jar,
            stderr
        ));
    }
    Ok(())
}

fn resolve_forge_arg(
    arg: &str,
    data: &std::collections::HashMap<String, ForgeDataEntry>,
    libraries_dir: &Path,
    data_dir: &Path,
    game_dir: &Path,
    mc_version: &str,
) -> Result<String> {
    if let Some(key) = arg.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        // Special built-in keys injected by the installer at runtime
        match key {
            "SIDE" => return Ok("client".to_string()),
            "MINECRAFT_VERSION" => return Ok(mc_version.to_string()),
            "MINECRAFT_JAR" => {
                let path = libraries_dir.join(maven_local_path_ext(
                    &format!("net.minecraft:client:{}", mc_version)
                )?);
                return Ok(path.display().to_string());
            }
            _ => {}
        }
        let entry = data.get(key)
            .ok_or_else(|| anyhow::anyhow!("Unbekannter Forge-Data-Key: {}", key))?;
        resolve_forge_data_value(&entry.client, libraries_dir, data_dir, game_dir)
    } else if let Some(coord) = arg.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        Ok(libraries_dir.join(maven_local_path_ext(coord)?).display().to_string())
    } else {
        Ok(arg.to_string())
    }
}

fn resolve_forge_data_value(
    value: &str,
    libraries_dir: &Path,
    data_dir: &Path,
    _game_dir: &Path,
) -> Result<String> {
    if let Some(coord) = value.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        Ok(libraries_dir.join(maven_local_path_ext(coord)?).display().to_string())
    } else if let Some(lit) = value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        Ok(lit.to_string())
    } else if let Some(rel) = value.strip_prefix("/data/").or_else(|| value.strip_prefix("data/")) {
        Ok(data_dir.join(rel).display().to_string())
    } else {
        Ok(value.to_string())
    }
}

fn read_jar_main_class(jar_path: &Path) -> Result<String> {
    use std::io::Read;
    let file = std::fs::File::open(jar_path)
        .with_context(|| format!("JAR öffnen: {}", jar_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut manifest = archive.by_name("META-INF/MANIFEST.MF")
        .context("MANIFEST.MF nicht gefunden")?;
    let mut content = String::new();
    manifest.read_to_string(&mut content)?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("Main-Class:") {
            return Ok(rest.trim().to_string());
        }
    }
    Err(anyhow::anyhow!("Keine Main-Class in Manifest: {}", jar_path.display()))
}

async fn download_forge_lib(
    client: &reqwest::Client,
    lib: &ForgeLibEntry,
    libraries_dir: &Path,
) -> Result<Option<PathBuf>> {
    // Use explicit artifact path if available, otherwise derive from name
    let rel = if let Some(path) = lib.downloads.as_ref()
        .and_then(|d| d.artifact.as_ref())
        .map(|a| a.path.as_str())
        .filter(|p| !p.is_empty())
    {
        PathBuf::from(path)
    } else {
        maven_local_path_ext(&lib.name)?
    };
    let dest = libraries_dir.join(&rel);
    if dest.exists() { return Ok(Some(dest)); }

    // Use explicit download URL if available
    if let Some(art) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        if !art.url.is_empty() {
            let sha = if art.sha1.is_empty() { None } else { Some(art.sha1.as_str()) };
            download_file(client, &art.url, &dest, sha).await
                .with_context(|| format!("Forge lib: {}", lib.name))?;
            return Ok(Some(dest));
        }
    }

    // Try Maven repositories
    let rel_str = rel.display().to_string().replace('\\', "/");
    let repos = [
        "https://maven.minecraftforge.net/",
        "https://maven.neoforged.net/releases/",
        "https://repo1.maven.org/maven2/",
        "https://libraries.minecraft.net/",
    ];
    for repo in &repos {
        if download_file(client, &format!("{}{}", repo, rel_str), &dest, None).await.is_ok() {
            return Ok(Some(dest));
        }
    }

    if dest.exists() { return Ok(Some(dest)); }
    tracing::warn!("Konnte Forge-Lib nicht herunterladen: {}", lib.name);
    Ok(None)
}

// ─── Maven Path ───────────────────────────────────────────────────────────────

fn maven_local_path(coords: &str) -> Result<PathBuf> {
    // "net.fabricmc:fabric-loader:0.15.3" → net/fabricmc/fabric-loader/0.15.3/fabric-loader-0.15.3.jar
    let parts: Vec<&str> = coords.splitn(3, ':').collect();
    if parts.len() < 3 {
        return Err(anyhow::anyhow!("Ungültige Maven-Koordinaten: {}", coords));
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version_part = parts[2];

    // Handle optional classifier: "0.15.3:native"
    let (version, classifier) = if let Some((v, c)) = version_part.split_once(':') {
        (v, Some(c))
    } else {
        (version_part, None)
    };

    let filename = match classifier {
        Some(c) => format!("{}-{}-{}.jar", artifact, version, c),
        None => format!("{}-{}.jar", artifact, version),
    };

    Ok(PathBuf::from(format!("{}/{}/{}/{}", group, artifact, version, filename)))
}

/// Like `maven_local_path` but also handles `@extension` suffix (e.g. `coord@txt`).
fn maven_local_path_ext(coords: &str) -> Result<PathBuf> {
    // Split off @extension if present
    let (coords_clean, extension) = if let Some(at) = coords.rfind('@') {
        (&coords[..at], &coords[at + 1..])
    } else {
        (coords, "jar")
    };
    let parts: Vec<&str> = coords_clean.splitn(3, ':').collect();
    if parts.len() < 3 {
        return Err(anyhow::anyhow!("Ungültige Maven-Koordinaten: {}", coords));
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version_part = parts[2];
    let (version, classifier) = if let Some((v, c)) = version_part.split_once(':') {
        (v, Some(c))
    } else {
        (version_part, None)
    };
    let filename = match classifier {
        Some(c) => format!("{}-{}-{}.{}", artifact, version, c, extension),
        None => format!("{}-{}.{}", artifact, version, extension),
    };
    Ok(PathBuf::from(format!("{}/{}/{}/{}", group, artifact, version, filename)))
}

// ─── Natives-Extraktion ───────────────────────────────────────────────────────

pub async fn extract_natives(
    jar_path: &Path,
    dest_dir: &Path,
    exclude: &[String],
) -> Result<()> {
    let jar = jar_path.to_path_buf();
    let dest = dest_dir.to_path_buf();
    let excl = exclude.to_vec();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let file = std::fs::File::open(&jar)
            .with_context(|| format!("Natives-JAR öffnen: {}", jar.display()))?;
        let mut archive = zip::ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_string();

            if name.ends_with('/') || excl.iter().any(|e| name.starts_with(e.as_str())) {
                continue;
            }

            let out_path = dest.join(&name);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
        }
        Ok(())
    })
    .await??;

    Ok(())
}

// ─── Download ─────────────────────────────────────────────────────────────────

pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    path: &Path,
    expected_sha1: Option<&str>,
) -> Result<()> {
    use sha1::{Digest, Sha1};

    if let Some(sha1) = expected_sha1 {
        if path.exists() {
            let data = tokio::fs::read(path).await?;
            let digest = hex::encode(Sha1::digest(&data));
            if digest == sha1 {
                return Ok(());
            }
        }
    } else if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let response = client.get(url).send().await?.error_for_status()?;
    let mut file = tokio::fs::File::create(path).await?;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?).await?;
    }
    Ok(())
}

// ─── Assets ───────────────────────────────────────────────────────────────────

async fn download_assets(
    client: &reqwest::Client,
    index_path: &Path,
    assets_dir: &Path,
) -> Result<()> {
    #[derive(serde::Deserialize)]
    struct AssetObject {
        hash: String,
        #[allow(dead_code)]
        size: u64,
    }
    #[derive(serde::Deserialize)]
    struct AssetIndex {
        objects: std::collections::HashMap<String, AssetObject>,
    }

    let json = tokio::fs::read_to_string(index_path).await?;
    let index: AssetIndex = serde_json::from_str(&json)?;

    let objects_dir = assets_dir.join("objects");
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    let mut tasks = Vec::new();

    for obj in index.objects.values() {
        let prefix = obj.hash[..2].to_string();
        let url = format!(
            "https://resources.download.minecraft.net/{}/{}",
            prefix, obj.hash
        );
        let path = objects_dir.join(&prefix).join(&obj.hash);
        let hash = obj.hash.clone();
        let client = client.clone();
        let permit = sem.clone().acquire_owned().await?;
        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            download_file(&client, &url, &path, Some(&hash)).await
        }));
    }

    for task in tasks {
        task.await??;
    }
    Ok(())
}

// ─── Classpath ────────────────────────────────────────────────────────────────

fn build_classpath_with_extras(
    libraries_dir: &Path,
    libraries: &[Library],
    client_jar: &Path,
    extra_libs: &[PathBuf],
) -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut entries: Vec<PathBuf> = Vec::new();

    // Loader libs first (Fabric/Quilt need to be before vanilla)
    entries.extend_from_slice(extra_libs);

    for lib in libraries {
        if !lib.applies_to_current_os() {
            continue;
        }
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            entries.push(libraries_dir.join(&artifact.path));
        }
    }
    entries.push(client_jar.to_path_buf());

    entries
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(sep)
}

// ─── Spiel-Argumente ──────────────────────────────────────────────────────────

fn build_game_args(
    meta: &VersionMeta,
    game_dir: &Path,
    assets_dir: &Path,
    player_name: &str,
    player_uuid: &str,
    token: &str,
    version_id: &str,
    width: u32,
    height: u32,
) -> Vec<String> {
    let replace = |s: &str| -> String {
        s.replace("${auth_player_name}", player_name)
            .replace("${auth_uuid}", player_uuid)
            .replace("${auth_access_token}", token)
            .replace("${user_type}", "msa")
            .replace("${version_name}", version_id)
            .replace("${game_directory}", &game_dir.display().to_string())
            .replace("${assets_root}", &assets_dir.display().to_string())
            .replace("${assets_index_name}", &meta.assets)
            .replace("${user_properties}", "{}")
            .replace("${version_type}", "release")
    };

    let mut args: Vec<String> = Vec::new();

    if let Some(old_args) = &meta.minecraft_arguments {
        args.extend(old_args.split_whitespace().map(replace));
    } else if let Some(args_block) = &meta.arguments {
        args.extend(args_block.game.iter().filter_map(|v| v.as_str().map(replace)));
    }

    if !args.contains(&"--width".to_string()) {
        args.push("--width".into());
        args.push(width.to_string());
        args.push("--height".into());
        args.push(height.to_string());
    }

    args
}
