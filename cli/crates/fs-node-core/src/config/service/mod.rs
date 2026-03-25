use fs_error::FsyError;
// Service class definition – maps to containers/{name}/{name}.toml
//
// Design Pattern: Module split:
//   types.rs     — ServiceType enum + de_service_types (role classification)
//   mod.rs       — ServiceMeta, ServiceClass, ContainerDef, ServiceContract,
//                  ServiceLifecycle, setup types, …
//
// Field order (MANDATORY per RULES.md):
//   module → vars → load → container → environment → setup → lifecycle
//
// The TOML key `[module]` is kept for file-level compatibility;
// internally we use `ServiceMeta` / `ServiceClass`.

pub mod types;

pub use types::{de_service_types, Capability, ExportedVarContract, ServiceType};

use indexmap::{IndexMap, IndexSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use toml::Value;

// ── Schema helpers ─────────────────────────────────────────────────────────────

/// JSON-Schema helper for `IndexMap<String, toml::Value>` fields.
/// `toml::Value` has no JsonSchema impl — we accept any JSON object here.
fn schema_any_object(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!(true)
}

use crate::config::manifest::ModuleManifest;

use crate::resource::Resource;

// ── Service Class ─────────────────────────────────────────────────────────────

/// A service class definition (the template/blueprint for a service).
/// Loaded from containers/{name}/{name}.toml.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceClass {
    /// Metadata block – TOML key is `[module]` for file compatibility.
    #[serde(rename = "module")]
    pub meta: ServiceMeta,

    /// Jinja2 template variables declared by the module.
    /// Any TOML value is accepted; validated at render time.
    #[serde(default)]
    #[schemars(schema_with = "schema_any_object")]
    pub vars: IndexMap<String, Value>,

    #[serde(default)]
    pub load: ServiceLoad,

    /// Container deployment – present for container-based resources.
    /// Absent for native apps (those use `service` instead).
    #[serde(default)]
    pub container: Option<ContainerDef>,

    /// Native app deployment – present for resources deployed as systemd services.
    /// Absent for container-based resources (those use `container` instead).
    #[serde(default)]
    pub service: Option<NativeServiceDef>,

    #[serde(default)]
    pub environment: IndexMap<String, String>,

    /// Setup wizard configuration – what this service needs before it can run.
    #[serde(default)]
    pub setup: ServiceSetup,

    /// Routing contract – what the service exposes to the proxy.
    /// Proxy modules iterate over all contracts to generate routing config.
    #[serde(default)]
    pub contract: ServiceContract,

    /// Plugin manifest – commands, inputs and outputs for the process plugin protocol.
    /// Absent for modules that have not yet been migrated to the plugin system.
    #[serde(default, rename = "plugin")]
    #[schemars(schema_with = "schema_any_object")]
    pub manifest: Option<ModuleManifest>,

    /// Documentation entries for environment variables.
    /// Shown in the Container Manager's Catalog editor view.
    #[serde(default)]
    pub variables: Vec<VariableDef>,

    /// Lifecycle hooks — what to do on install, update, swap, decommission.
    #[serde(default)]
    pub lifecycle: ServiceLifecycle,
}

// ── Service Contract ──────────────────────────────────────────────────────────

/// Routing and capability contract declared by a service module.
///
/// The proxy driver reads `ServiceContract` to generate per-service routing
/// config — analogous to a Kubernetes `Ingress` spec.  The service declares
/// what it needs; the proxy decides how to implement it.
///
/// Empty `routes` = no proxy routing generated (internal services).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceContract {
    /// HTTP routes this service exposes. Empty = proxy skips this service.
    #[serde(default)]
    pub routes: Vec<RouteSpec>,

    /// Extra HTTP headers the proxy injects when forwarding to this service.
    #[serde(default)]
    pub headers: Vec<HeaderSpec>,

    /// Whether the container speaks TLS internally.
    /// `true` → proxy uses HTTPS to reach the container (e.g. Kanidm).
    /// `false` (default) → proxy speaks plain HTTP to the container.
    #[serde(default)]
    pub upstream_tls: bool,

    /// Override the proxy health-check path for this service.
    /// Falls back to `module.health_path` when absent.
    pub health_path: Option<String>,
}

