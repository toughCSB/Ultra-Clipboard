use std::sync::Arc;

use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "com.toughcsb.ultraclipboard.sync";

#[derive(Debug, thiserror::Error)]
#[error("sync peer credential is unavailable")]
pub struct SecretError;

pub struct SecretValue(Zeroizing<String>);

impl SecretValue {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

pub trait SecretStore: Send + Sync {
    fn set(&self, reference: &str, secret: &str) -> Result<(), SecretError>;
    fn get(&self, reference: &str) -> Result<SecretValue, SecretError>;
    fn delete(&self, reference: &str) -> Result<(), SecretError>;
}

#[derive(Default)]
pub struct OsSecretStore(std::sync::Mutex<()>);

impl SecretStore for OsSecretStore {
    fn set(&self, reference: &str, secret: &str) -> Result<(), SecretError> {
        let _guard = self.0.lock().map_err(|_| SecretError)?;
        entry(reference)?
            .set_password(secret)
            .map_err(|_| SecretError)
    }

    fn get(&self, reference: &str) -> Result<SecretValue, SecretError> {
        let _guard = self.0.lock().map_err(|_| SecretError)?;
        entry(reference)?
            .get_password()
            .map(|secret| SecretValue(Zeroizing::new(secret)))
            .map_err(|_| SecretError)
    }

    fn delete(&self, reference: &str) -> Result<(), SecretError> {
        let _guard = self.0.lock().map_err(|_| SecretError)?;
        entry(reference)?
            .delete_credential()
            .map_err(|_| SecretError)
    }
}

fn entry(reference: &str) -> Result<keyring::Entry, SecretError> {
    if reference.trim().is_empty() {
        return Err(SecretError);
    }
    keyring::Entry::new(KEYRING_SERVICE, reference).map_err(|_| SecretError)
}

pub type SharedSecretStore = Arc<dyn SecretStore>;

#[cfg(test)]
pub struct MemorySecretStore {
    values: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

#[cfg(test)]
impl MemorySecretStore {
    pub fn new() -> Self {
        Self {
            values: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[cfg(test)]
impl SecretStore for MemorySecretStore {
    fn set(&self, reference: &str, secret: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .map_err(|_| SecretError)?
            .insert(reference.to_owned(), secret.to_owned());
        Ok(())
    }

    fn get(&self, reference: &str) -> Result<SecretValue, SecretError> {
        self.values
            .lock()
            .map_err(|_| SecretError)?
            .get(reference)
            .cloned()
            .map(|secret| SecretValue(Zeroizing::new(secret)))
            .ok_or(SecretError)
    }

    fn delete(&self, reference: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .map_err(|_| SecretError)?
            .remove(reference)
            .map(|_| ())
            .ok_or(SecretError)
    }
}
