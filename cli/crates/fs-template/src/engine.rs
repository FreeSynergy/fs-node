// fs-template — TemplateEngine

use fs_error::FsError;

use crate::context::TemplateContext;
use crate::filters::{DomainLabel, Indent, ToEnvKey, ToSlug};

// ── TemplateEngine ────────────────────────────────────────────────────────────

/// Tera-based template rendering engine.
///
/// Supports rendering from strings and from directories of `.tera` / `.j2` template files.
pub struct TemplateEngine {
    tera: tera::Tera,
}

impl TemplateEngine {
    /// Create an engine for ad-hoc string rendering only.
    pub fn new() -> Self {
        let mut tera = tera::Tera::default();
        Self::register_filters(&mut tera);
        Self { tera }
    }

    /// Create an engine that loads all templates from `dir`.
    ///
    /// Globbing pattern: `{dir}/**/*.tera` and `{dir}/**/*.j2`.
    ///
    /// Returns an error if the directory doesn't exist or templates fail to parse.
    /// If no templates are found, an empty (string-rendering-only) engine is returned.
    pub fn from_dir(dir: impl AsRef<std::path::Path>) -> Result<Self, FsError> {
        let dir = dir.as_ref();

        if !dir.exists() {
            return Err(FsError::not_found(format!(
                "template directory not found: {}",
                dir.display()
            )));
        }

        let tera_glob = format!("{}/**/*.tera", dir.display());
        let j2_glob = format!("{}/**/*.j2", dir.display());

        // Try loading .tera files first
        let mut tera = match tera::Tera::new(&tera_glob) {
            Ok(t) => t,
            Err(e) => {
                // tera::Tera::new errors when glob matches zero files or on parse failure.
                // Check if it is a "no templates" situation vs real parse error.
                let msg = e.to_string();
                if msg.contains("No match") || msg.contains("no match") || msg.contains("glob") {
                    tera::Tera::default()
                } else {
                    return Err(FsError::internal(format!("template error: {e}")));
                }
            }
        };

        // Extend with .j2 files
        if let Err(e) = tera.add_template_files(
            glob::glob(&j2_glob)
                .map_err(|e| FsError::internal(format!("glob error: {e}")))?
                .filter_map(|p| p.ok())
                .map(|p| (p, None::<String>))
                .collect::<Vec<_>>(),
        ) {
            let msg = e.to_string();
            if !msg.contains("No match") && !msg.contains("no match") {
                return Err(FsError::internal(format!("template error: {e}")));
            }
        }

        Self::register_filters(&mut tera);
        Ok(Self { tera })
    }

    /// Add or replace a named template from a string.
    pub fn add_template(
        &mut self,
        name: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<(), FsError> {
        let name = name.into();
        let source = source.into();
        self.tera
            .add_raw_template(&name, &source)
            .map_err(|e| FsError::internal(format!("template error: {e}")))
    }

    /// Render a template string directly (not registered by name).
    pub fn render_str(&self, template: &str, ctx: &TemplateContext) -> Result<String, FsError> {
        // tera::Tera::one_off is a free function for one-shot rendering
        tera::Tera::one_off(template, &ctx.to_tera(), false)
            .map_err(|e| FsError::internal(format!("template error: {e}")))
    }

    /// Render a named template (must have been loaded via [`from_dir`] or [`add_template`]).
    ///
    /// [`from_dir`]: TemplateEngine::from_dir
    /// [`add_template`]: TemplateEngine::add_template
    pub fn render(&self, name: &str, ctx: &TemplateContext) -> Result<String, FsError> {
        self.tera
            .render(name, &ctx.to_tera())
            .map_err(|e| FsError::internal(format!("template error: {e}")))
    }

    /// List all registered template names.
    pub fn template_names(&self) -> Vec<&str> {
        self.tera.get_template_names().collect()
    }

    // ── private ───────────────────────────────────────────────────────────────

    fn register_filters(tera: &mut tera::Tera) {
        tera.register_filter("to_env_key", ToEnvKey);
        tera.register_filter("to_slug", ToSlug);
        tera.register_filter("domain_label", DomainLabel);
        tera.register_filter("indent", Indent);
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}