/// A URL route this service exposes through the proxy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RouteSpec {
    /// Unique identifier within this module (e.g. "main", "admin", "api").
    pub id: String,

    /// URL path prefix to match (e.g. "/" or "/auth").
    pub path: String,

    /// Strip the matched prefix before forwarding to the upstream.
    #[serde(default)]
    pub strip: bool,

    /// Human-readable description (shown in TUI and generated docs).
    pub description: Option<String>,
}

/// An HTTP header the proxy injects when forwarding requests.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HeaderSpec {
    /// Header name (e.g. "X-Forwarded-Proto").
    pub name: String,
    /// Header value — Jinja2 templates allowed (e.g. "{{ service_domain }}").
    pub value: String,
}

// ── Variable documentation ────────────────────────────────────────────────────

/// Documentation entry for a single container environment variable.
///
/// Stored under `[[variables]]` in the module TOML.
/// Shown in the Container Manager's Catalog editor so admins understand
/// what each env var does and can improve the description via LLM assist.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VariableDef {
    /// Environment variable name, e.g. `"DFLY_requirepass"`.
    pub name: String,

    /// Human-readable explanation shown in the Manager UI.
    pub description: String,

    /// `true` when the value is sensitive (password, token, API key).
    /// Masked in the UI; stored in vault rather than plain config.
    #[serde(default)]
    pub secret: bool,

    /// `true` when this variable must be set before the container starts.
    #[serde(default)]
    pub required: bool,

    /// Informational default value (the actual default lives in the template string).
    pub default: Option<String>,
}

// ── Setup wizard types ────────────────────────────────────────────────────────

/// All configuration fields this service requires during `fsn init`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceSetup {
    #[serde(default)]
    pub fields: Vec<SetupField>,
}

/// A single field the wizard will prompt for.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    #[default]
    String,
    Secret, // masked input, stored in vault
    Email,
    Ip,
    Select, // requires `options`
    Bool,
}

// ── Service Metadata ──────────────────────────────────────────────────────────

/// Core metadata declared under the `[module]` TOML key.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceMeta {
    pub name: String,

    #[serde(default)]
    pub alias: Vec<String>,

    /// Functional types – determines typed interfaces and project slots.
    ///
    /// Accepts either `type = "proxy"` (legacy single string) or
    /// `types = ["proxy", "webhoster_simple"]` (multi-type array).
    /// Both keys are accepted; `types` takes precedence if both are present.
    #[serde(
        rename = "types",
        alias = "type",
        default,
        deserialize_with = "de_service_types"
    )]
    #[schemars(with = "Vec<ServiceType>", rename = "types")]
    pub service_types: Vec<ServiceType>,

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

    /// Fine-grained capabilities this plugin provides beyond the type defaults.
    /// Example: `capabilities = ["iam_scim", "iam_ldap"]` in the plugin TOML.
    #[serde(default)]
    pub capabilities: Vec<Capability>,

    /// Service role declarations — which roles this module provides / requires.
    #[serde(default)]
    pub roles: ModuleRoles,

    /// UI integration hints — how the Desktop should open this service.
    #[serde(default)]
    pub ui: ModuleUi,
}

// ── ModuleRoles ───────────────────────────────────────────────────────────────

/// Service role declarations embedded in `[module.roles]`.
///
/// Roles are MIME-like identifiers for system functions (e.g. "proxy", "iam").
/// `provides` lists what this module can fulfil.
/// `requires` lists what must be assigned before this module will work.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ModuleRoles {
    /// Role IDs this module can fulfil (e.g. `["proxy", "webhoster"]`).
    #[serde(default)]
    pub provides: Vec<String>,

    /// Role IDs this module depends on being fulfilled by another service.
    #[serde(default)]
    pub requires: Vec<String>,
}

// ── ModuleUi ──────────────────────────────────────────────────────────────────

/// Desktop UI hints embedded in `[module.ui]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ModuleUi {
    /// Whether this service has a web UI that can be opened in the Desktop browser.
    #[serde(default)]
    pub supports_web: bool,

    /// How the Desktop opens this service: `"tab"` (default), `"window"`, `"embed"`.
    pub open_mode: Option<String>,

    /// Jinja2 template for the service URL (e.g. `"https://{{ service_domain }}"`).
    pub web_url_template: Option<String>,
}

