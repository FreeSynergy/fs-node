// fs-core – FreeSynergy.Node core data types and config parsing.
//
// This crate has NO async dependencies and NO binary I/O.
// It is the foundation every other FSN crate depends on.

pub mod audit;
pub mod config;
pub mod error;
pub mod health;
pub mod resource;
pub mod state;
pub mod store;

pub use audit::{AuditEntry, AuditLog};
pub use config::bot::{BotConfig, BotMeta, BotType};
pub use error::{FsError, FsyError};
pub use resource::{
    BotResource, HostResource, ProjectResource, Resource, ResourcePhase, ServiceResource,
    VarProvider,
};
