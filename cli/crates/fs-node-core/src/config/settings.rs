// Application settings – stored at ~/.config/fsn/settings.toml
//
// Contains user-level preferences: store URLs, UI language, service role assignments, etc.
// Loaded once at startup; saved when the user changes settings in the TUI.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use fs_error::FsyError;

// ── ServiceRoleMap ────────────────────────────────────────────────────────────

/// Maps service role IDs to the assigned container/service name.
///
/// Example: `"auth" → "kanidm"`, `"mail" → "stalwart"`, `"proxy" → "zentinel"`.
///
/// Carries its own assignment/lookup behavior instead of exposing raw map operations.
/// Serializes transparently so the TOML format is unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServiceRoleMap(HashMap<String, String>);

impl ServiceRoleMap {
    /// Returns the service name assigned to `role_id`, if any.
    pub fn get(&self, role_id: &str) -> Option<&str> {
        self.0.get(role_id).map(String::as_str)
    }

    /// Assign `service_name` to `role_id`.
    pub fn assign(&mut self, role_id: impl Into<String>, service_name: impl Into<String>) {
        self.0.insert(role_id.into(), service_name.into());
    }

    /// Remove the assignment for `role_id`.
    pub fn unassign(&mut self, role_id: &str) {
        self.0.remove(role_id);
    }

    /// Iterate over all `(role_id, service_name)` assignments.
    pub fn all(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Returns `true` if no roles are assigned.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ── AppSettings ───────────────────────────────────────────────────────────────

/// Global FSN application settings.
/// Persisted to `~/.config/fsn/settings.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Module stores to query when browsing or installing services.
    #[serde(default = "default_stores")]
    pub stores: Vec<StoreConfig>,

    /// Preferred UI language (BCP 47 code, e.g. "de", "fr").
    /// `None` = auto-detect from system locale.
    #[serde(default)]
    pub preferred_lang: Option<String>,

    /// Module IDs that have been installed (local copy synced from store).
    /// Example: ["zentinel", "kanidm"]
    #[serde(default)]
    pub installed_modules: Vec<String>,

    /// Service role assignments (role ID → container/service name).
    /// Example: `{ "auth" = "kanidm", "mail" = "stalwart", "proxy" = "zentinel" }`
    #[serde(default)]
    pub service_roles: ServiceRoleMap,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            stores: default_stores(),
            preferred_lang: None,
            installed_modules: Vec::new(),
            service_roles: ServiceRoleMap::default(),
        }
    }
}

impl AppSettings {
    /// Returns `true` if the module with the given ID is marked as installed.
    pub fn is_installed(&self, id: &str) -> bool {
        self.installed_modules.iter().any(|m| m == id)
    }

    /// Mark a module as installed (idempotent).
    pub fn mark_installed(&mut self, id: &str) {
        if !self.is_installed(id) {
            self.installed_modules.push(id.to_string());
        }
    }

    /// Remove a module from the installed list (idempotent).
    pub fn mark_uninstalled(&mut self, id: &str) {
        self.installed_modules.retain(|m| m != id);
    }
}

fn default_stores() -> Vec<StoreConfig> {
    vec![StoreConfig {
        name: "FSN Official".into(),
        url: "https://raw.githubusercontent.com/FreeSynergy/Store/main".into(),
        git_url: Some("https://github.com/FreeSynergy/Store.git".into()),
        local_path: None,
        enabled: true,
        primary: true,
    }]
}

// ── StoreConfig ───────────────────────────────────────────────────────────────

/// One configured module store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Display name shown in the TUI Settings screen.
    pub name: String,

    /// Base URL of the store (used for index.toml and raw file downloads).
    /// The catalog is fetched from `{url}/node/catalog.toml`.
    pub url: String,

    /// Git clone URL for syncing the full module tree locally.
    /// When absent, derived from `url` by stripping the raw.githubusercontent.com prefix.
    /// Example: "https://github.com/FreeSynergy/Store.git"
    #[serde(default)]
    pub git_url: Option<String>,

    /// Absolute local path to an already-checked-out Store directory.
    /// When set, `sync_modules` uses this path directly and skips git operations.
    /// Intended for development setups where the Store repo is already present.
    /// Example: "/home/kal/Server/FreeSynergy.Store"
    #[serde(default)]
    pub local_path: Option<String>,

    /// Whether this store is actively queried.
    /// Disabled stores are shown in Settings but not used.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether this is the built-in official store.
    /// Primary stores can be toggled but not edited or deleted by the user.
    #[serde(default)]
    pub primary: bool,
}

fn default_true() -> bool {
    true
}

