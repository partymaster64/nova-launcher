use std::io::Read;
use std::path::Path;
use base64::Engine;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

// ─── Minimal NBT cursor ───────────────────────────────────────────────────────

struct Cur<'a> { data: &'a [u8], pos: usize }

impl<'a> Cur<'a> {
    fn new(data: &'a [u8]) -> Self { Self { data, pos: 0 } }

    fn u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }
    fn u16be(&mut self) -> Option<u16> {
        let b: [u8; 2] = self.data.get(self.pos..self.pos+2)?.try_into().ok()?;
        self.pos += 2;
        Some(u16::from_be_bytes(b))
    }
    fn i32be(&mut self) -> Option<i32> {
        let b: [u8; 4] = self.data.get(self.pos..self.pos+4)?.try_into().ok()?;
        self.pos += 4;
        Some(i32::from_be_bytes(b))
    }
    fn nbt_string(&mut self) -> Option<String> {
        let len = self.u16be()? as usize;
        let bytes = self.data.get(self.pos..self.pos+len)?;
        let s = String::from_utf8_lossy(bytes).into_owned();
        self.pos += len;
        Some(s)
    }
    fn skip_payload(&mut self, tag: u8) -> Option<()> {
        match tag {
            1 => { self.pos += 1; Some(()) }
            2 => { self.pos += 2; Some(()) }
            3 => { self.pos += 4; Some(()) }
            4 => { self.pos += 8; Some(()) }
            5 => { self.pos += 4; Some(()) }
            6 => { self.pos += 8; Some(()) }
            7 => { let n = self.i32be()? as usize; self.pos = self.pos.checked_add(n)?; Some(()) }
            8 => { self.nbt_string()?; Some(()) }
            9 => {
                let elem = self.u8()?;
                let n = self.i32be()?.max(0) as usize;
                for _ in 0..n { self.skip_payload(elem)?; }
                Some(())
            }
            10 => {
                loop {
                    let t = self.u8()?;
                    if t == 0 { break; }
                    self.nbt_string()?;
                    self.skip_payload(t)?;
                }
                Some(())
            }
            11 => { let n = self.i32be()? as usize; self.pos = self.pos.checked_add(n * 4)?; Some(()) }
            12 => { let n = self.i32be()? as usize; self.pos = self.pos.checked_add(n * 8)?; Some(()) }
            _ => None,
        }
    }
}

// ─── World NBT reading ────────────────────────────────────────────────────────

pub fn read_level_name(world_dir: &Path) -> Option<String> {
    let raw = std::fs::read(world_dir.join("level.dat")).ok()?;
    let mut dec = flate2::read::GzDecoder::new(raw.as_slice());
    let mut decompressed = Vec::new();
    dec.read_to_end(&mut decompressed).ok()?;

    let mut cur = Cur::new(&decompressed);

    // Root TAG_Compound header: type byte + 2-byte name length + name bytes
    let root_type = cur.u8()?;
    if root_type != 10 { return None; }
    let name_len = cur.u16be()? as usize;
    cur.pos += name_len; // skip root name

    // Walk root compound looking for "Data" (tag type 10)
    loop {
        let t = cur.u8()?;
        if t == 0 { break; }
        let key = cur.nbt_string()?;
        if t == 10 && key == "Data" {
            // Inside Data compound, find "LevelName" (tag type 8)
            loop {
                let dt = cur.u8()?;
                if dt == 0 { break; }
                let dkey = cur.nbt_string()?;
                if dt == 8 && dkey == "LevelName" {
                    return cur.nbt_string();
                }
                cur.skip_payload(dt)?;
            }
            return None;
        }
        cur.skip_payload(t)?;
    }
    None
}

pub fn read_world_icon(world_dir: &Path) -> Option<String> {
    let bytes = std::fs::read(world_dir.join("icon.png")).ok()?;
    Some(B64.encode(&bytes))
}

// ─── Server NBT reading ───────────────────────────────────────────────────────

#[derive(serde::Serialize, Clone, Debug)]
pub struct ServerNbt {
    pub name: String,
    pub ip: String,
    pub icon: Option<String>, // base64 PNG (without data: prefix)
}

