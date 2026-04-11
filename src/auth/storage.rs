use anyhow::Result;
use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE_NAME: &str = "nova-launcher";

/// Gespeichertes Konto mit allen Auth-Tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccount {
    pub uuid: String,
    pub username: String,
    pub minecraft_token: String,
    pub refresh_token: String,
    /// Unix-Timestamp wann der Minecraft-Token abläuft
    pub token_expires_at: i64,
}

impl StoredAccount {
    pub fn is_token_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.token_expires_at - 60 // 60s Puffer
    }
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
