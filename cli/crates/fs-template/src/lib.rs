//! Tera template rendering for FreeSynergy.
//!
//! Generates Quadlet files, config files, and other text artifacts from templates.
//!
//! # Quick start
//! ```rust,ignore
//! use fs_template::{TemplateEngine, TemplateContext};
//!
//! let engine = TemplateEngine::new();
//! let mut ctx = TemplateContext::new();
//! ctx.set_str("name", "zentinel");
//! let out = engine.render_str("service: {{ name }}", &ctx).unwrap();
//! ```
mod context;
mod engine;
mod filters;
mod validator;

pub use context::TemplateContext;
pub use engine::TemplateEngine;
pub use validator::TemplateValidator;