pub fn read_servers(instance_dir: &Path) -> Vec<ServerNbt> {
    let raw = match std::fs::read(instance_dir.join("servers.dat")) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    // servers.dat may be uncompressed NBT (modern MC) or gzip-compressed — try both
    let decompressed: Vec<u8> = if raw.starts_with(&[0x1f, 0x8b]) {
        let mut dec = flate2::read::GzDecoder::new(raw.as_slice());
        let mut buf = Vec::new();
        if dec.read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        buf
    } else {
        raw
    };

    let mut cur = Cur::new(&decompressed);

    // Root TAG_Compound header
    let root_type = match cur.u8() {
        Some(t) => t,
        None => return Vec::new(),
    };
    if root_type != 10 { return Vec::new(); }
    let name_len = match cur.u16be() {
        Some(n) => n as usize,
        None => return Vec::new(),
    };
    cur.pos += name_len;

    let mut servers = Vec::new();

    // Find "servers" TAG_List
    loop {
        let t = match cur.u8() {
            Some(v) => v,
            None => break,
        };
        if t == 0 { break; }
        let key = match cur.nbt_string() {
            Some(k) => k,
            None => break,
        };
        if t == 9 && key == "servers" {
            // TAG_List: elem type byte + count i32
            let elem_type = match cur.u8() {
                Some(e) => e,
                None => break,
            };
            let count = match cur.i32be() {
                Some(c) => c.max(0) as usize,
                None => break,
            };
            if elem_type != 10 { break; } // expect list of compounds
            for _ in 0..count {
                let mut name = String::new();
                let mut ip = String::new();
                let mut icon: Option<String> = None;
                loop {
                    let ft = match cur.u8() {
                        Some(v) => v,
                        None => return servers,
                    };
                    if ft == 0 { break; }
                    let fkey = match cur.nbt_string() {
                        Some(k) => k,
                        None => return servers,
                    };
                    if ft == 8 && fkey == "name" {
                        name = cur.nbt_string().unwrap_or_default();
                    } else if ft == 8 && fkey == "ip" {
                        ip = cur.nbt_string().unwrap_or_default();
                    } else if ft == 8 && fkey == "icon" {
                        let raw_icon = cur.nbt_string().unwrap_or_default();
                        // Strip data URL prefix if present
                        let stripped = raw_icon
                            .strip_prefix("data:image/png;base64,")
                            .unwrap_or(&raw_icon)
                            .to_string();
                        icon = if stripped.is_empty() { None } else { Some(stripped) };
                    } else {
                        if cur.skip_payload(ft).is_none() { return servers; }
                    }
                }
                if !ip.is_empty() {
                    servers.push(ServerNbt { name, ip, icon });
                }
            }
            break;
        }
        if cur.skip_payload(t).is_none() { break; }
    }

    // Deduplicate by normalized address (strip default :25565 port)
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<ServerNbt> = servers.into_iter().filter(|s| {
        let normalized = s.ip.strip_suffix(":25565").unwrap_or(&s.ip).to_lowercase();
        seen.insert(normalized)
    }).collect();
    deduped
}

// ─── SLP (Server List Ping) ───────────────────────────────────────────────────

#[derive(serde::Serialize, Clone, Debug)]
pub struct PingResult {
    pub motd_html: String,
    pub online: u32,
    pub max: u32,
    pub version: Option<String>,
    pub favicon: Option<String>, // base64 PNG
    pub latency_ms: u64,
}

fn write_varint(buf: &mut Vec<u8>, val: i32) {
    let mut uval = val as u32;
    loop {
        let mut b = (uval & 0x7F) as u8;
        uval >>= 7;
        if uval != 0 { b |= 0x80; }
        buf.push(b);
        if uval == 0 { break; }
    }
}

async fn read_varint_async(stream: &mut tokio::net::TcpStream) -> std::io::Result<i32> {
    use tokio::io::AsyncReadExt;
    let mut result = 0i32;
    let mut shift = 0u32;
    loop {
        let mut buf = [0u8; 1];
        stream.read_exact(&mut buf).await?;
        let b = buf[0];
        result |= ((b & 0x7F) as i32) << shift;
        if b & 0x80 == 0 { break; }
        shift += 7;
        if shift >= 35 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "varint overflow"));
        }
    }
    Ok(result)
}

fn named_color_to_hex(name: &str) -> Option<&'static str> {
    match name {
        "black" => Some("#000000"),
        "dark_blue" => Some("#0000AA"),
        "dark_green" => Some("#00AA00"),
        "dark_aqua" => Some("#00AAAA"),
        "dark_red" => Some("#AA0000"),
        "dark_purple" => Some("#AA00AA"),
        "gold" => Some("#FFAA00"),
        "gray" => Some("#AAAAAA"),
        "dark_gray" => Some("#555555"),
        "blue" => Some("#5555FF"),
        "green" => Some("#55FF55"),
        "aqua" => Some("#55FFFF"),
        "red" => Some("#FF5555"),
        "light_purple" => Some("#FF55FF"),
        "yellow" => Some("#FFFF55"),
        "white" => Some("#FFFFFF"),
        _ => None,
    }
}

