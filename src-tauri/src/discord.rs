/// Minimal Discord IPC Rich Presence client.
/// Works on Linux with native, Flatpak and Snap Discord installs,
/// on macOS and on Windows (named pipe).
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(unix)]
use tracing::warn;

#[derive(Debug, Clone)]
pub struct Activity {
    pub details: Option<String>,
    pub state: Option<String>,
    pub start_timestamp: Option<u64>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
}

/// Activity shown while the launcher is open but no game is running.
pub fn idle_activity() -> Activity {
    Activity {
        details: Some("Im Launcher".to_string()),
        state: None,
        start_timestamp: None,
        large_image: Some("nova_icon_dark".to_string()),
        large_text: Some("Nova Launcher".to_string()),
        small_image: Some("nova_idle_clock".to_string()),
        small_text: Some("Leerlauf".to_string()),
    }
}

#[derive(Debug)]
pub enum DiscordMsg {
    Set(Activity),
    Clear,
    Stop,
}

// ─── Platform sockets ─────────────────────────────────────────────────────────

#[cfg(unix)]
fn socket_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    // XDG_RUNTIME_DIR — most common on systemd-based Linux
    if let Ok(run) = std::env::var("XDG_RUNTIME_DIR") {
        let base = std::path::PathBuf::from(&run);
        for i in 0..10 {
            paths.push(base.join(format!("discord-ipc-{}", i)));
            // Flatpak
            paths.push(base.join(format!("app/com.discordapp.Discord/discord-ipc-{}", i)));
            // Snap
            paths.push(base.join(format!("snap.discord/discord-ipc-{}", i)));
        }
    }
    // TMPDIR / /tmp fallback
    let tmp = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
    for i in 0..10 {
        paths.push(std::path::PathBuf::from(format!("{}/discord-ipc-{}", tmp, i)));
    }
    paths
}

// ─── Low-level packet I/O ─────────────────────────────────────────────────────

#[cfg(unix)]
async fn open_stream() -> Option<tokio::net::UnixStream> {
    for path in socket_paths() {
        match tokio::net::UnixStream::connect(&path).await {
            Ok(s) => return Some(s),
            Err(e) => {
                if path.exists() {
                    warn!("[Discord RPC] socket connect failed {}: {}", path.display(), e);
                }
            }
        }
    }
    warn!("[Discord RPC] no Discord socket found — is Discord running?");
    None
}

#[cfg(unix)]
async fn write_packet(
    stream: &mut tokio::net::UnixStream,
    opcode: u32,
    payload: &str,
) -> std::io::Result<()> {
    let bytes = payload.as_bytes();
    let mut buf = Vec::with_capacity(8 + bytes.len());
    buf.extend_from_slice(&opcode.to_le_bytes());
    buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(bytes);
    stream.write_all(&buf).await
}

#[cfg(unix)]
async fn read_packet(stream: &mut tokio::net::UnixStream) -> std::io::Result<Vec<u8>> {
    let mut hdr = [0u8; 8];
    stream.read_exact(&mut hdr).await?;
    let len = u32::from_le_bytes(hdr[4..8].try_into().unwrap()) as usize;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await?;
    Ok(body)
}

// ─── JSON helpers ─────────────────────────────────────────────────────────────

#[cfg(unix)]
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(unix)]
fn nonce() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(unix)]
fn activity_json(a: &Activity) -> String {
    let mut parts = Vec::new();
    if let Some(d) = &a.details { parts.push(format!(r#""details":"{}""#, escape(d))); }
    if let Some(s) = &a.state   { parts.push(format!(r#""state":"{}""#,   escape(s))); }
    if let Some(ts) = a.start_timestamp {
        parts.push(format!(r#""timestamps":{{"start":{}}}"#, ts));
    }
    // Assets
    let mut assets = Vec::new();
    if let Some(img) = &a.large_image  { assets.push(format!(r#""large_image":"{}""#,  escape(img))); }
    if let Some(txt) = &a.large_text   { assets.push(format!(r#""large_text":"{}""#,   escape(txt))); }
    if let Some(img) = &a.small_image  { assets.push(format!(r#""small_image":"{}""#,  escape(img))); }
    if let Some(txt) = &a.small_text   { assets.push(format!(r#""small_text":"{}""#,   escape(txt))); }
    if !assets.is_empty() {
        parts.push(format!(r#""assets":{{{}}}"#, assets.join(",")));
    }
    format!("{{{}}}", parts.join(","))
}

#[cfg(unix)]
fn set_activity_payload(a: &Activity) -> String {
    format!(
        r#"{{"cmd":"SET_ACTIVITY","args":{{"pid":{},"activity":{}}},"nonce":"{}"}}"#,
        std::process::id(),
        activity_json(a),
        nonce()
    )
}

#[cfg(unix)]
fn clear_activity_payload() -> String {
    format!(
        r#"{{"cmd":"SET_ACTIVITY","args":{{"pid":{},"activity":null}},"nonce":"{}"}}"#,
        std::process::id(),
        nonce()
    )
}

// ─── Background task ─────────────────────────────────────────────────────────

pub fn channel() -> (tokio::sync::mpsc::Sender<DiscordMsg>, tokio::sync::mpsc::Receiver<DiscordMsg>) {
    tokio::sync::mpsc::channel(32)
}

pub async fn run(client_id: String, rx: tokio::sync::mpsc::Receiver<DiscordMsg>) {
    #[cfg(unix)]
    run_loop(client_id, rx).await;
    #[cfg(not(unix))]
    {
        let _ = client_id;
        let _ = rx;
    }
}

#[cfg(unix)]
async fn run_loop(client_id: String, mut rx: tokio::sync::mpsc::Receiver<DiscordMsg>) {
    loop {
        let initial = loop {
            match rx.recv().await {
                Some(DiscordMsg::Set(a)) => break a,
                Some(DiscordMsg::Clear) => {}
                Some(DiscordMsg::Stop) | None => return,
            }
        };

        let mut stream = loop {
            if let Some(s) = open_stream().await { break s; }
            tokio::select! {
                msg = rx.recv() => match msg {
                    Some(DiscordMsg::Stop) | None => return,
                    Some(DiscordMsg::Set(a)) => { let _ = a; }
                    Some(DiscordMsg::Clear) => {}
                },
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(15)) => {}
            }
        };

        let hs = format!(r#"{{"v":1,"client_id":"{}"}}"#, client_id);
        if write_packet(&mut stream, 0, &hs).await.is_err() { continue; }
        if read_packet(&mut stream).await.is_err() { continue; }

        if write_packet(&mut stream, 1, &set_activity_payload(&initial)).await.is_err() { continue; }
        if read_packet(&mut stream).await.is_err() { continue; }

        loop {
            match rx.recv().await {
                Some(DiscordMsg::Set(a)) => {
                    if write_packet(&mut stream, 1, &set_activity_payload(&a)).await.is_err() { break; }
                    if read_packet(&mut stream).await.is_err() { break; }
                }
                Some(DiscordMsg::Clear) => {
                    write_packet(&mut stream, 1, &clear_activity_payload()).await.ok();
                    read_packet(&mut stream).await.ok();
                    break; // back to waiting for next Set
                }
                Some(DiscordMsg::Stop) | None => return,
            }
        }
    }
}
