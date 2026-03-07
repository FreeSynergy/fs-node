pub mod host;
pub mod module;
pub mod project;
pub mod registry;
pub mod vault;

pub use host::HostConfig;
pub use module::{Constraints, ContainerDef, Locality, ModuleClass, ModuleMeta};
pub use project::{ModuleRef, ProjectConfig};
pub use registry::ModuleRegistry;
pub use vault::VaultConfig;
