//! Secret storage: the Windows Credential Manager, or on Android the Android Keystore (through
//! Spaceport's plugin). Secrets never touch plain files.

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

/// Android: secrets encrypted with a key that lives in the Android Keystore (see
/// `SpaceportPlugin.kt`).
#[cfg(target_os = "android")]
pub struct KeystoreStore<R: tauri::Runtime>(pub tauri::plugin::PluginHandle<R>);

#[cfg(target_os = "android")]
impl<R: tauri::Runtime> SecretStore for KeystoreStore<R> {
    fn get(&self, name: &str) -> Result<Option<String>, SecretError> {
        #[derive(serde::Deserialize)]
        struct Found {
            #[serde(default)]
            value: Option<String>,
        }
        self.0
            .run_mobile_plugin::<Found>("secretGet", serde_json::json!({ "name": name }))
            .map(|found| found.value)
            .map_err(|e| SecretError(e.to_string()))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), SecretError> {
        self.0
            .run_mobile_plugin::<serde_json::Value>(
                "secretSet",
                serde_json::json!({ "name": name, "value": value }),
            )
            .map(|_| ())
            .map_err(|e| SecretError(e.to_string()))
    }

    fn delete(&self, name: &str) -> Result<(), SecretError> {
        self.0
            .run_mobile_plugin::<serde_json::Value>(
                "secretDelete",
                serde_json::json!({ "name": name }),
            )
            .map(|_| ())
            .map_err(|e| SecretError(e.to_string()))
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
