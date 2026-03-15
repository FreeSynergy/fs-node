//! fsn-wizard — Import tool for FreeSynergy.
//!
//! Import tool: converts Docker Compose / YAML container definitions into FSN module TOML.
//! This is an import-only tool — Docker Compose is not used at runtime. FSN uses Quadlets.
//!
//! # Pipeline
//!
//! ```text
//! ComposeInput (text/path/url)
//!     → parse()   → ComposeService
//!     → detect()  → ServiceType (proxy, mail, git, …)
//!     → generate() → ModuleToml
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use fsn_wizard::{Wizard, ComposeInput};
//!
//! let wizard = Wizard::new();
//! let result = wizard.convert(ComposeInput::text(yaml_str))?;
//! println!("{}", result.to_toml());
//! ```

pub mod capability_matcher;
pub mod compose;
pub mod detect;
pub mod discovery;
pub mod generate;
pub mod join;
pub mod setup_fields;
pub mod steps;
pub mod token;
pub mod wizard;

pub use capability_matcher::{CapabilityBinding, CapabilityMatcher};
pub use compose::{ComposeInput, ComposeService};
pub use detect::ServiceTypeHint;
pub use discovery::{DiscoveredNode, ManualDiscovery, MdnsDiscovery, NodeDiscovery};
pub use generate::ModuleToml;
pub use join::JoinToken;
pub use setup_fields::{SetupField, SetupFieldType};
pub use steps::WizardStep;
pub use token::{StoredToken, TokenFile};
pub use wizard::Wizard;
