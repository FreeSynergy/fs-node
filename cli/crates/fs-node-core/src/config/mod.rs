pub mod bot;
pub mod discovery;
pub mod host;
pub mod manifest;
pub mod meta;
pub mod plugin;
pub mod project;
pub mod registry;
pub mod service;
pub mod settings;
pub mod validate;
pub mod vault;

pub use bot::{BotConfig, BotMeta, BotType};
pub use discovery::{find_host, find_host_by_name, find_project};
pub use host::{HostAcme, HostConfig, HostDns, HostMeta};
pub use manifest::{
    InstanceInfo, LogLevel, LogLine, ManifestInputs, ManifestOutputFile, ModuleManifest,
    OutputFile, PeerRoute, PeerService, PluginContext, PluginResponse, ShellCommand,
};
pub use meta::ResourceMeta;
pub use plugin::{PluginConfig, PluginMeta};
pub use project::{
    ModuleRef, // backwards-compat alias
    ProjectConfig,
    ProjectLoad,
    ProjectMeta,
    ServiceEntry,
    ServiceInstanceConfig,
    ServiceInstanceMeta,
    ServiceSlots,
};
pub use registry::ServiceRegistry;
pub use service::{
    Capability, Constraints, ContainerDef, DeploymentKind, ExportedVarContract, FieldType,
    HeaderSpec, LifecycleHook, Locality, ModuleRoles, ModuleUi, NativeServiceDef, PeerHook,
    RouteSpec, ServiceClass, ServiceContract, ServiceLifecycle, ServiceLoad, ServiceMeta,
    ServiceSetup, ServiceType, SetupField, SubServiceRef,
};
pub use settings::{
    resolve_plugins_dir, resolve_plugins_dir_no_fallback, AppSettings, ServiceRoleMap,
    ServiceRoleRegistry, StoreConfig,
};
pub use vault::VaultConfig;

// ── Shared TOML loader ────────────────────────────────────────────────────────

/// Load and deserialize any TOML config file into `T`.
///
/// Single source of truth for the read-and-parse pattern used by all config
/// types (`ProjectConfig`, `HostConfig`, `ServiceInstanceConfig`, …).
/// Returns typed `FsyError` variants so callers do not need to map manually.
pub fn load_toml<T>(path: &std::path::Path) -> Result<T, fs_error::FsyError>
where
    T: serde::de::DeserializeOwned,
{
    load_toml_validated(path, validate::TomlKind::Generic)
}

/// Load and deserialize a TOML config file with schema + safety validation.
///
/// Chain of Responsibility:
///   1. Read file
///   2. validate::validate_toml_content (size → syntax → safety → schema)
///   3. Deserialize into `T`
pub fn load_toml_validated<T>(
    path: &std::path::Path,
    kind: validate::TomlKind,
) -> Result<T, fs_error::FsyError>
where
    T: serde::de::DeserializeOwned,
{
    let path_str = path.display().to_string();
    let content = std::fs::read_to_string(path)
        .map_err(|_| fs_error::FsyError::NotFound(path_str.clone()))?;
    validate::validate_toml_content(&content, kind, &path_str)?;
    toml::from_str(&content).map_err(|e| fs_error::FsyError::Parse(format!("{path_str}: {e}")))
}
