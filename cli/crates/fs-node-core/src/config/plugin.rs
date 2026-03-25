// Plugin configuration – helper configs per service type.
//
// Plugins are NOT modules. They provide type-level configuration
// (e.g. DNS providers, ACME providers) that all modules of the same type can use.
//
// File layout:  containers/{name}/plugins/{plugin_type}/{name}.toml
// Example key:  "zentinel/dns/hetzner"
//
// Field order (mandatory): plugin → vars

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::resource::Resource;

/// Root structure of a plugin config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub plugin: PluginMeta,

    /// Key-value pairs injected into the Jinja2 template context.
    /// Values may reference `{{ vault_* }}` secrets.
    #[serde(default)]
    pub vars: IndexMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    /// Machine name (e.g. "hetzner", "letsencrypt").
    pub name: String,

    /// Plugin category (e.g. "dns", "acme").
    #[serde(rename = "type")]
    pub plugin_type: String,

    pub description: Option<String>,
}

impl PluginConfig {
    pub fn load(path: &std::path::Path) -> Result<Self, fs_error::FsyError> {
        crate::config::load_toml(path)
    }
}

impl Resource for PluginConfig {
    fn kind(&self) -> &'static str {
        "plugin"
    }
    fn id(&self) -> &str {
        &self.plugin.name
    }
    fn description(&self) -> Option<&str> {
        self.plugin.description.as_deref()
    }
    fn tags(&self) -> &[String] {
        &[]
    }

    fn validate(&self) -> Result<(), fs_error::FsyError> {
        if self.plugin.name.is_empty() {
            return Err(fs_error::FsyError::Config("plugin.name is required".into()));
        }
        if self.plugin.plugin_type.is_empty() {
            return Err(fs_error::FsyError::Config("plugin.type is required".into()));
        }
        Ok(())
    }
}
