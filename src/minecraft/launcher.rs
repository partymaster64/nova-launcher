use anyhow::{Context, Result};
use futures::channel::mpsc;
use futures::SinkExt;
use std::path::{Path, PathBuf};

use crate::auth::microsoft::{complete_auth, refresh_ms_token};
use crate::auth::storage::StoredAccount;
use crate::config::Config;
use super::version::{fetch_version_meta, Library, VersionEntry, VersionMeta};

// ─── Launch-Events ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LaunchEvent {
    /// Token wurde erneuert – UI soll Account speichern
    TokenRefreshed(StoredAccount),
    /// Fortschritt (Schritt-Text, 0.0–1.0)
    Progress { step: String, percent: f32 },
    /// Minecraft-Prozess wurde gestartet
    Running,
    /// Minecraft normal beendet
    Done,
    /// Fehler aufgetreten
    Failed(String),
}

// ─── Launch-Optionen ──────────────────────────────────────────────────────────

pub struct LaunchOptions {
    pub client: reqwest::Client,
    pub config: Config,
    pub version_entry: VersionEntry,
    pub account: StoredAccount,
    pub tx: mpsc::UnboundedSender<LaunchEvent>,
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
    let account = if opts.account.is_token_expired() {
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
    let meta = match fetch_version_meta(&opts.client, &opts.version_entry.url).await {
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
    let applicable: Vec<_> = meta
        .libraries
        .iter()
        .filter(|l| l.applies_to_current_os())
        .collect();
    let lib_total = applicable.len().max(1) as f32;

    for (i, lib) in applicable.iter().enumerate() {
        let pct = 0.1 + 0.4 * ((i as f32 + 1.0) / lib_total);
        progress!(format!("Libraries ({}/{})", i + 1, applicable.len()), pct);

        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let dest = libraries_dir.join(&artifact.path);
            if let Err(e) =
                download_file(&opts.client, &artifact.url, &dest, Some(&artifact.sha1)).await
            {
                fail!(format!("Library {}: {e}", lib.name));
            }
        }

        if let Some(native) = lib.native_artifact() {
            let native_jar = libraries_dir.join(&native.path);
            if let Err(e) =
                download_file(&opts.client, &native.url, &native_jar, Some(&native.sha1)).await
            {
                fail!(format!("Native-Download {}: {e}", lib.name));
            }
            if let Err(e) =
                extract_natives(&native_jar, &natives_dir, &lib.extract_excludes()).await
            {
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

    // ── Starten ─────────────────────────────────────────────────────────────
    progress!("Starte Minecraft...", 0.95);
    let classpath = build_classpath(&libraries_dir, &meta.libraries, &client_jar);
    let java = opts.config.java_executable();

    let mut cmd = tokio::process::Command::new(&java);

    // Basis-JVM-Argumente
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

    // Main-Class
    cmd.arg(&meta.main_class);

    // Spiel-Argumente
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
    cmd.current_dir(&game_dir);

    tracing::info!("Starte Minecraft {} mit Java '{}'", version_id, java);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => fail!(format!("Java konnte nicht gestartet werden: {e}")),
    };

    let _ = tx.unbounded_send(LaunchEvent::Running);

    match child.wait().await {
        Ok(status) if status.success() => {
            let _ = tx.unbounded_send(LaunchEvent::Done);
        }
        Ok(status) => {
            let _ = tx.unbounded_send(LaunchEvent::Failed(format!(
                "Minecraft beendet mit Code {:?}",
                status.code()
            )));
        }
        Err(e) => {
            let _ = tx.unbounded_send(LaunchEvent::Failed(e.to_string()));
        }
    }
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

            // Verzeichnisse und ausgeschlossene Einträge überspringen
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
        tasks.push(tokio::spawn(async move {
            download_file(&client, &url, &path, Some(&hash)).await
        }));
    }

    for task in tasks {
        task.await??;
    }
    Ok(())
}

// ─── Classpath ────────────────────────────────────────────────────────────────

fn build_classpath(
    libraries_dir: &Path,
    libraries: &[Library],
    client_jar: &Path,
) -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut entries: Vec<PathBuf> = Vec::new();

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

    // Altes Format (<1.13)
    if let Some(old_args) = &meta.minecraft_arguments {
        args.extend(old_args.split_whitespace().map(replace));
    }
    // Neues Format (>=1.13)
    else if let Some(args_block) = &meta.arguments {
        args.extend(args_block.game.iter().filter_map(|v| v.as_str().map(replace)));
    }

    // Fenstermaße hinzufügen, wenn nicht bereits im Manifest enthalten
    if !args.contains(&"--width".to_string()) {
        args.push("--width".into());
        args.push(width.to_string());
        args.push("--height".into());
        args.push(height.to_string());
    }

    args
}
