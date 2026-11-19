//! Local account metadata and refresh-token storage.
//!
//! Refresh tokens are kept out of instance configuration and logs. When a
//! platform keyring is available it is preferred; otherwise UMC falls back to
//! a permission-restricted file under `UMC_DATA_DIR/credentials`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const KEYRING_SERVICE: &str = "umc";
const DEFAULT_ACCOUNT_FILE: &str = "default";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Keyring,
    File,
}

impl Backend {
    fn as_str(self) -> &'static str {
        match self {
            Backend::Keyring => "keyring",
            Backend::File => "file",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BackendChoice {
    backend: Backend,
    warning: Option<&'static str>,
}

/// Non-secret account information safe to list over JSON-RPC or print.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub schema_version: u32,
    pub account_id: String,
    pub username: String,
    pub last_login: String,
    #[serde(default)]
    pub credential_backend: Option<String>,
}

/// A refresh token stored on the local machine.
#[derive(Serialize, Deserialize)]
struct StoredCredential {
    schema_version: u32,
    account_id: String,
    refresh_token: String,
    updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CredentialStore {
    root: PathBuf,
    /// Resolved lazily so ordinary commands never touch D-Bus or a keyring.
    choice: OnceLock<BackendChoice>,
}

impl CredentialStore {
    /// Creates a store, preferring the OS keyring and falling back to a
    /// permission-restricted file when the keyring is unavailable. Backend
    /// probing happens on first credential use.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            choice: OnceLock::new(),
        }
    }

    fn choice(&self) -> &BackendChoice {
        self.choice.get_or_init(choose_backend)
    }

