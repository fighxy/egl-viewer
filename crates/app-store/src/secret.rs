//! The Eagle API token lives in the OS secret store, never in a JSON file (docs/spec.md
//! §8.3 "Auth", D-010). Windows Credential Manager through `keyring`.

use std::collections::HashMap;
use std::sync::Mutex;

use library_core::SourceError;

/// The service name in the OS secret store.
const SERVICE: &str = "egl-viewer";

/// One secret, addressed by a name such as `eagle-token:192.168.1.20:41595`.
pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<String>, SourceError>;
    fn set(&self, name: &str, value: &str) -> Result<(), SourceError>;
}

/// Windows Credential Manager (docs/architecture.md §4.6).
pub struct KeyringStore;

impl SecretStore for KeyringStore {
    fn get(&self, name: &str) -> Result<Option<String>, SourceError> {
        let entry = keyring::Entry::new(SERVICE, name).map_err(keyring_error)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keyring_error(error)),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), SourceError> {
        let entry = keyring::Entry::new(SERVICE, name).map_err(keyring_error)?;
        entry.set_password(value).map_err(keyring_error)
    }
}

fn keyring_error(error: keyring::Error) -> SourceError {
    SourceError::Storage(format!("secret store: {error}"))
}

/// For tests and platforms without a credential manager.
#[derive(Default)]
pub struct MemoryStore {
    values: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<String>, SourceError> {
        Ok(self
            .values
            .lock()
            .map_err(|_| SourceError::Storage("secret store lock".into()))?
            .get(name)
            .cloned())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), SourceError> {
        self.values
            .lock()
            .map_err(|_| SourceError::Storage("secret store lock".into()))?
            .insert(name.to_owned(), value.to_owned());
        Ok(())
    }
}

/// The secret name for the token of one Eagle endpoint.
pub fn eagle_token_name(host: &str, port: u16) -> String {
    format!("eagle-token:{host}:{port}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryStore::default();
        assert_eq!(store.get("eagle-token:127.0.0.1:41595").unwrap(), None);
        store.set("eagle-token:127.0.0.1:41595", "sekret").unwrap();
        assert_eq!(
            store.get("eagle-token:127.0.0.1:41595").unwrap().as_deref(),
            Some("sekret")
        );
        // Every endpoint keeps its own token.
        assert_eq!(store.get("eagle-token:pc:41595").unwrap(), None);
    }

    #[test]
    fn token_names_carry_the_endpoint() {
        assert_eq!(
            eagle_token_name("192.168.1.20", 41595),
            "eagle-token:192.168.1.20:41595"
        );
    }
}
