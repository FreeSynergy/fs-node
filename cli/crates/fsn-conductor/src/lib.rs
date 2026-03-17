//! FreeSynergy Conductor — compose YAML → Podman Quadlet pipeline.
//!
//! # Pipeline
//!
//! 1. **Parse** — [`compose::parse_file`] reads a docker-compose / Podman-compose YAML
//! 2. **Analyze** — [`analysis::analyze_vars`] infers type, role and confidence for env vars
//! 3. **Validate** — [`validation::validate`] checks for errors and warns about missing healthchecks
//! 4. **Convert** — [`converter::convert`] maps compose services to [`fsn_container::ServiceConfig`]
//! 5. **Install** — [`fsn_container::QuadletManager`] writes `.container` unit files + daemon-reload
//!
//! Store enrichment via [`store_client::enrich`] is optional and non-blocking.

pub mod analysis;
pub mod compose;
pub mod converter;
pub mod instance;
pub mod pipeline;
pub mod store_client;
pub mod validation;

// Re-export the most commonly used types at crate root
pub use analysis::{analyze_var, analyze_vars, AnalyzedVar, VarRole, VarType};
pub use compose::{parse_file, parse_str, ComposeFile, ComposeService, EnvVar};
pub use instance::InstanceName;
pub use pipeline::{analyze, install, AnalyzeResult};
pub use validation::{validate, IssueLevel, ValidationIssue, ValidationReport};