    pub fn backend(&self) -> &'static str {
        self.choice().backend.as_str()
    }

    #[cfg(test)]
    pub(crate) fn file_store(root: impl Into<PathBuf>) -> Self {
        let choice = OnceLock::new();
        let _ = choice.set(BackendChoice {
            backend: Backend::File,
            warning: None,
        });
        Self {
            root: root.into(),
            choice,
        }
    }

    pub fn warning(&self) -> Option<&str> {
        self.choice().warning
    }

    fn credential_path(&self, account_id: &str) -> Result<PathBuf, String> {
        validate_account_id(account_id)?;
        Ok(self
            .root
            .join("credentials")
            .join(format!("{account_id}.json")))
    }

    pub async fn load_refresh_token(&self, account_id: &str) -> Result<Option<String>, String> {
        validate_account_id(account_id)?;
        if self.choice().backend == Backend::Keyring {
            match keyring_entry(account_id)
                .and_then(|entry| entry.get_password().map_err(|e| e.to_string()))
            {
                Ok(token) => return Ok(Some(token)),
                Err(_) => {
                    // Missing entry or an unavailable/locked keyring: fall through
                    // to the file backend so a fallback login remains usable.
                }
            }
        }
        let path = self.credential_path(account_id)?;
        crate::engine::ensure_no_symlink(&self.root, &path).await?;
        let raw = match tokio::fs::read(&path).await {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Could not read the stored credential: {error}")),
        };
        let stored: StoredCredential = serde_json::from_slice(&raw)
            .map_err(|_| "The stored credential is invalid; sign in again".to_string())?;
        validate_account_id(&stored.account_id)?;
        Ok(Some(stored.refresh_token))
    }

    /// Stores a refresh token. Returns a warning when the keyring failed and
    /// the token was placed in the file fallback instead.
    pub async fn save_refresh_token(
        &self,
        account_id: &str,
        refresh_token: &str,
    ) -> Result<Option<String>, String> {
        validate_account_id(account_id)?;
        if self.choice().backend == Backend::Keyring {
            match keyring_entry(account_id)
                .and_then(|entry| entry.set_password(refresh_token).map_err(|e| e.to_string()))
            {
                Ok(()) => {
                    let path = self.credential_path(account_id)?;
                    let _ = tokio::fs::remove_file(path).await;
                    return Ok(None);
                }
                Err(error) => {
                    self.write_credential_file(account_id, refresh_token)
                        .await?;
                    return Ok(Some(format!(
                        "OS keyring failed ({error}); the refresh token was stored in a 0600 file"
                    )));
                }
            }
        }
        self.write_credential_file(account_id, refresh_token)
            .await?;
        Ok(None)
    }

    async fn write_credential_file(
        &self,
        account_id: &str,
        refresh_token: &str,
    ) -> Result<(), String> {
        let stored = StoredCredential {
            schema_version: SCHEMA_VERSION,
            account_id: account_id.to_owned(),
            refresh_token: refresh_token.to_owned(),
            updated_at: crate::engine::timestamp_rfc3339(),
        };
        let bytes = serde_json::to_vec(&stored).map_err(|e| e.to_string())?;
        write_private_atomic(&self.root, &self.credential_path(account_id)?, &bytes).await
    }

    pub async fn delete_refresh_token(&self, account_id: &str) -> Result<(), String> {
        validate_account_id(account_id)?;
        if self.choice().backend == Backend::Keyring {
            if let Ok(entry) = keyring_entry(account_id) {
                let _ = entry.delete_credential();
            }
        }
        let path = self.credential_path(account_id)?;
        crate::engine::ensure_no_symlink(&self.root, &path).await?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Could not delete the stored credential: {error}")),
        }
    }

    pub async fn list_accounts(&self) -> Result<Vec<AccountRecord>, String> {
        let dir = self.root.join("accounts");
        crate::engine::ensure_no_symlink(&self.root, &dir).await?;
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("Could not read the account directory: {error}")),
        };
        let mut accounts = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Could not read the account directory: {e}"))?
        {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            if let Ok(raw) = tokio::fs::read(&path).await {
                if let Ok(record) = serde_json::from_slice::<AccountRecord>(&raw) {
                    accounts.push(record);
                }
            }
        }
        accounts.sort_by(|left, right| left.username.cmp(&right.username));
        Ok(accounts)
    }

    pub async fn load_account(&self, account_id: &str) -> Result<Option<AccountRecord>, String> {
        validate_account_id(account_id)?;
        let path = self
            .root
            .join("accounts")
            .join(format!("{account_id}.json"));
        crate::engine::ensure_no_symlink(&self.root, &path).await?;
        match tokio::fs::read(&path).await {
            Ok(raw) => serde_json::from_slice(&raw)
                .map(Some)
                .map_err(|_| "The stored account metadata is invalid".to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("Could not read the account metadata: {error}")),
        }
    }

    pub async fn save_account(&self, record: &AccountRecord) -> Result<(), String> {
        validate_account_id(&record.account_id)?;
        let path = self
            .root
            .join("accounts")
            .join(format!("{}.json", record.account_id));
        let bytes = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
        write_private_atomic(&self.root, &path, &bytes).await
    }

    pub async fn delete_account(&self, account_id: &str) -> Result<(), String> {
        validate_account_id(account_id)?;
        let path = self
            .root
            .join("accounts")
            .join(format!("{account_id}.json"));
        crate::engine::ensure_no_symlink(&self.root, &path).await?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Could not delete the account metadata: {error}")),
        }
    }

    pub async fn default_account(&self) -> Result<Option<String>, String> {
        let path = self.root.join("accounts").join(DEFAULT_ACCOUNT_FILE);
        crate::engine::ensure_no_symlink(&self.root, &path).await?;
        match tokio::fs::read_to_string(&path).await {
            Ok(value) => {
                let id = value.trim().to_owned();
                if id.is_empty() {
                    return Ok(None);
                }
                validate_account_id(&id)?;
                Ok(Some(id))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("Could not read the default account: {error}")),
        }
    }

    pub async fn set_default_account(&self, account_id: &str) -> Result<(), String> {
        validate_account_id(account_id)?;
        let path = self.root.join("accounts").join(DEFAULT_ACCOUNT_FILE);
        write_private_atomic(&self.root, &path, account_id.as_bytes()).await
    }

    pub async fn clear_default_account(&self, account_id: &str) -> Result<(), String> {
        if self.default_account().await?.as_deref() != Some(account_id) {
            return Ok(());
        }
        let path = self.root.join("accounts").join(DEFAULT_ACCOUNT_FILE);
        crate::engine::ensure_no_symlink(&self.root, &path).await?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Could not clear the default account: {error}")),
        }
    }

    /// Takes an inter-process lock for a single account. The returned file must
    /// stay alive for the duration of the refresh.
    pub fn lock_account(&self, account_id: &str) -> Result<std::fs::File, String> {
        use fs2::FileExt;
        validate_account_id(account_id)?;
        let dir = self.root.join("accounts");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        crate::engine::set_private_dir_blocking(&dir)?;
        let path = dir.join(format!("{account_id}.lock"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("Could not open the account lock: {e}"))?;
        crate::engine::set_private_file(&path)?;
        file.lock_exclusive()
            .map_err(|e| format!("Could not lock the account: {e}"))?;
        Ok(file)
    }
}

