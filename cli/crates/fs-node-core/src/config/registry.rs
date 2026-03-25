use fs_error::FsyError;
// Resource registry – scans resources/ directory and loads all resource
// class TOMLs and plugin TOMLs.
//
// Resource layout:
//   Depth 3: resources/{kind}/{name}/{name}.toml         → key "{kind}/{name}"
//   Depth 4: resources/{kind}/{parent}/{name}/{name}.toml → key "{kind}/{parent}/{name}"
//
//   {kind} is one of: apps | containers | bots | widgets
//
// Plugin layout:
//   Depth 4: resources/plugins/{plugin_type}/{name}.toml → key "plugins/{plugin_type}/{name}"

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::config::plugin::PluginConfig;
use crate::config::service::ServiceClass;

/// In-memory index of all available resource classes and plugins.
///
/// Class key  = "{kind}/{name}"              (e.g. "apps/zentinel", "containers/forgejo")
/// Plugin key = "plugins/{plugin_type}/{name}" (e.g. "plugins/dns/hetzner")
#[derive(Debug, Default)]
pub struct ServiceRegistry {
    classes: HashMap<String, ServiceClass>,
    plugins: HashMap<String, PluginConfig>,
    /// Base path of the resources/ directory
    resources_dir: PathBuf,
}

impl ServiceRegistry {
    /// Scan a resources/ directory and load all class TOMLs and plugin TOMLs.
    ///
    /// Resource layout (classes):
    ///   Depth 3: `resources/{kind}/{name}/{name}.toml`            → key = `{kind}/{name}`
    ///   Depth 4: `resources/{kind}/{parent}/{name}/{name}.toml`   → key = `{kind}/{parent}/{name}`
    ///
    ///   {kind} is one of: apps | containers | bots | widgets
    ///
    /// Plugin layout:
    ///   Depth 4: `resources/plugins/{plugin_type}/{name}.toml`    → key = `plugins/{plugin_type}/{name}`
    pub fn load(resources_dir: &Path) -> Result<Self, FsyError> {
        let mut registry = Self {
            classes: HashMap::new(),
            plugins: HashMap::new(),
            resources_dir: resources_dir.to_path_buf(),
        };

        // ── Resource class scan (depth 3–4) ──────────────────────────────────
        for entry in WalkDir::new(resources_dir)
            .min_depth(3)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }

            // File name must match its parent directory (e.g. forgejo/forgejo.toml)
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            let parent_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or_default();

            if file_stem != parent_name {
                continue;
            }

            // Skip plugin directories (handled separately below)
            if path.components().any(|c| c.as_os_str() == "plugins") {
                continue;
            }

            // Compute depth relative to resources_dir to pick the right key format
            let depth = path
                .components()
                .count()
                .saturating_sub(resources_dir.components().count());

            let class_key = if depth == 3 {
                // resources/{kind}/{name}/{name}.toml  →  {kind}/{name}
                let kind = path
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                format!("{kind}/{file_stem}")
            } else {
                // resources/{kind}/{parent}/{name}/{name}.toml  →  {kind}/{parent}/{name}
                let grandparent_dir = path
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                let kind = path
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                format!("{kind}/{grandparent_dir}/{file_stem}")
            };

            match Self::load_class(path) {
                Ok(class) => {
                    registry.classes.insert(class_key, class);
                }
                Err(e) => {
                    eprintln!("Warning: skipping {}: {}", path.display(), e);
                }
            }
        }

        // ── Plugin scan: resources/plugins/{plugin_type}/{name}.toml ─────────
        let plugins_dir = resources_dir.join("plugins");
        if plugins_dir.exists() {
            for entry in WalkDir::new(&plugins_dir)
                .min_depth(2)
                .max_depth(2)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                    continue;
                }

                // Key: "plugins/{plugin_type}/{name}"  e.g. "plugins/dns/hetzner"
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                let plugin_type = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                let plugin_key = format!("plugins/{plugin_type}/{name}");

                match Self::load_plugin(path) {
                    Ok(plugin) => {
                        registry.plugins.insert(plugin_key, plugin);
                    }
                    Err(e) => {
                        eprintln!("Warning: skipping plugin {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(registry)
    }

    fn load_class(path: &Path) -> Result<ServiceClass, FsyError> {
        let content = std::fs::read_to_string(path).map_err(FsyError::Io)?;
        let p = path.display().to_string();
        toml::from_str(&content).map_err(|e| FsyError::Parse(format!("{p}: {e}")))
    }

    fn load_plugin(path: &Path) -> Result<PluginConfig, FsyError> {
        let content = std::fs::read_to_string(path).map_err(FsyError::Io)?;
        let p = path.display().to_string();
        toml::from_str(&content).map_err(|e| FsyError::Parse(format!("{p}: {e}")))
    }

    /// Look up a module class by its "{type}/{name}" key.
    pub fn get(&self, class_key: &str) -> Option<&ServiceClass> {
        self.classes.get(class_key)
    }

    /// Look up a plugin by plugin type and name.
    ///
    /// Example: `get_plugin("dns", "hetzner")`
    pub fn get_plugin(&self, plugin_type: &str, name: &str) -> Option<&PluginConfig> {
        let key = format!("plugins/{plugin_type}/{name}");
        self.plugins.get(&key)
    }

    /// All loaded module classes.
    pub fn all(&self) -> impl Iterator<Item = (&str, &ServiceClass)> {
        self.classes.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// All loaded plugins.
    pub fn all_plugins(&self) -> impl Iterator<Item = (&str, &PluginConfig)> {
        self.plugins.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn resources_dir(&self) -> &Path {
        &self.resources_dir
    }
}
