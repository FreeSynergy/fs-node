// fs-template — Custom Tera filters

use std::collections::HashMap;

use tera::{Filter, Value};

// ── ToEnvKey ──────────────────────────────────────────────────────────────────

/// Convert a string to UPPER_SNAKE_CASE for use as an env var key.
///
/// Non-alphanumeric characters are replaced with `_` and the result is uppercased.
///
/// # Examples
/// - `"my-service"` → `"MY_SERVICE"`
/// - `"my.domain.com"` → `"MY_DOMAIN_COM"`
pub struct ToEnvKey;

impl Filter for ToEnvKey {
    fn filter(&self, value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        let s = value
            .as_str()
            .ok_or_else(|| tera::Error::msg("to_env_key: expected a string value"))?;

        let result: String = s
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .to_uppercase();

        Ok(Value::String(result))
    }
}

// ── ToSlug ────────────────────────────────────────────────────────────────────

/// Convert a string to a kebab-case slug.
///
/// Whitespace and non-alphanumeric characters are replaced with `-`,
/// consecutive dashes are collapsed, and the result is lowercased.
///
/// # Examples
/// - `"My Service"` → `"my-service"`
pub struct ToSlug;

impl Filter for ToSlug {
    fn filter(&self, value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        let s = value
            .as_str()
            .ok_or_else(|| tera::Error::msg("to_slug: expected a string value"))?;

        let lowered = s.to_lowercase();
        let mut slug = String::with_capacity(lowered.len());
        let mut prev_dash = false;

        for c in lowered.chars() {
            if c.is_alphanumeric() {
                slug.push(c);
                prev_dash = false;
            } else if !prev_dash {
                slug.push('-');
                prev_dash = true;
            }
        }

        // Trim leading/trailing dashes
        let slug = slug.trim_matches('-').to_string();
        Ok(Value::String(slug))
    }
}

// ── DomainLabel ───────────────────────────────────────────────────────────────

/// Return the first label (leftmost segment) of a domain name.
///
/// # Examples
/// - `"forgejo.example.com"` → `"forgejo"`
pub struct DomainLabel;

impl Filter for DomainLabel {
    fn filter(&self, value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        let s = value
            .as_str()
            .ok_or_else(|| tera::Error::msg("domain_label: expected a string value"))?;

        let label = s.split('.').next().unwrap_or(s);
        Ok(Value::String(label.to_string()))
    }
}

// ── Indent ────────────────────────────────────────────────────────────────────

/// Indent all lines of a string by N spaces (default 2).
///
/// Useful for generating YAML or KDL with nested blocks.
///
/// # Arguments
/// - `width` (optional, integer) — number of spaces per indent level, default `2`
pub struct Indent;

impl Filter for Indent {
    fn filter(&self, value: &Value, args: &HashMap<String, Value>) -> tera::Result<Value> {
        let s = value
            .as_str()
            .ok_or_else(|| tera::Error::msg("indent: expected a string value"))?;

        let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(2) as usize;

        let pad = " ".repeat(width);
        let result = s
            .lines()
            .map(|line| format!("{pad}{line}"))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(Value::String(result))
    }
}
