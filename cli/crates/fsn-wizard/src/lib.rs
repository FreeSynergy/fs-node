//! fsn-wizard — Container assistant for FreeSynergy.
//!
//! Converts Docker Compose / YAML container definitions into FSN module TOML.
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

pub mod compose;
pub mod detect;
pub mod generate;
pub mod wizard;

pub use compose::{ComposeInput, ComposeService};
pub use detect::ServiceTypeHint;
pub use generate::ModuleToml;
pub use wizard::Wizard;
