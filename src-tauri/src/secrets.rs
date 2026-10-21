//! Secret storage. On Windows this is the Credential Manager; secrets never touch plain files.

const SERVICE: &str = "Spaceport";

#[derive(Debug, thiserror::Error)]
#[error("credential store error: {0}")]
pub struct SecretError(String);

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, name: &str, value: &str) -> Result<(), SecretError>;
    /// Removing a secret that does not exist is not an error.
    fn delete(&self, name: &str) -> Result<(), SecretError>;
}

/// Stores each secret as a generic credential named `Spaceport` with the secret name as user.
pub struct KeyringStore;

impl KeyringStore {
    fn entry(name: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, name).map_err(|e| SecretError(e.to_string()))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, name: &str) -> Result<Option<String>, SecretError> {
        match Self::entry(name)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), SecretError> {
        Self::entry(name)?
            .set_password(value)
            .map_err(|e| SecretError(e.to_string()))
    }

    fn delete(&self, name: &str) -> Result<(), SecretError> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }
}

/// In-memory store for tests.
#[cfg(test)]
pub mod testing {
    use super::{SecretError, SecretStore};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct MemoryStore(Mutex<HashMap<String, String>>);

    impl SecretStore for MemoryStore {
        fn get(&self, name: &str) -> Result<Option<String>, SecretError> {
            Ok(self.0.lock().unwrap().get(name).cloned())
        }

        fn set(&self, name: &str, value: &str) -> Result<(), SecretError> {
            self.0
                .lock()
                .unwrap()
                .insert(name.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, name: &str) -> Result<(), SecretError> {
            self.0.lock().unwrap().remove(name);
            Ok(())
        }
    }
}
