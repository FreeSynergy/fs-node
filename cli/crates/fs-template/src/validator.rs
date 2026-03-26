// Template validator — checks that templates reference only declared variables.
//
// Design:
//   - `TemplateValidator` holds a set of known variable names.
//   - `validate_str` parses a template string and returns all referenced
//     variables that are NOT in the known set.
//   - `validate_required` checks that all required variables are present
//     in a `TemplateContext` before rendering.

use std::collections::HashSet;

use fs_error::FsError;

use crate::context::TemplateContext;

// ── TemplateValidator ─────────────────────────────────────────────────────────

/// Validates template strings against a declared set of variables.
///
/// # Example
/// ```rust,ignore
/// use fs_template::{TemplateContext, TemplateValidator};
///
/// let mut validator = TemplateValidator::new();
/// validator.declare(["name", "image"]);
///
/// // Returns unknown variables
/// let unknown = validator.validate_str("Image={{ image }}, Port={{ port }}").unwrap();
/// assert_eq!(unknown, vec!["port"]);
/// ```
#[derive(Debug, Default)]
pub struct TemplateValidator {
    known: HashSet<String>,
    required: HashSet<String>,
}

impl TemplateValidator {
    /// Create an empty validator (no declared variables).
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare variables that are valid in templates.
    pub fn declare<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) -> &mut Self {
        for name in names {
            self.known.insert(name.to_string());
        }
        self
    }

    /// Mark variables as required (must appear in context before rendering).
    pub fn require<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) -> &mut Self {
        for name in names {
            let n = name.to_string();
            self.known.insert(n.clone());
            self.required.insert(n);
        }
        self
    }

    /// Parse `template` and return any variable names that are referenced but
    /// not declared via [`declare`] or [`require`].
    ///
    /// Returns an empty `Vec` when all referenced variables are known.
    ///
    /// [`declare`]: TemplateValidator::declare
    /// [`require`]: TemplateValidator::require
    pub fn validate_str(&self, template: &str) -> Result<Vec<String>, FsError> {
        let vars = extract_variables(template);
        let unknown: Vec<String> = vars
            .into_iter()
            .filter(|v| !self.known.contains(v))
            .collect();
        Ok(unknown)
    }

    /// Return any required variables that are not present in `ctx`.
    ///
    /// Use this before calling `TemplateEngine::render_str` to catch missing
    /// inputs early with a descriptive error instead of a Tera render error.
    pub fn check_required(&self, ctx: &TemplateContext) -> Vec<String> {
        self.required
            .iter()
            .filter(|req| !ctx.contains_key(req))
            .cloned()
            .collect()
    }

    /// Combine `validate_str` and `check_required` into a single call.
    ///
    /// Returns `Err` if there are unknown variables in the template OR if
    /// required variables are missing from the context.
    pub fn validate_all(&self, template: &str, ctx: &TemplateContext) -> Result<(), FsError> {
        let unknown = self.validate_str(template)?;
        if !unknown.is_empty() {
            return Err(FsError::internal(format!(
                "template references unknown variables: {}",
                unknown.join(", ")
            )));
        }

        let missing = self.check_required(ctx);
        if !missing.is_empty() {
            return Err(FsError::internal(format!(
                "missing required template variables: {}",
                missing.join(", ")
            )));
        }

        Ok(())
    }
}

// ── Variable extraction ───────────────────────────────────────────────────────

/// Extract all `{{ variable }}` references from a Tera template string.
///
/// This is a best-effort scan — it does not parse full Tera expressions.
/// It finds bare `{{ name }}` and `{{ name | filter }}` patterns.
fn extract_variables(template: &str) -> Vec<String> {
    let mut vars: Vec<String> = Vec::new();
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("}}") else { break };
        let inner = rest[..end].trim();
        rest = &rest[end + 2..];

        // Take the identifier before any `|` (filter) or `.` (member access)
        let name = inner
            .split(['|', '.', ' ', '\n', '\t'])
            .next()
            .unwrap_or("")
            .trim();

        if !name.is_empty() && is_ident(name) && !vars.contains(&name.to_string()) {
            vars.push(name.to_string());
        }
    }

    vars
}

/// Return `true` when `s` is a valid identifier (letters, digits, `_`).
fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .map(|c| c.is_alphabetic() || c == '_')
            .unwrap_or(false)
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_simple_variable() {
        let vars = extract_variables("Hello {{ name }}!");
        assert_eq!(vars, vec!["name"]);
    }

    #[test]
    fn extract_multiple_variables() {
        let vars = extract_variables("{{ image }}:{{ tag }}");
        assert!(vars.contains(&"image".to_string()));
        assert!(vars.contains(&"tag".to_string()));
    }

    #[test]
    fn extract_with_filter() {
        let vars = extract_variables("{{ name | to_env_key }}");
        assert_eq!(vars, vec!["name"]);
    }

    #[test]
    fn no_duplicate_variables() {
        let vars = extract_variables("{{ name }} and {{ name }}");
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn validate_str_finds_unknown() {
        let mut v = TemplateValidator::new();
        v.declare(["name", "image"]);
        let unknown = v.validate_str("{{ name }} {{ port }}").unwrap();
        assert_eq!(unknown, vec!["port"]);
    }

    #[test]
    fn validate_str_all_known() {
        let mut v = TemplateValidator::new();
        v.declare(["name", "image"]);
        let unknown = v.validate_str("{{ name }}: {{ image }}").unwrap();
        assert!(unknown.is_empty());
    }

    #[test]
    fn check_required_missing() {
        let mut v = TemplateValidator::new();
        v.require(["name", "image"]);

        let mut ctx = TemplateContext::new();
        ctx.set_str("name", "zentinel");
        // "image" is missing

        let missing = v.check_required(&ctx);
        assert_eq!(missing, vec!["image"]);
    }

    #[test]
    fn check_required_all_present() {
        let mut v = TemplateValidator::new();
        v.require(["name"]);

        let mut ctx = TemplateContext::new();
        ctx.set_str("name", "zentinel");

        let missing = v.check_required(&ctx);
        assert!(missing.is_empty());
    }

    #[test]
    fn validate_all_success() {
        let mut v = TemplateValidator::new();
        v.require(["name"]);

        let mut ctx = TemplateContext::new();
        ctx.set_str("name", "zentinel");

        assert!(v.validate_all("{{ name }}", &ctx).is_ok());
    }

    #[test]
    fn validate_all_unknown_var() {
        let mut v = TemplateValidator::new();
        v.require(["name"]);

        let mut ctx = TemplateContext::new();
        ctx.set_str("name", "zentinel");

        let err = v
            .validate_all("{{ name }} {{ unknown }}", &ctx)
            .unwrap_err();
        assert!(err.to_string().contains("unknown"));
    }
}
