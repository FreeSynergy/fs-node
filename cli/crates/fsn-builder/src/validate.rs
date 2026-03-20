//! `fsn-builder validate` — validate a resource package directory.
//!
//! Expects the package directory to contain a `resource.toml` file.
//! Loads the resource, runs the `Validate` trait, and prints the status.

use anyhow::{bail, Context, Result};
use fsn_types::resources::{
    container::ContainerResource,
    meta::ValidationStatus,
    validator::Validate,
};
use std::path::Path;

pub fn run(path: &Path) -> Result<()> {
    let toml_path = path.join("resource.toml");
    if !toml_path.exists() {
        bail!("No resource.toml found in {}", path.display());
    }

    let raw = std::fs::read_to_string(&toml_path)
        .with_context(|| format!("Cannot read {}", toml_path.display()))?;

    // Try to determine resource type from the TOML.
    let value: toml::Value = toml::from_str(&raw)
        .with_context(|| "resource.toml is not valid TOML")?;

    let resource_type = value
        .get("meta")
        .and_then(|m| m.get("resource_type"))
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");

    match resource_type {
        "container" => {
            let mut resource: ContainerResource = toml::from_str(&raw)
                .with_context(|| "Failed to parse resource.toml as ContainerResource")?;
            resource.validate();
            print_status(&resource.meta.id, &resource.meta.status);
        }
        other => {
            bail!("Unsupported resource type '{other}'. Only container is supported by this validator.");
        }
    }

    Ok(())
}

fn print_status(id: &str, status: &ValidationStatus) {
    let (badge, message) = match status {
        ValidationStatus::Ok         => ("✅", "Resource is valid."),
        ValidationStatus::Incomplete => ("⚠️ ", "Resource is incomplete — some required fields are missing."),
        ValidationStatus::Broken     => ("❌", "Resource is broken — critical fields are missing or invalid."),
    };
    println!("{badge} {id}: {message}");
}
