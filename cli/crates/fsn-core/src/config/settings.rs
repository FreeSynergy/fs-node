// Application settings – stored at ~/.config/fsn/settings.toml
//
// Contains user-level preferences: store URLs, UI language, etc.
// Loaded once at startup; saved when the user changes settings in the TUI.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::FsnError;

// ── AppSettings ───────────────────────────────────────────────────────────────

/// Global FSN application settings.
/// Persisted to `~/.config/fsn/settings.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Module stores to query when browsing or installing services.
    #[serde(default = "default_stores")]
    pub stores: Vec<StoreConfig>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { stores: default_stores() }
    }
}

fn default_stores() -> Vec<StoreConfig> {
    vec![StoreConfig {
        name:    "FSN Official".into(),
        url:     "https://raw.githubusercontent.com/Lord-KalEl/FreeSynergy.Node/main".into(),
        enabled: true,
    }]
}

// ── StoreConfig ───────────────────────────────────────────────────────────────

/// One configured module store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Display name shown in the TUI Settings screen.
    pub name: String,

    /// Base URL of the store.
    /// The index is fetched from `{url}/store/index.toml`.
    pub url: String,

    /// Whether this store is actively queried.
    /// Disabled stores are shown in Settings but not used.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

// ── Load / Save ───────────────────────────────────────────────────────────────

impl AppSettings {
    /// Load settings from `~/.config/fsn/settings.toml`.
    /// Returns `Default` when the file does not exist.
    pub fn load() -> Result<Self, FsnError> {
        let path = settings_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(|e| FsnError::ConfigParse {
            path: path.display().to_string(),
            source: e,
        })
    }

    /// Persist settings to `~/.config/fsn/settings.toml`.
    pub fn save(&self) -> Result<(), FsnError> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| FsnError::Template(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// Returns the platform-appropriate settings file path.
/// Uses `$HOME/.config/fsn/settings.toml` (XDG-compatible).
fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("fsn").join("settings.toml")
}
