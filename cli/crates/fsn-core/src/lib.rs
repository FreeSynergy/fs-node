// fsn-core – FreeSynergy.Node core data types and config parsing.
//
// This crate has NO async dependencies and NO binary I/O.
// It is the foundation every other FSN crate depends on.

pub mod config;
pub mod health;
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

// ── Form vocabulary (ported from fsn-lib/fsn-core) ───────────────────────────

/// What a form node returns after handling a keyboard or mouse event.
#[derive(Debug, Clone, PartialEq)]
pub enum FormAction {
    Consumed, ValueChanged, FocusNext, FocusPrev, AcceptAndNext,
    TabNext, TabPrev, Submit, Cancel, LangToggle, Quit, Unhandled,
}

/// What a selection popup returns after handling a key or mouse event.
#[derive(Debug, PartialEq)]
pub enum SelectionResult {
    Consumed,
    Accepted(String),
    AcceptedMulti(Vec<String>),
    Rejected,
    Unhandled,
}