impl ServiceMeta {
    /// Returns `true` if this service is purely internal infrastructure
    /// (no subdomain, no proxy route, no user-facing UI).
    /// Requires ALL declared types to be internal.
    pub fn is_internal_only(&self) -> bool {
        !self.service_types.is_empty() && self.service_types.iter().all(|t| t.is_internal())
    }

    /// Returns `true` if any of the declared types matches `t`.
    pub fn has_type(&self, t: &ServiceType) -> bool {
        self.service_types.contains(t)
    }

    /// The primary type (first in the list), or `Custom` if the list is empty.
    pub fn primary_type(&self) -> &ServiceType {
        self.service_types.first().unwrap_or(&ServiceType::Custom)
    }

    /// Comma-separated label list for TUI display (e.g. "Reverse Proxy, Webhoster (Simple)").
    pub fn types_label(&self) -> String {
        if self.service_types.is_empty() {
            return ServiceType::Custom.label().to_string();
        }
        self.service_types
            .iter()
            .map(|t| t.label())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Deployment constraints declared per service class.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Constraints {
    /// Maximum number of instances of this service class per host (null = unlimited).
    pub per_host: Option<u32>,

    /// Maximum number of instances of this service class per IP (null = unlimited).
    pub per_ip: Option<u32>,

    /// Locality constraint – if Some(SameHost), must run on same host as consumer.
    pub locality: Option<Locality>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Locality {
    SameHost,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FederationMeta {
    pub enabled: bool,
    pub min_trust: u8,
}

// ── Load / Dependencies ───────────────────────────────────────────────────────

/// Sub-service and service references declared under `[load]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceLoad {
    /// Sub-services this service owns and creates (e.g. postgres, dragonfly).
    /// TOML key: `modules` kept for file compatibility.
    #[serde(default, alias = "modules")]
    pub sub_services: IndexMap<String, SubServiceRef>,

    /// Peer services whose config this service reads (no ownership, no deployment).
    /// TOML: `peer_services = ["kanidm", "zentinel"]`
    #[serde(default)]
    pub peer_services: IndexSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubServiceRef {
    /// Class key, e.g. "database/postgres".
    /// TOML: `module_class` or `service_class` (both accepted).
    #[serde(alias = "module_class")]
    pub service_class: String,
}

// ── Deployment kind ───────────────────────────────────────────────────────────

/// How a resource is deployed on the host.
///
/// Derived from the `ServiceClass` fields, not stored separately.
/// Used by the deploy engine to decide between Quadlet and systemd unit generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentKind {
    /// Deployed as a Podman container via a Quadlet unit file.
    Container,
    /// Deployed as a native binary via a systemd .service (and optional .socket) unit.
    NativeApp,
}

impl ServiceClass {
    /// Deployment kind — derived from which deployment block is present.
    pub fn deployment_kind(&self) -> DeploymentKind {
        if self.service.is_some() {
            DeploymentKind::NativeApp
        } else {
            DeploymentKind::Container
        }
    }
}

// ── Native service definition ─────────────────────────────────────────────────

/// Native app deployment — maps to the `[service]` TOML block.
///
/// Used for Rust binaries (Zentinel, Stalwart, Kanidm, Tuwunel, mistral.rs, …)
/// that run directly under systemd instead of inside a Podman container.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NativeServiceDef {
    /// Absolute path to the binary (Jinja2 templates allowed).
    pub binary: String,

    /// Command-line arguments passed to the binary (Jinja2 templates allowed).
    #[serde(default)]
    pub args: Vec<String>,

    /// Run the service as this Unix user.
    pub user: Option<String>,

    /// Run the service as this Unix group.
    pub group: Option<String>,

    /// Ports to activate via systemd socket activation.
    /// Required for privileged ports (< 1024) without setcap.
    /// Generates a companion .socket unit alongside the .service unit.
    #[serde(default)]
    pub socket_ports: Vec<u16>,

    /// Health check (same structure as container healthcheck).
    pub healthcheck: Option<HealthCheck>,
}

// ── Container Definition ──────────────────────────────────────────────────────

/// Container definition – maps to the `[container]` TOML block.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

    /// Resource limits (ulimit key → value). Any TOML value accepted.
    #[serde(default)]
    #[schemars(schema_with = "schema_any_object")]
    pub ulimits: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HealthCheck {
    pub cmd: String,
    pub interval: String,
    pub timeout: String,
    pub retries: u32,
    pub start_period: String,
}

// ── Service Phase ─────────────────────────────────────────────────────────────

/// All phases a service passes through during its lifetime.
///
/// Used for status display, progress tracking and phase-gated hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServicePhase {
    /// Service record created, nothing deployed yet.
    Init,
    /// Container image is being pulled and Quadlets written.
    Install,
    /// Post-install configuration (secrets, initial data, peer registration).
    Configure,
    /// Container is starting up.
    Start,
    /// Waiting for health checks to pass.
    HealthCheck,
    /// Service is fully operational.
    Running,
    /// New image is being pulled and Quadlets are being updated.
    Update,
    /// Data backup in progress before a destructive operation.
    Backup,
    /// Schema / data migration running (e.g. database upgrade).
    Migrate,
    /// A replacement service is being installed; this service is still live.
    Swap,
    /// Service is being removed; data archival running.
    Decommission,
    /// Container is stopped; data retained on disk.
    Stop,
}

impl ServicePhase {
    /// Short display label shown in TUI and CLI output.
    pub fn label(self) -> &'static str {
        match self {
            Self::Init => "Init",
            Self::Install => "Install",
            Self::Configure => "Configure",
            Self::Start => "Start",
            Self::HealthCheck => "Health Check",
            Self::Running => "Running",
            Self::Update => "Update",
            Self::Backup => "Backup",
            Self::Migrate => "Migrate",
            Self::Swap => "Swap",
            Self::Decommission => "Decommission",
            Self::Stop => "Stop",
        }
    }

