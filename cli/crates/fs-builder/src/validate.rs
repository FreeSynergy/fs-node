//! `fs-builder validate` — validate a resource package directory.
//!
//! Expects the package directory to contain a `resource.toml` file.
//! Loads the resource, runs the `Validate` trait, and prints the status.
//!
//! # OOP design
//!
//! Each resource type implements `ResourceValidator` locally, moving parse-and-
//! validate logic onto the type.  New types register a single entry in
//! `RESOURCE_VALIDATORS` — no external `match resource_type` needed.

use anyhow::{Context, Result};
use fs_types::resources::{
    container::ContainerResource, meta::ValidationStatus, validator::Validate,
};
use std::path::Path;

// ── ResourceValidator ─────────────────────────────────────────────────────────

/// Extension trait that makes each resource type responsible for parsing and
/// validating itself from raw TOML.
///
/// Replaces the external `match resource_type` block with a static registry
/// following the *Strategy* pattern.
trait ResourceValidator {
    fn parse_and_validate(raw: &str) -> anyhow::Result<(String, ValidationStatus)>;
}

impl ResourceValidator for ContainerResource {
    fn parse_and_validate(raw: &str) -> anyhow::Result<(String, ValidationStatus)> {
        let mut resource: ContainerResource = toml::from_str(raw)
            .with_context(|| "Failed to parse resource.toml as ContainerResource")?;
        resource.validate();
        Ok((resource.meta.id.clone(), resource.meta.status))
    }
}

// ── Registry ──────────────────────────────────────────────────────────────────

type ValidateFn = fn(&str) -> anyhow::Result<(String, ValidationStatus)>;

/// Static mapping from `resource_type` string to validator.
/// Add a new entry here to support an additional resource type.
static RESOURCE_VALIDATORS: &[(&str, ValidateFn)] =
    &[("container", ContainerResource::parse_and_validate)];

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(path: &Path) -> Result<()> {
    let toml_path = path.join("resource.toml");
    if !toml_path.exists() {
        anyhow::bail!("No resource.toml found in {}", path.display());
    }

    let raw = std::fs::read_to_string(&toml_path)
        .with_context(|| format!("Cannot read {}", toml_path.display()))?;

    let value: toml::Value =
        toml::from_str(&raw).with_context(|| "resource.toml is not valid TOML")?;

    let resource_type = value
        .get("meta")
        .and_then(|m| m.get("resource_type"))
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");

    let validator = RESOURCE_VALIDATORS
        .iter()
        .find(|(t, _)| *t == resource_type)
        .map(|(_, f)| *f)
        .ok_or_else(|| {
            let supported = RESOURCE_VALIDATORS
                .iter()
                .map(|(t, _)| *t)
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::anyhow!(
                "Unsupported resource type '{}'. Supported: {}",
                resource_type,
                supported
            )
        })?;

    let (id, status) = validator(&raw)?;
    print_status(&id, &status);
    Ok(())
}

fn print_status(id: &str, status: &ValidationStatus) {
    let (badge, message) = match status {
        ValidationStatus::Ok => ("✅", "Resource is valid."),
        ValidationStatus::Incomplete => (
            "⚠️ ",
            "Resource is incomplete — some required fields are missing.",
        ),
        ValidationStatus::Broken => (
            "❌",
            "Resource is broken — critical fields are missing or invalid.",
        ),
    };
    println!("{badge} {id}: {message}");
}