fn choose_backend() -> BackendChoice {
    let requested = std::env::var("UMC_CREDENTIAL_STORE")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if requested == "file" {
        return BackendChoice {
            backend: Backend::File,
            warning: None,
        };
    }
    if keyring_available() {
        return BackendChoice {
            backend: Backend::Keyring,
            warning: None,
        };
    }
    BackendChoice {
        backend: Backend::File,
        warning: Some("OS keyring is unavailable; refresh tokens will be stored in a 0600 file"),
    }
}

fn keyring_available() -> bool {
    if keyring::Entry::store_status().is_err() {
        return false;
    }
    match keyring::Entry::new(KEYRING_SERVICE, "__umc_probe__") {
        Ok(entry) => matches!(entry.get_password(), Ok(_) | Err(keyring::Error::NoEntry)),
        Err(_) => false,
    }
}

fn keyring_entry(account_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, account_id).map_err(|e| e.to_string())
}

pub fn validate_account_id(account_id: &str) -> Result<(), String> {
    if account_id.is_empty()
        || account_id.len() > 64
        || !account_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("Invalid account ID".into());
    }
    Ok(())
}

async fn write_private_atomic(root: &Path, path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid credential path".to_string())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| e.to_string())?;
    crate::engine::set_private_dir(parent).await?;
    crate::engine::ensure_no_symlink(root, path).await?;
    let temp = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    tokio::fs::write(&temp, bytes)
        .await
        .map_err(|e| format!("Could not write the credential file: {e}"))?;
    crate::engine::set_private_file(&temp)?;
    if cfg!(windows) && tokio::fs::try_exists(path).await.unwrap_or(false) {
        tokio::fs::remove_file(path)
            .await
            .map_err(|e| format!("Could not replace the credential file: {e}"))?;
    }
    tokio::fs::rename(&temp, path)
        .await
        .map_err(|e| format!("Could not finalize the credential file: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!("umc-credentials-{}", Uuid::new_v4()))
    }

    #[tokio::test]
    async fn file_store_round_trips_and_deletes() {
        let root = temp_root();
        let store = CredentialStore::file_store(root.clone());
        assert_eq!(store.load_refresh_token("abc").await.unwrap(), None);
        store
            .save_refresh_token("abc", "secret-token")
            .await
            .unwrap();
        assert_eq!(
            store.load_refresh_token("abc").await.unwrap().as_deref(),
            Some("secret-token")
        );
        store.delete_refresh_token("abc").await.unwrap();
        assert_eq!(store.load_refresh_token("abc").await.unwrap(), None);
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn account_metadata_round_trips() {
        let root = temp_root();
        let store = CredentialStore::file_store(root.clone());
        let record = AccountRecord {
            schema_version: SCHEMA_VERSION,
            account_id: "0123456789abcdef0123456789abcdef".into(),
            username: "Player".into(),
            last_login: crate::engine::timestamp_rfc3339(),
            credential_backend: Some("file".into()),
        };
        store.save_account(&record).await.unwrap();
        let loaded = store
            .load_account(&record.account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.username, "Player");
        assert_eq!(store.list_accounts().await.unwrap().len(), 1);
        store.set_default_account(&record.account_id).await.unwrap();
        assert_eq!(
            store.default_account().await.unwrap().as_deref(),
            Some(record.account_id.as_str())
        );
        store.delete_account(&record.account_id).await.unwrap();
        assert!(store.list_accounts().await.unwrap().is_empty());
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    #[test]
    fn rejects_path_traversal_in_account_ids() {
        assert!(validate_account_id("../escape").is_err());
        assert!(validate_account_id("a/b").is_err());
        assert!(validate_account_id("").is_err());
        assert!(validate_account_id("0123456789abcdef0123456789abcdef").is_ok());
    }
}
