pub mod host;
pub mod service;
pub mod project;
pub mod registry;
pub mod vault;

pub use host::HostConfig;
pub use service::{
    Constraints, ContainerDef, Locality,
    ServiceClass, ServiceMeta, ServiceType,
    ServiceLoad, ServiceSetup, SetupField, FieldType,
    SubServiceRef, ServiceRef,
};
pub use project::{
    ModuleRef,       // backwards-compat alias
    ProjectConfig, ProjectLoad, ProjectMeta,
    ServiceEntry, ServiceSlots,
};
pub use registry::ServiceRegistry;
pub use vault::VaultConfig;
