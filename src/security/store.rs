use crate::security::SecretString;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cofre de credenciais indisponível")]
    Backend,
    #[error("senha não encontrada")]
    NotFound,
}

pub trait CredentialStore {
    fn save(&self, device_id: &str, secret: &SecretString) -> Result<(), StoreError>;
    fn load(&self, device_id: &str) -> Result<SecretString, StoreError>;
    fn delete(&self, device_id: &str) -> Result<(), StoreError>;
}

/// Cofre do SO (Windows Credential Manager).
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self {
            service: "simlite".to_string(),
        }
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for KeyringStore {
    fn save(&self, device_id: &str, secret: &SecretString) -> Result<(), StoreError> {
        let entry = keyring::Entry::new(&self.service, device_id).map_err(|_| StoreError::Backend)?;
        entry
            .set_password(secret.expose())
            .map_err(|_| StoreError::Backend)
    }

    fn load(&self, device_id: &str) -> Result<SecretString, StoreError> {
        let entry = keyring::Entry::new(&self.service, device_id).map_err(|_| StoreError::Backend)?;
        match entry.get_password() {
            Ok(p) => Ok(SecretString::new(p)),
            Err(keyring::Error::NoEntry) => Err(StoreError::NotFound),
            Err(_) => Err(StoreError::Backend),
        }
    }

    fn delete(&self, device_id: &str) -> Result<(), StoreError> {
        let entry = keyring::Entry::new(&self.service, device_id).map_err(|_| StoreError::Backend)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(StoreError::Backend),
        }
    }
}

#[derive(Default)]
pub struct MemoryStore {
    inner: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl CredentialStore for MemoryStore {
    fn save(&self, device_id: &str, secret: &SecretString) -> Result<(), StoreError> {
        self.inner
            .lock()
            .map_err(|_| StoreError::Backend)?
            .insert(device_id.to_string(), secret.expose().to_string());
        Ok(())
    }

    fn load(&self, device_id: &str) -> Result<SecretString, StoreError> {
        self.inner
            .lock()
            .map_err(|_| StoreError::Backend)?
            .get(device_id)
            .cloned()
            .map(SecretString::new)
            .ok_or(StoreError::NotFound)
    }

    fn delete(&self, device_id: &str) -> Result<(), StoreError> {
        self.inner
            .lock()
            .map_err(|_| StoreError::Backend)?
            .remove(device_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip_and_delete() {
        let store = MemoryStore::default();
        let secret = SecretString::new("hunter2");
        store.save("dvr-casa", &secret).unwrap();
        assert_eq!(store.load("dvr-casa").unwrap().expose(), "hunter2");
        store.delete("dvr-casa").unwrap();
        assert!(matches!(store.load("dvr-casa"), Err(StoreError::NotFound)));
    }

    #[test]
    fn store_error_does_not_include_secret() {
        let err = StoreError::NotFound.to_string();
        assert!(!err.contains("password"));
        assert!(!err.contains("hunter"));
    }
}
