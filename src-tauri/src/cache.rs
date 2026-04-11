use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use serde::{Deserialize, Serialize};

const CACHE_FILE: &str = "http_cache.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    data: serde_json::Value,
    expires_at: i64, // unix timestamp (seconds)
}

pub struct HttpCache {
    inner: Mutex<HashMap<String, CacheEntry>>,
    cache_dir: PathBuf,
}

impl HttpCache {
    pub fn load(cache_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&cache_dir).ok();
        let inner = Self::read_from_disk(&cache_dir);
        Self { inner: Mutex::new(inner), cache_dir }
    }

    fn read_from_disk(cache_dir: &Path) -> HashMap<String, CacheEntry> {
        let path = cache_dir.join(CACHE_FILE);
        let data = std::fs::read_to_string(path).unwrap_or_default();
        let now = unix_now();
        serde_json::from_str::<HashMap<String, CacheEntry>>(&data)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, e)| e.expires_at > now)
            .collect()
    }

    fn flush(&self, inner: &HashMap<String, CacheEntry>) {
        let path = self.cache_dir.join(CACHE_FILE);
        if let Ok(json) = serde_json::to_string(inner) {
            std::fs::write(path, json).ok();
        }
    }

    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        let inner = self.inner.lock().ok()?;
        let entry = inner.get(key)?;
        if entry.expires_at > unix_now() {
            Some(entry.data.clone())
        } else {
            None
        }
    }

    pub fn set(&self, key: String, data: serde_json::Value, ttl_secs: i64) {
        if let Ok(mut inner) = self.inner.lock() {
            let now = unix_now();
            // Evict expired entries
            inner.retain(|_, e| e.expires_at > now);
            // Cap at 100 entries: drop those expiring soonest
            const MAX_ENTRIES: usize = 100;
            if inner.len() >= MAX_ENTRIES {
                if let Some(oldest_key) = inner
                    .iter()
                    .min_by_key(|(_, e)| e.expires_at)
                    .map(|(k, _)| k.clone())
                {
                    inner.remove(&oldest_key);
                }
            }
            inner.insert(key, CacheEntry { data, expires_at: now + ttl_secs });
            self.flush(&inner);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.clear();
            std::fs::remove_file(self.cache_dir.join(CACHE_FILE)).ok();
        }
    }

    pub fn size_bytes(&self) -> u64 {
        self.cache_dir.join(CACHE_FILE)
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0)
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