    /// One-sentence description of what happens during this phase.
    pub fn description(self) -> &'static str {
        match self {
            Self::Init => "Service record created; no containers deployed yet.",
            Self::Install => "Container image is being pulled and Quadlet files written.",
            Self::Configure => {
                "Post-install configuration: secrets, initial data, peer registration."
            }
            Self::Start => "Systemd unit is starting the container.",
            Self::HealthCheck => "Waiting for health checks to pass before marking Running.",
            Self::Running => "Service is fully operational.",
            Self::Update => "New image is being pulled; Quadlet files are being updated.",
            Self::Backup => "Data backup in progress before a destructive operation.",
            Self::Migrate => "Schema or data migration is running.",
            Self::Swap => "A replacement service is being installed; this one is still live.",
            Self::Decommission => "Service is being removed; data archival in progress.",
            Self::Stop => "Container stopped; data retained on disk.",
        }
    }
}

// ── Service Installed (Bus Event) ─────────────────────────────────────────────

/// Payload emitted on the service bus when a service finishes installation.
/// Consumed by peers that have `on_peer_install` hooks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInstalled {
    /// Primary type label of the installed service (e.g. `"wiki/outline"`).
    pub service_type: String,
    /// Unique instance identifier within the project.
    pub service_id: String,
}

// ── Service Lifecycle ─────────────────────────────────────────────────────────

/// Lifecycle hooks declared under `[lifecycle]` in a module TOML.
///
/// Phases run in order:  init → install → configure → start → running
///                       running → update → backup → migrate → swap → decommission
///
/// Each hook is idempotent by design — the engine may re-run it on reconcile.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceLifecycle {
    /// Hooks that fire after successful installation.
    #[serde(default)]
    pub on_install: Vec<LifecycleHook>,

    /// Hooks that fire during the configure phase (secrets, peer registration).
    #[serde(default)]
    pub on_configure: Vec<LifecycleHook>,

    /// Hooks that fire when another service is installed alongside this one.
    /// Each entry declares which peer type triggers it (`trigger = "wiki.*"`).
    #[serde(default)]
    pub on_peer_install: Vec<PeerHook>,

    /// Hooks that fire before and after an update (new image pull).
    #[serde(default)]
    pub on_update: Vec<LifecycleHook>,

    /// Hooks that fire during a migrate phase (schema or data migration).
    #[serde(default)]
    pub on_migrate: Vec<LifecycleHook>,

    /// Hooks that fire during a swap (this service is being replaced).
    #[serde(default)]
    pub on_swap: Vec<LifecycleHook>,

    /// Hooks that fire during decommission (graceful shutdown + data archival).
    #[serde(default)]
    pub on_decommission: Vec<LifecycleHook>,
}