// ── Load / Save ───────────────────────────────────────────────────────────────

impl AppSettings {
    /// Load settings from `~/.config/fsn/settings.toml`.
    /// Returns `Default` when the file does not exist.
    pub fn load() -> Result<Self, FsyError> {
        let path = settings_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(|e| FsyError::Parse(format!("{}: {e}", path.display())))
    }

    /// Persist settings to `~/.config/fsn/settings.toml`.
    pub fn save(&self) -> Result<(), FsyError> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content =
            toml::to_string_pretty(self).map_err(|e| FsyError::Internal(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// Returns the platform-appropriate settings file path.
/// Uses `$HOME/.config/fsn/settings.toml` (XDG-compatible).
fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".config")
        .join("fsn")
        .join("settings.toml")
}

// ── Container Plugins directory ────────────────────────────────────────────────

/// Resolve the directory that holds container definitions.
///
/// Priority (first match wins):
///   1. `FS_PLUGINS_DIR` environment variable — explicit override.
///   2. First enabled store with a `local_path` set → `{local_path}/node/containers/`.
///   3. Fallback: `{node_root}/containers/`.
///
/// Callers pass the FSN workspace root so the fallback path always resolves
/// even when no settings file or env var is present.
pub fn resolve_plugins_dir(node_root: &std::path::Path) -> PathBuf {
    if let Some(dir) = resolve_plugins_dir_no_fallback() {
        return dir;
    }
    node_root.join("containers")
}

/// Resolve the plugins directory without requiring a `node_root` fallback.
///
/// Returns `None` when neither env var nor settings provide a path.
/// Used in contexts (TUI, web API) that do not have access to the Node workspace root.
///
/// Priority:
///   1. `FS_PLUGINS_DIR` environment variable.
///   2. First enabled store with a `local_path` → `{local_path}/Node/`.
pub fn resolve_plugins_dir_no_fallback() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("FS_PLUGINS_DIR") {
        return Some(PathBuf::from(dir));
    }
    if let Ok(settings) = AppSettings::load() {
        if let Some(store) = settings
            .stores
            .iter()
            .find(|s| s.enabled && s.local_path.is_some())
        {
            let base = PathBuf::from(store.local_path.as_deref().unwrap());
            return Some(base.join("node").join("containers"));
        }
    }
    None
}

// ── ServiceRoleRegistry ───────────────────────────────────────────────────────

/// Scans all module TOML files and builds a map of role → providers.
///
/// Used by the Settings UI to populate the service role selector dropdowns.
/// Call `ServiceRoleRegistry::build_from_dir(modules_dir)` on startup.
#[derive(Debug, Default, Clone)]
pub struct ServiceRoleRegistry {
    /// Maps role ID → list of module names that provide it.
    pub providers: HashMap<String, Vec<String>>,
}

/// Minimal TOML shape for extracting roles from a module file.
#[derive(Deserialize)]
struct MinimalModuleFile {
    #[serde(rename = "module")]
    meta: MinimalModuleMeta,
}

#[derive(Deserialize)]
struct MinimalModuleMeta {
    name: String,
    #[serde(default)]
    roles: MinimalRoles,
}

#[derive(Deserialize, Default)]
struct MinimalRoles {
    #[serde(default)]
    provides: Vec<String>,
}

impl ServiceRoleRegistry {
    /// Build the registry by walking `modules_dir` and parsing all `*.toml` files.
    ///
    /// Errors in individual files are silently skipped — partial results are
    /// always better than a startup crash.
    pub fn build_from_dir(modules_dir: &std::path::Path) -> Self {
        let mut providers: HashMap<String, Vec<String>> = HashMap::new();

        if !modules_dir.exists() {
            return Self { providers };
        }

        for entry in walkdir_toml(modules_dir) {
            let Ok(content) = std::fs::read_to_string(&entry) else {
                continue;
            };
            let Ok(parsed) = toml::from_str::<MinimalModuleFile>(&content) else {
                continue;
            };
            for role in parsed.meta.roles.provides {
                providers
                    .entry(role)
                    .or_default()
                    .push(parsed.meta.name.clone());
            }
        }

        Self { providers }
    }

    /// Returns all module names that claim to provide `role_id`.
    pub fn providers_for(&self, role_id: &str) -> &[String] {
        self.providers
            .get(role_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Returns all role IDs seen across all modules.
    pub fn all_roles(&self) -> impl Iterator<Item = &String> {
        self.providers.keys()
    }
}

/// Walk `dir` recursively and return paths to all `*.toml` files.
fn walkdir_toml(dir: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            result.extend(walkdir_toml(&path));
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            result.push(path);
        }
    }
    result
}
