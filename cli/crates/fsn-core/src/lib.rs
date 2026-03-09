// fsn-core – FreeSynergy.Node core data types and config parsing.
//
// This crate has NO async dependencies and NO binary I/O.
// It is the foundation every other FSN crate depends on.

pub mod config;
pub mod resource;
pub mod state;
pub mod error;
pub mod store;

pub use config::bot::{BotConfig, BotMeta, BotType};
pub use error::FsnError;
pub use resource::{
    Resource, ResourcePhase,
    ProjectResource, HostResource, ServiceResource, BotResource,
    VarProvider,
};
