// Service class definition – maps to modules/{type}/{name}/{name}.toml
//
// Field order (MANDATORY per RULES.md):
//   module → vars → load → container → environment → setup
//
// The TOML key `[module]` is kept for file-level compatibility;
// internally we use `ServiceMeta` / `ServiceClass`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use toml::Value;

use crate::error::FsnError;
use crate::resource::Resource;

// ── Service Type ──────────────────────────────────────────────────────────────

/// The functional role of a service.
/// Determines which typed interface the service exposes and which
/// project slots it can fill (IAM, Mail, Wiki, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    /// Identity & Access Management (Kanidm, Keycloak, …)
    Iam,
    /// Reverse proxy / ingress (Zentinel, …)
    Proxy,
    /// Mail server (Stalwart, …)
    Mail,
    /// Git hosting (Forgejo, Gitea, …)
    Git,
    /// Wiki / knowledge base (Outline, BookStack, …)
    Wiki,
    /// Team chat / Matrix (Tuwunel, …)
    Chat,
    /// Collaborative editing (CryptPad, …)
    Collab,
    /// Issue / task tracker (Vikunja, …)
    Tasks,
    /// Ticketing / shop (Pretix, …)
    Tickets,
    /// Maps & geo (uMap, …)
    Maps,
    /// Observability / metrics (OpenObserver, …)
    Monitoring,
    /// Relational database (Postgres) – internal, not user-facing
    Database,
    /// Key-value cache (Dragonfly/Redis) – internal, not user-facing
    Cache,
    /// Bot / automation (Matrix bot, Telegram bot, …)
    Bot,
    /// User-defined type
    #[serde(other)]
    #[default]
    Custom,
}

impl std::fmt::Display for ServiceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ServiceType::Iam        => "iam",
            ServiceType::Proxy      => "proxy",
            ServiceType::Mail       => "mail",
            ServiceType::Git        => "git",
            ServiceType::Wiki       => "wiki",
            ServiceType::Chat       => "chat",
            ServiceType::Collab     => "collab",
            ServiceType::Tasks      => "tasks",
            ServiceType::Tickets    => "tickets",
            ServiceType::Maps       => "maps",
            ServiceType::Monitoring => "monitoring",
            ServiceType::Database   => "database",
            ServiceType::Cache      => "cache",
            ServiceType::Bot        => "bot",
            ServiceType::Custom     => "custom",
        };
        write!(f, "{s}")
    }
}

impl ServiceType {
    /// Returns `true` for types that are internal dependencies
    /// (not user-facing, no subdomain, no proxy route).
    pub fn is_internal(&self) -> bool {
        matches!(self, ServiceType::Database | ServiceType::Cache)
    }
}

// ── Service Class ─────────────────────────────────────────────────────────────

/// A service class definition (the template/blueprint for a service).
/// Loaded from modules/{type}/{name}/{name}.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceClass {
    /// Metadata block – TOML key is `[module]` for file compatibility.
    #[serde(rename = "module")]
    pub meta: ServiceMeta,

    #[serde(default)]
    pub vars: IndexMap<String, Value>,

    #[serde(default)]
    pub load: ServiceLoad,

    pub container: ContainerDef,

    #[serde(default)]
    pub environment: IndexMap<String, String>,

    /// Setup wizard configuration – what this service needs before it can run.
    #[serde(default)]
    pub setup: ServiceSetup,
}

// ── Setup wizard types ────────────────────────────────────────────────────────

/// All configuration fields this service requires during `fsn init`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceSetup {
    #[serde(default)]
    pub fields: Vec<SetupField>,
}

/// A single field the wizard will prompt for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupField {
    /// Key to set: "vault_*" → stored in vault, anything else → env reminder.
    pub key: String,

    /// English label shown in prompt AND used as .po lookup key.
    pub label: String,

    /// Optional longer explanation shown below the prompt.
    pub description: Option<String>,

    #[serde(default)]
    pub field_type: FieldType,

    /// Auto-generate a random value; user can press Enter to accept or type override.
    #[serde(default)]
    pub auto_generate: bool,

    /// Pre-filled default value shown in the prompt.
    pub default: Option<String>,

    /// For FieldType::Select – the available choices.
    #[serde(default)]
    pub options: Vec<String>,

    /// Skip this field if the key already exists in vault (idempotent).
    #[serde(default = "default_true")]
    pub skip_if_set: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    #[default]
    String,
    Secret,  // masked input, stored in vault
    Email,
    Ip,
    Select,  // requires `options`
    Bool,
}

