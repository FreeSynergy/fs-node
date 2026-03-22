use fs_error::FsyError;
// Project config – maps to projects/{name}/{name}.project.toml
//
// Naming convention (per RULES.md):
//   {name}.project.toml     → local deployment (this machine)
//   {name}.{host}.toml      → remote host deployment
//   {name}.federation.toml  → federation config

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use toml::Value;

use crate::config::meta::ResourceMeta;
use crate::config::service::ServiceType;
use crate::resource::{ProjectResource, Resource, ServiceResource};

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
    /// Common fields: name, alias, description, version, tags.
    #[serde(flatten)]
    pub meta: ResourceMeta,

    pub domain: String,

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

fn default_lang() -> String { "en".into() }

impl ServiceSlots {
    /// Look up which instance fills the given slot by name.
    ///
    /// Checks the typed fields first, then falls back to `extra`.
    /// Returns `None` when the slot is not assigned in this project.
    pub fn find(&self, slot: &str) -> Option<&str> {
        match slot {
            "iam"        => self.iam.as_deref(),
            "mail"       => self.mail.as_deref(),
            "wiki"       => self.wiki.as_deref(),
            "git"        => self.git.as_deref(),
            "chat"       => self.chat.as_deref(),
            "collab"     => self.collab.as_deref(),
            "tasks"      => self.tasks.as_deref(),
            "monitoring" => self.monitoring.as_deref(),
            other        => self.extra.get(other).map(String::as_str),
        }
    }
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

    /// Display alias, also used as subdomain override.
    pub alias: Option<String>,

    /// Which host slug this service runs on.
    pub host: Option<String>,

    /// Subdomain prefix → {subdomain}.{project.domain}. Defaults to instance name.
    pub subdomain: Option<String>,

    /// Port override (uses service-class default when absent).
    pub port: Option<u16>,

    /// Image version / tag.
    #[serde(default = "default_service_version")]
    pub version: String,

    /// Free-form tags.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Instance-level environment variable overrides.
    /// Merged on top of the service class's [environment] block during resolution.
    #[serde(default)]
    pub env: IndexMap<String, String>,

    #[serde(default)]
    pub vars: IndexMap<String, Value>,
}

fn default_service_version() -> String { "latest".into() }

/// Backwards-compat alias.
pub type ModuleRef = ServiceEntry;

// ── Standalone service instance file ──────────────────────────────────────────

/// Full service instance config stored in its own file.
/// Maps to: projects/{project}/services/{name}.service.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstanceConfig {
    pub service: ServiceInstanceMeta,

    /// Environment variable overrides: KEY → value.
    #[serde(default)]
    pub vars: IndexMap<String, Value>,

    /// Optional human-readable comments for each var, keyed by var name.
    /// Written as [vars_comments] in the TOML file; UI-only, not used by the deployer.
    #[serde(default)]
    pub vars_comments: IndexMap<String, String>,
}

/// Metadata block inside a standalone service instance file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstanceMeta {
    /// Common fields: name, alias, description, version, tags.
    #[serde(flatten)]
    pub meta: ResourceMeta,

    /// Service class path, e.g. "git/forgejo".
    pub service_class: String,

    /// Which project this service belongs to (project slug).
    pub project: String,

    /// Which host slug this service runs on.
    pub host: Option<String>,

    /// Subdomain prefix → {subdomain}.{project.domain}.
    pub subdomain: Option<String>,

    /// Port override (uses service-class default when absent).
    pub port: Option<u16>,

    /// External service — no container, no environment, only variables.
    /// When `true`, the deploy pipeline skips container creation.
    #[serde(default)]
    pub external: bool,

    /// Git repository of the deployed code (optional metadata).
    pub git_repo: Option<String>,

    /// Public website URL (optional metadata).
    pub website: Option<String>,

    /// Bot names attached to this service.
    #[serde(default)]
    pub bots: Vec<String>,
}

impl ServiceInstanceConfig {
    pub fn load(path: &std::path::Path) -> Result<Self, fs_error::FsyError> {
        crate::config::load_toml_validated(path, crate::config::validate::TomlKind::Service)
    }
}

impl Resource for ServiceInstanceConfig {
    fn kind(&self) -> &'static str { "service" }
    fn id(&self) -> &str { &self.service.meta.name }
    fn display_name(&self) -> &str { self.service.meta.display_name() }
    fn tags(&self) -> &[String] { &self.service.meta.tags }

    fn validate(&self) -> Result<(), FsyError> {
        if self.service.meta.name.is_empty() {
            return Err(FsyError::Config("service.name is required".into()));
        }
        if self.service.service_class.is_empty() {
            return Err(FsyError::Config("service.service_class is required".into()));
        }
        if self.service.project.is_empty() {
            return Err(FsyError::Config("service.project is required".into()));
        }
        Ok(())
    }
}

impl ServiceResource for ServiceInstanceConfig {
    fn service_class(&self) -> &str { &self.service.service_class }
    fn host(&self)          -> Option<&str> { self.service.host.as_deref() }
    fn subdomain(&self)     -> Option<&str> { self.service.subdomain.as_deref() }
    fn port(&self)          -> Option<u16>  { self.service.port }
    fn project(&self)       -> &str { &self.service.project }
}