fn section_codes_to_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 32);
    let mut open_spans: u32 = 0;
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{00A7}' {
            if let Some(code) = chars.next() {
                let code = code.to_ascii_lowercase();
                let color = match code {
                    '0' => Some("#000000"),
                    '1' => Some("#0000AA"),
                    '2' => Some("#00AA00"),
                    '3' => Some("#00AAAA"),
                    '4' => Some("#AA0000"),
                    '5' => Some("#AA00AA"),
                    '6' => Some("#FFAA00"),
                    '7' => Some("#AAAAAA"),
                    '8' => Some("#555555"),
                    '9' => Some("#5555FF"),
                    'a' => Some("#55FF55"),
                    'b' => Some("#55FFFF"),
                    'c' => Some("#FF5555"),
                    'd' => Some("#FF55FF"),
                    'e' => Some("#FFFF55"),
                    'f' => Some("#FFFFFF"),
                    _ => None,
                };
                if let Some(hex) = color {
                    out.push_str("<span style=\"color:");
                    out.push_str(hex);
                    out.push_str("\">");
                    open_spans += 1;
                } else if code == 'l' {
                    out.push_str("<span style=\"font-weight:bold\">");
                    open_spans += 1;
                } else if code == 'o' {
                    out.push_str("<span style=\"font-style:italic\">");
                    open_spans += 1;
                } else if code == 'r' {
                    for _ in 0..open_spans { out.push_str("</span>"); }
                    open_spans = 0;
                }
            }
        } else {
            match ch {
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '&' => out.push_str("&amp;"),
                c => out.push(c),
            }
        }
    }
    for _ in 0..open_spans { out.push_str("</span>"); }
    out
}

fn description_to_html(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => section_codes_to_html(s),
        serde_json::Value::Object(obj) => {
            let text = obj.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let color = obj.get("color").and_then(|v| v.as_str());
            let bold = obj.get("bold").and_then(|v| v.as_bool()).unwrap_or(false);

            let mut style = String::new();
            if let Some(c) = color {
                let hex = named_color_to_hex(c).unwrap_or(c);
                style.push_str(&format!("color:{};", hex));
            }
            if bold {
                style.push_str("font-weight:bold;");
            }

            let mut out = String::new();
            let has_style = !style.is_empty();
            if has_style {
                out.push_str(&format!("<span style=\"{}\">", style));
            }
            out.push_str(&section_codes_to_html(text));
            if has_style {
                out.push_str("</span>");
            }

            // Recurse into "extra" array
            if let Some(serde_json::Value::Array(extra)) = obj.get("extra") {
                for item in extra {
                    out.push_str(&description_to_html(item));
                }
            }
            out
        }
        _ => String::new(),
    }
}

pub async fn ping_server(host: &str, port: u16) -> Result<PingResult, String> {
    use tokio::io::AsyncWriteExt;
    use tokio::io::AsyncReadExt;
    use std::time::Instant;

    let addr = format!("{}:{}", host, port);
    let connect_future = tokio::net::TcpStream::connect(&addr);
    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        connect_future,
    )
    .await
    .map_err(|_| "Connection timeout".to_string())?
    .map_err(|e| e.to_string())?;

    // Build handshake packet payload
    let mut handshake_payload = Vec::new();
    write_varint(&mut handshake_payload, 0x00); // packet id
    write_varint(&mut handshake_payload, 47);   // protocol version
    write_varint(&mut handshake_payload, host.len() as i32);
    handshake_payload.extend_from_slice(host.as_bytes());
    handshake_payload.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake_payload, 1); // next state: status

    // Wrap in length-prefixed packet
    let mut handshake_packet = Vec::new();
    write_varint(&mut handshake_packet, handshake_payload.len() as i32);
    handshake_packet.extend_from_slice(&handshake_payload);

    stream.write_all(&handshake_packet).await.map_err(|e| e.to_string())?;

    // Send status request packet
    stream.write_all(&[0x01, 0x00]).await.map_err(|e| e.to_string())?;

    let t0 = Instant::now();

    // Read response with 5-second timeout
    let read_future = async {
        let _pkt_len = read_varint_async(&mut stream).await?;
        let _pkt_id = read_varint_async(&mut stream).await?;
        let str_len = read_varint_async(&mut stream).await?;
        let str_len = str_len.max(0) as usize;
        let mut json_bytes = vec![0u8; str_len];
        stream.read_exact(&mut json_bytes).await?;
        Ok::<Vec<u8>, std::io::Error>(json_bytes)
    };

    let json_bytes = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        read_future,
    )
    .await
    .map_err(|_| "Read timeout".to_string())?
    .map_err(|e| e.to_string())?;

    let latency_ms = t0.elapsed().as_millis() as u64;

    let json_str = String::from_utf8_lossy(&json_bytes);
    let json: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;

    let motd_html = json.get("description")
        .map(|d| description_to_html(d))
        .unwrap_or_default();

    let online = json.pointer("/players/online")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let max = json.pointer("/players/max")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let version = json.pointer("/version/name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let favicon = json.get("favicon")
        .and_then(|v| v.as_str())
        .map(|s| {
            s.strip_prefix("data:image/png;base64,")
                .unwrap_or(s)
                .to_string()
        });

    Ok(PingResult {
        motd_html,
        online,
        max,
        version,
        favicon,
        latency_ms,
    })
}
