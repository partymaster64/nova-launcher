use crate::config::{Config, Instance};
use image::codecs::jpeg::JpegEncoder;
use std::path::{Path, PathBuf};

// ─── Shared data root ────────────────────────────────────────────────────────

fn shared_root() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nova-launcher")
        .join("shared")
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Apply (or remove) all sync settings for all instances.
/// Call this on startup, after every config save, and after creating new instances.
pub fn apply_syncs(cfg: &Config) {
    let instances: Vec<&Instance> = cfg.instances.iter().collect();

    apply_or_restore_dir(&instances, "saves", cfg.saves_sync);
    apply_or_restore_dir(&instances, "resourcepacks", cfg.resource_pack_sync);
    // Screenshots: symlink-based so all instances see each other's screenshots
    apply_or_restore_dir(&instances, "screenshots", cfg.screenshot_sync);
    apply_or_restore_file(&instances, "servers.dat", cfg.servers_sync);
    apply_or_restore_file(&instances, "options.txt", cfg.config_sync);
    apply_or_restore_file(&instances, "keybinds.txt", cfg.config_sync);
}

// ─── Directory sync (symlink-based) ──────────────────────────────────────────

fn apply_or_restore_dir(instances: &[&Instance], dir_name: &str, enabled: bool) {
    if enabled {
        let _ = link_dir(instances, dir_name);
    } else {
        let _ = unlink_dir(instances, dir_name);
    }
}

/// Replace each instance's `dir_name/` with a symlink to a single shared dir.
fn link_dir(instances: &[&Instance], dir_name: &str) -> std::io::Result<()> {
    let shared = shared_root().join(dir_name);
    std::fs::create_dir_all(&shared)?;

    for inst in instances {
        let target = inst.resolved_game_dir().join(dir_name);

        // Already a symlink — nothing to do
        if target.is_symlink() {
            continue;
        }

        // Real directory exists: migrate its contents to shared (only if shared is empty)
        if target.is_dir() {
            if is_dir_empty(&shared) {
                copy_dir_contents(&target, &shared)?;
            }
            std::fs::remove_dir_all(&target)?;
        }

        // Create parent game_dir just in case the instance was never launched
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::os::unix::fs::symlink(&shared, &target)?;
    }
    Ok(())
}

/// Remove symlinks and give each instance its own copy of the shared dir.
fn unlink_dir(instances: &[&Instance], dir_name: &str) -> std::io::Result<()> {
    let shared = shared_root().join(dir_name);

    for inst in instances {
        let target = inst.resolved_game_dir().join(dir_name);

        if target.is_symlink() {
            std::fs::remove_file(&target)?;
            if shared.is_dir() {
                std::fs::create_dir_all(&target)?;
                copy_dir_contents(&shared, &target)?;
            }
        }
    }
    Ok(())
}

// ─── File sync (symlink-based) ────────────────────────────────────────────────

fn apply_or_restore_file(instances: &[&Instance], file_name: &str, enabled: bool) {
    if enabled {
        let _ = link_file(instances, file_name);
    } else {
        let _ = unlink_file(instances, file_name);
    }
}

fn link_file(instances: &[&Instance], file_name: &str) -> std::io::Result<()> {
    let shared_dir = shared_root();
    std::fs::create_dir_all(&shared_dir)?;
    let shared_file = shared_dir.join(file_name);

    for inst in instances {
        let game_dir = inst.resolved_game_dir();
        let target = game_dir.join(file_name);

        if target.is_symlink() {
            continue;
        }

        // Migrate existing file to shared (first instance wins)
        if target.is_file() {
            if !shared_file.exists() {
                std::fs::copy(&target, &shared_file)?;
            }
            std::fs::remove_file(&target)?;
        }

        // Only symlink if the shared file actually exists (the game creates it on first launch)
        if shared_file.exists() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(&shared_file, &target)?;
        }
    }
    Ok(())
}

fn unlink_file(instances: &[&Instance], file_name: &str) -> std::io::Result<()> {
    let shared_file = shared_root().join(file_name);

    for inst in instances {
        let target = inst.resolved_game_dir().join(file_name);

        if target.is_symlink() {
            std::fs::remove_file(&target)?;
            if shared_file.is_file() {
                std::fs::copy(&shared_file, &target)?;
            }
        }
    }
    Ok(())
}

// ─── Screenshot compression ───────────────────────────────────────────────────

/// Compress every PNG in an instance's screenshots/ folder to JPEG in-place.
/// Runs in a blocking thread pool to avoid stalling the async executor.
pub async fn compress_instance_screenshots(game_dir: PathBuf) {
    let screenshots_dir = game_dir.join("screenshots");
    if !screenshots_dir.is_dir() {
        return;
    }

    let entries: Vec<PathBuf> = match std::fs::read_dir(&screenshots_dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
            .collect(),
        Err(_) => return,
    };

    for path in entries {
        tokio::task::spawn_blocking(move || {
            compress_png_to_jpg(&path);
        })
        .await
        .ok();
    }
}

fn compress_png_to_jpg(path: &Path) {
    let img = match image::open(path) {
        Ok(i) => i,
        Err(_) => return,
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("screenshot");
    let dir = path.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let new_path = dir.join(format!("{}.jpg", stem));
    let file = match std::fs::File::create(&new_path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut writer = std::io::BufWriter::new(file);
    let encoder = JpegEncoder::new_with_quality(&mut writer, 85);
    if img.write_with_encoder(encoder).is_ok() {
        drop(writer);
        if new_path != path {
            std::fs::remove_file(path).ok();
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn is_dir_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true)
}

fn copy_dir_contents(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_contents(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
