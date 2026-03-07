// Project config – maps to projects/{name}/{name}.project.toml
//
// Naming convention (per RULES.md):
//   {name}.project.toml     → local deployment (this machine)
//   {name}.{host}.toml      → remote host deployment
//   {name}.federation.toml  → federation config

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use toml::Value;

use crate::error::FsnError;

/// Root structure of a project config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectMeta,

    #[serde(default)]
    pub load: ProjectLoad,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub domain: String,
    pub description: Option<String>,
    pub contact: Option<ContactInfo>,
    pub branding: Option<BrandingConfig>,
    pub sites: Option<IndexMap<String, SiteConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    pub email: Option<String>,
    pub acme_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingConfig {
    pub path: String,
    pub logo: Option<String>,
    pub logo_dark: Option<String>,
    pub favicon: Option<String>,
    pub theme_css: Option<String>,
    pub bg_pattern: Option<String>,
    pub meta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub path: String,
    pub domain: Option<String>,
}

/// The `[load]` table – which module instances to deploy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectLoad {
    /// key = instance name (e.g. "forgejo"), value = module reference
    #[serde(default)]
    pub modules: IndexMap<String, ModuleRef>,
}

/// A module instance declaration inside a project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleRef {
    /// Path to the module class: "{type}/{name}" (e.g. "git/forgejo")
    pub module_class: String,

    /// Optional per-instance variable overrides.
    #[serde(default)]
    pub vars: IndexMap<String, Value>,
}

impl ProjectConfig {
    /// Load a project config from a TOML file.
    pub fn load(path: &Path) -> Result<Self, FsnError> {
        let content = std::fs::read_to_string(path).map_err(|_| FsnError::ConfigNotFound {
            path: path.display().to_string(),
        })?;
        toml::from_str(&content).map_err(|e| FsnError::ConfigParse {
            path: path.display().to_string(),
            source: e,
        })
    }
}