/// A single lifecycle hook — the action type determines which fields are present.
///
/// Uses TOML's internally-tagged enum format: `action = "run"` selects the variant.
///
/// ```toml
/// [[lifecycle.on_install]]
/// action  = "run"
/// command = "kanidm-admin create-user admin"
///
/// [[lifecycle.on_install]]
/// action = "bus_emit"
/// event  = "service.installed"
///
/// [[lifecycle.on_update]]
/// action = "backup"
/// target = "/srv/backups/outline"   # optional
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum LifecycleHook {
    /// Run a shell command inside the container (via `podman exec`).
    /// Command is split on whitespace — use scripts for complex logic.
    Run { command: String },

    /// Emit a named event onto the FSN service bus.
    BusEmit {
        event: String,
        #[serde(default)]
        #[schemars(schema_with = "schema_any_object")]
        data: IndexMap<String, Value>,
    },

    /// Create a data backup before proceeding.
    /// `target` defaults to `{data_dir}-backup-{timestamp}`.
    Backup { target: Option<String> },

    /// Export service data to a portable format for consumption by another service.
    Export {
        target: Option<String>,
        /// Export format: `"json"` | `"toml"` | `"tar"`.
        format: Option<String>,
    },
}

/// A lifecycle hook that fires when a specific peer service type is installed.
///
/// Only shell commands are supported — peer hooks execute a command inside this
/// container when another service of the matching type is installed alongside it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PeerHook {
    /// Glob-style trigger pattern matching the peer's primary ServiceType label.
    /// Examples: `"wiki.*"`, `"git/forgejo"`, `"iam.*"`.
    pub trigger: String,

    /// Shell command to run when the trigger fires (via `podman exec`).
    pub command: String,

    /// Script arguments (positional).
    #[serde(default)]
    pub args: Vec<String>,
}

impl ServiceLifecycle {
    /// Returns `true` if no hooks are defined (all fields empty).
    pub fn is_empty(&self) -> bool {
        self.on_install.is_empty()
            && self.on_configure.is_empty()
            && self.on_peer_install.is_empty()
            && self.on_update.is_empty()
            && self.on_migrate.is_empty()
            && self.on_swap.is_empty()
            && self.on_decommission.is_empty()
    }

    /// Returns the peer hooks whose trigger matches the given service type label.
    /// `peer_type_label` is the primary type label of the newly installed peer.
    pub fn matching_peer_hooks(&self, peer_type_label: &str) -> Vec<&PeerHook> {
        self.on_peer_install
            .iter()
            .filter(|h| glob_matches(&h.trigger, peer_type_label))
            .collect()
    }
}

/// Minimal glob matcher: `*` matches anything within a single path segment,
/// `.*` at the end matches any sub-type.
fn glob_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" || pattern == value {
        return true;
    }
    // Pattern like "wiki.*" → prefix match on "wiki/"
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return value.starts_with(&format!("{prefix}/")) || value == prefix;
    }
    false
}

// ── Resource impl for ServiceClass ────────────────────────────────────────────

impl Resource for ServiceClass {
    fn kind(&self) -> &'static str {
        "service_class"
    }
    fn id(&self) -> &str {
        &self.meta.name
    }
    fn display_name(&self) -> &str {
        &self.meta.name
    }
    fn description(&self) -> Option<&str> {
        self.meta.description.as_deref()
    }
    fn tags(&self) -> &[String] {
        &self.meta.tags
    }

    fn validate(&self) -> Result<(), FsyError> {
        if self.meta.name.is_empty() {
            return Err(FsyError::Config("module.name is required".into()));
        }
        if self.meta.version.is_empty() {
            return Err(FsyError::Config("module.version is required".into()));
        }
        match (&self.container, &self.service) {
            (Some(c), _) => {
                if c.image.is_empty() {
                    return Err(FsyError::Config("container.image is required".into()));
                }
            }
            (None, Some(s)) => {
                if s.binary.is_empty() {
                    return Err(FsyError::Config("service.binary is required".into()));
                }
            }
            (None, None) => {
                return Err(FsyError::Config(
                    "either [container] or [service] block is required".into(),
                ));
            }
        }
        Ok(())
    }
}
