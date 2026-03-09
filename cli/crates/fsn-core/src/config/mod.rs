pub mod bot;
pub mod host;
pub mod plugin;
pub mod project;
pub mod registry;
pub mod service;
pub mod settings;
pub mod vault;

pub use bot::{BotConfig, BotMeta, BotType};
pub use host::{HostConfig, HostDns, HostAcme, HostMeta};
pub use service::{
    Constraints, ContainerDef, Locality,
    ServiceClass, ServiceMeta, ServiceType,
    ServiceLoad, ServiceSetup, SetupField, FieldType,
    SubServiceRef, ServiceRef,
    ServiceContract, RouteSpec, HeaderSpec,
};
pub use project::{
    ModuleRef,       // backwards-compat alias
    ProjectConfig, ProjectLoad, ProjectMeta,
    ServiceEntry, ServiceSlots,
    ServiceInstanceConfig, ServiceInstanceMeta,
};
pub use plugin::{PluginConfig, PluginMeta};
pub use registry::ServiceRegistry;
pub use settings::{AppSettings, StoreConfig};
pub use vault::VaultConfig;
