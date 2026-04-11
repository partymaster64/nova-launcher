use anyhow::Result;
use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE_NAME: &str = "nova-launcher";

/// Gespeichertes Konto mit allen Auth-Tokens
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredAccount {
    pub uuid: String,
    pub username: String,
    pub minecraft_token: String,
    pub refresh_token: String,
    /// Unix-Timestamp wann der Minecraft-Token abläuft
    pub token_expires_at: i64,
    /// true = Offline-Konto (kein Microsoft-Login)
    #[serde(default)]
    pub offline: bool,
}

impl StoredAccount {
    /// Erstellt ein Offline-Konto. UUID wird deterministisch aus dem Username abgeleitet (UUID v3, DNS-Namespace).
    pub fn new_offline(username: &str) -> Self {
        Self {
            uuid: offline_uuid(username),
            username: username.to_string(),
            minecraft_token: "0".to_string(),
            refresh_token: String::new(),
            token_expires_at: i64::MAX, // läuft nie ab
            offline: true,
        }
    }

    pub fn is_token_expired(&self) -> bool {
        if self.offline {
            return false;
        }
        let now = chrono::Utc::now().timestamp();
        now >= self.token_expires_at - 60 // 60s Puffer
    }
}

/// Deterministischen UUID v3 aus Benutzername ableiten (DNS-Namespace, wie Prism Launcher).
fn offline_uuid(username: &str) -> String {
    uuid::Uuid::new_v3(&uuid::Uuid::NAMESPACE_DNS, username.as_bytes()).to_string()
}

/// Alle gespeicherten Konten (als JSON im Keyring)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub accounts: Vec<StoredAccount>,
}

impl AccountStore {
    fn keyring_entry() -> Result<Entry> {
        Ok(Entry::new(SERVICE_NAME, "accounts")?)
    }

    pub fn load() -> Result<Self> {
        let entry = Self::keyring_entry()?;
        match entry.get_password() {
            Ok(json) => {
                let store = serde_json::from_str(&json).unwrap_or_default();
                Ok(store)
            }
            Err(keyring::Error::NoEntry) => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        let entry = Self::keyring_entry()?;
        let json = serde_json::to_string(self)?;
        entry.set_password(&json)?;
        Ok(())
    }

    /// Speichert auf einem echten OS-Thread (kein Tokio-Context → kein zbus-Panic).
    /// Gibt das Ergebnis über einen oneshot-Channel zurück, den man in async awaiten kann.
    pub async fn save_async(&self) -> Result<()> {
        let clone = self.clone();
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<()>>();
        std::thread::spawn(move || {
            let _ = tx.send(clone.save());
        });
        rx.await.map_err(|_| anyhow::anyhow!("Keyring-Thread gestorben"))??;
        Ok(())
    }

    pub fn add_or_update(&mut self, account: StoredAccount) {
        if let Some(existing) = self.accounts.iter_mut().find(|a| a.uuid == account.uuid) {
            *existing = account;
        } else {
            self.accounts.push(account);
        }
    }

    pub fn remove(&mut self, uuid: &str) {
        self.accounts.retain(|a| a.uuid != uuid);
    }

    pub fn get(&self, uuid: &str) -> Option<&StoredAccount> {
        self.accounts.iter().find(|a| a.uuid == uuid)
    }
}