// ── Service Metadata ──────────────────────────────────────────────────────────

/// Core metadata declared under the `[module]` TOML key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMeta {
    pub name: String,

    #[serde(default)]
    pub alias: Vec<String>,

    /// Functional type – determines typed interface and project slot.
    /// TOML key: `type`.
    #[serde(rename = "type", default)]
    pub service_type: ServiceType,

    pub author: Option<String>,
    pub version: String,

    #[serde(default)]
    pub tags: Vec<String>,

    pub description: Option<String>,
    pub website: Option<String>,
    pub repository: Option<String>,

    /// Primary internal port the service listens on.
    pub port: u16,

    #[serde(default)]
    pub constraints: Constraints,

    pub federation: Option<FederationMeta>,

    /// Path used by Zentinel upstream health checks.
    pub health_path: Option<String>,
    pub health_port: Option<u16>,
    pub health_scheme: Option<String>,
}

/// Deployment constraints declared per service class.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Constraints {
    /// Maximum number of instances of this service class per host (null = unlimited).
    pub per_host: Option<u32>,

    /// Maximum number of instances of this service class per IP (null = unlimited).
    pub per_ip: Option<u32>,

    /// Locality constraint – if Some(SameHost), must run on same host as consumer.
    pub locality: Option<Locality>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Locality {
    SameHost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationMeta {
    pub enabled: bool,
    pub min_trust: u8,
}

// ── Load / Dependencies ───────────────────────────────────────────────────────

/// Sub-service and service references declared under `[load]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceLoad {
    /// Sub-services this service owns and creates (e.g. postgres, dragonfly).
    /// TOML key: `modules` kept for file compatibility.
    #[serde(default, alias = "modules")]
    pub sub_services: IndexMap<String, SubServiceRef>,

    /// Other services whose config this service reads (no ownership).
    #[serde(default)]
    pub services: IndexMap<String, ServiceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubServiceRef {
    /// Class key, e.g. "database/postgres".
    /// TOML: `module_class` or `service_class` (both accepted).
    #[serde(alias = "module_class")]
    pub service_class: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceRef {}

// ── Container Definition ──────────────────────────────────────────────────────

/// Container definition – maps to the `[container]` TOML block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerDef {
    pub name: String,
    pub image: String,
    pub image_tag: String,

    /// Auto-generated by engine – NEVER set manually in service TOML.
    #[serde(default)]
    pub networks: Vec<String>,

    #[serde(default)]
    pub volumes: Vec<String>,

    /// Forbidden on all services except proxy/zentinel.
    #[serde(default)]
    pub published_ports: Vec<String>,

    pub healthcheck: Option<HealthCheck>,

    /// Run as a specific UID[:GID] (e.g. "1000" or "15371:15371").
    pub user: Option<String>,

    #[serde(default)]
    pub read_only: bool,

    #[serde(default)]
    pub tmpfs: Vec<String>,

    #[serde(default)]
    pub security_opt: Vec<String>,

    #[serde(default)]
    pub ulimits: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub cmd: String,
    pub interval: String,
    pub timeout: String,
    pub retries: u32,
    pub start_period: String,
}

// ── Resource impl for ServiceClass ────────────────────────────────────────────

impl Resource for ServiceClass {
    fn kind(&self) -> &'static str { "service_class" }
    fn id(&self) -> &str { &self.meta.name }
    fn display_name(&self) -> &str { &self.meta.name }
    fn description(&self) -> Option<&str> { self.meta.description.as_deref() }
    fn tags(&self) -> &[String] { &self.meta.tags }

    fn validate(&self) -> Result<(), FsnError> {
        if self.meta.name.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "module.name is required".into() });
        }
        if self.meta.version.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "module.version is required".into() });
        }
        if self.container.image.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "container.image is required".into() });
        }
        Ok(())
    }
}
