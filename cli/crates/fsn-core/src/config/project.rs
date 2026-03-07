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
use crate::resource::Resource;

/// Root structure of a project config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectMeta,

    /// Typed service slots – which instance fills each role.
    #[serde(default)]
    pub services: ServiceSlots,

    #[serde(default)]
    pub load: ProjectLoad,
}

// ── Service Slots ─────────────────────────────────────────────────────────────

/// Typed service slots at the project level.
/// Other services and bots use these to find the right instance.
///
/// In project.toml:
/// [services]
/// iam  = "kanidm"
/// mail = "stalwart"
/// wiki = "outline"
/// git  = "forgejo"
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceSlots {
    pub iam:        Option<String>,
    pub mail:       Option<String>,
    pub wiki:       Option<String>,
    pub git:        Option<String>,
    pub chat:       Option<String>,
    pub collab:     Option<String>,
    pub tasks:      Option<String>,
    pub monitoring: Option<String>,
    #[serde(default, flatten)]
    pub extra: IndexMap<String, String>,
}

// ── Project Metadata ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub domain: String,
    pub description: Option<String>,

    /// Project version – increment to trigger config re-generation.
    #[serde(default = "default_version")]
    pub version: String,

    /// Primary language (IETF tag, e.g. "en", "de").
    #[serde(default = "default_lang")]
    pub language: String,

    /// Additional supported languages (ordered by preference).
    #[serde(default)]
    pub languages: Vec<String>,

    /// Base installation directory on the host (e.g. "/opt/fsn" or "~/fsn").
    /// Overrides the host-level default when set.
    #[serde(default)]
    pub install_dir: Option<String>,

    pub contact: Option<ContactInfo>,
    pub branding: Option<BrandingConfig>,
    pub sites: Option<IndexMap<String, SiteConfig>>,
}

fn default_version() -> String { "0.1.0".into() }
fn default_lang()    -> String { "en".into() }

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

// ── Load (instance declarations) ──────────────────────────────────────────────

/// The [load] table – which service instances to deploy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectLoad {
    /// key = instance name (e.g. "forgejo"), value = service entry.
    /// Alias "modules" accepted for backward compatibility with existing project files.
    #[serde(default, alias = "modules")]
    pub services: IndexMap<String, ServiceEntry>,
}

/// A service instance declaration inside a project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// Service class path, e.g. "git/forgejo".
    /// Alias "module_class" accepted for backward compatibility.
    #[serde(alias = "module_class")]
    pub service_class: String,

    #[serde(default)]
    pub vars: IndexMap<String, Value>,
}

/// Backwards-compat alias.
pub type ModuleRef = ServiceEntry;

impl ProjectConfig {
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

impl Resource for ProjectConfig {
    fn kind(&self) -> &'static str { "project" }

    fn validate(&self) -> anyhow::Result<()> {
        if self.project.name.is_empty() {
            anyhow::bail!("project.name is required");
        }
        if self.project.domain.is_empty() {
            anyhow::bail!("project.domain is required");
        }
        Ok(())
    }
}
