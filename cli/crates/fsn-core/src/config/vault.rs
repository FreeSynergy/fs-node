// Vault config – maps to projects/{name}/vault.toml (git-ignored)
//
// Rules (per RULES.md):
//   - All keys MUST have the "vault_" prefix
//   - Never logged, never shown in Debug output
//   - Auto-generated on first install, never overwritten on re-run

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::FsnError;

/// Vault file – all values are secret strings (zero-on-drop, never logged).
#[derive(Debug)]
pub struct VaultConfig {
    values: HashMap<String, SecretString>,
}

/// Raw deserialization target – plain strings, converted to SecretString after load.
#[derive(Deserialize)]
struct RawVault(HashMap<String, String>);

impl VaultConfig {
    /// Load vault.toml. Returns empty vault if file does not exist.
    pub fn load(path: &Path) -> Result<Self, FsnError> {
        if !path.exists() {
            return Ok(Self {
                values: HashMap::new(),
            });
        }
        let content = std::fs::read_to_string(path).map_err(FsnError::Io)?;
        let raw: RawVault = toml::from_str(&content).map_err(|e| FsnError::ConfigParse {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(Self {
            values: raw
                .0
                .into_iter()
                .map(|(k, v)| (k, SecretString::from(v)))
                .collect(),
        })
    }

    /// Look up a vault key. Returns None if not present.
    pub fn get(&self, key: &str) -> Option<&SecretString> {
        self.values.get(key)
    }

    /// Expose a secret value for template rendering. Use sparingly.
    pub fn expose(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.expose_secret())
    }

    /// All vault keys (for template variable injection – values NOT exposed here).
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }
}

/// Placeholder Serialize so VaultConfig can be included in larger structs.
/// Values are NEVER serialized – this emits an empty map.
impl Serialize for VaultConfig {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let map = s.serialize_map(Some(0))?;
        map.end()
    }
}