impl ProjectConfig {
    pub fn load(path: &Path) -> Result<Self, FsyError> {
        crate::config::load_toml_validated(path, crate::config::validate::TomlKind::Project)
    }
}

impl Resource for ProjectConfig {
    fn kind(&self) -> &'static str { "project" }
    fn id(&self) -> &str { &self.project.meta.name }
    fn display_name(&self) -> &str { self.project.meta.display_name() }
    fn description(&self) -> Option<&str> { self.project.meta.description.as_deref() }
    fn tags(&self) -> &[String] { &self.project.meta.tags }

    fn validate(&self) -> Result<(), FsyError> {
        if self.project.meta.name.is_empty() {
            return Err(FsyError::Config("project.name is required".into()));
        }
        if self.project.domain.is_empty() {
            return Err(FsyError::Config("project.domain is required".into()));
        }
        Ok(())
    }
}

impl ProjectResource for ProjectConfig {
    fn domain(&self) -> &str { &self.project.domain }
    fn contact_email(&self) -> Option<&str> {
        self.project.contact.as_ref()
            .and_then(|c| c.email.as_deref().or(c.acme_email.as_deref()))
    }
    fn languages(&self) -> &[String] { &self.project.languages }
    fn install_dir(&self) -> Option<&str> { self.project.install_dir.as_deref() }
}

impl ProjectConfig {
    /// Pre-compute cross-service variables from the project's service entries.
    ///
    /// Produces `PROJECT_NAME`, `PROJECT_DOMAIN`, `PROJECT_EMAIL` plus per-service
    /// vars like `MAIL_HOST`, `IAM_URL`, `GIT_DOMAIN`, etc., derived from instance
    /// names and the project domain.
    ///
    /// Uses `ServiceType::from_class_prefix()` + `ServiceType::exported_contract()`
    /// as the single source of truth — no local match block.
    pub fn cross_service_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        vars.insert("PROJECT_NAME".into(),   self.project.meta.name.clone());
        vars.insert("PROJECT_DOMAIN".into(), self.project.domain.clone());
        if let Some(email) = self.contact_email() {
            vars.insert("PROJECT_EMAIL".into(), email.to_string());
        }

        for (instance_name, entry) in &self.load.services {
            let class_prefix = entry.service_class.split('/').next().unwrap_or("");
            let Some(stype)    = ServiceType::from_class_prefix(class_prefix) else { continue };
            let Some(contract) = stype.exported_contract()                    else { continue };

            let subdomain = entry.subdomain.as_deref().unwrap_or(instance_name.as_str());
            let domain    = format!("{}.{}", subdomain, self.project.domain);
            let port      = entry.port.unwrap_or(0);

            vars.extend(contract.resolve(instance_name, &domain, port));
        }

        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_config_parses_minimal_toml() {
        let config: ProjectConfig = toml::from_str(r#"
[project]
name = "myproject"
domain = "example.com"
        "#).unwrap();
        assert_eq!(config.project.meta.name, "myproject");
        assert_eq!(config.project.domain, "example.com");
        assert_eq!(config.project.language, "en"); // default
    }

    #[test]
    fn project_config_parses_services() {
        let config: ProjectConfig = toml::from_str(r#"
[project]
name = "myproject"
domain = "example.com"

[load.services.forgejo]
service_class = "git/forgejo"
version = "9"
        "#).unwrap();
        let entry = config.load.services.get("forgejo").unwrap();
        assert_eq!(entry.service_class, "git/forgejo");
        assert_eq!(entry.version, "9");
    }

    #[test]
    fn service_entry_version_defaults_to_latest() {
        let config: ProjectConfig = toml::from_str(r#"
[project]
name = "myproject"
domain = "example.com"

[load.services.forgejo]
service_class = "git/forgejo"
        "#).unwrap();
        assert_eq!(config.load.services["forgejo"].version, "latest");
    }

    #[test]
    fn project_config_parses_contact_sub_table() {
        let config: ProjectConfig = toml::from_str(r#"
[project]
name = "myproject"
domain = "example.com"

[project.contact]
email = "admin@example.com"
        "#).unwrap();
        let contact = config.project.contact.as_ref().unwrap();
        assert_eq!(contact.email.as_deref(), Some("admin@example.com"));
    }

    #[test]
    fn project_resource_impl_returns_correct_values() {
        use crate::resource::{ProjectResource, Resource};
        let config: ProjectConfig = toml::from_str(r#"
[project]
name = "testproject"
domain = "test.example.com"
        "#).unwrap();
        assert_eq!(config.id(), "testproject");
        assert_eq!(config.domain(), "test.example.com");
        assert_eq!(config.kind(), "project");
    }

    #[test]
    fn service_instance_config_parses_toml() {
        let config: ServiceInstanceConfig = toml::from_str(r#"
[service]
name = "forgejo"
service_class = "git/forgejo"
project = "myproject"
host = "myhost"
        "#).unwrap();
        assert_eq!(config.service.meta.name, "forgejo");
        assert_eq!(config.service.service_class, "git/forgejo");
        assert_eq!(config.service.project, "myproject");
        assert_eq!(config.service.host.as_deref(), Some("myhost"));
    }
}
