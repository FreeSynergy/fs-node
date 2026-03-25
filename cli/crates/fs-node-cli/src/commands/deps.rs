// `fsn deps <service>` — display the dependency graph for a service.
//
// Loads the project config, resolves which services are declared, and
// prints a tree showing runtime dependencies (iam, mail, wiki, etc.)
// based on the project's ServiceSlots.

use anyhow::{Context, Result};
use std::path::Path;

use fs_node_core::config::{find_project, ProjectConfig, ServiceType};

// ── run ───────────────────────────────────────────────────────────────────────

/// Show the dependency graph for `service`.
///
/// Resolves service slot assignments from the project config and renders them
/// as an ASCII tree to stdout.
pub async fn run(root: &Path, project: Option<&Path>, service: &str) -> Result<()> {
    let proj_path = project
        .map(|p| p.to_path_buf())
        .or_else(|| find_project(root, None))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No project found in '{}'. Use --project to specify one.",
                root.display()
            )
        })?;

    let proj = ProjectConfig::load(&proj_path)
        .with_context(|| format!("loading project config: {}", proj_path.display()))?;

    // Check the service exists in the project.
    if !proj.load.services.contains_key(service) {
        let available: Vec<&str> = proj.load.services.keys().map(String::as_str).collect();
        if available.is_empty() {
            anyhow::bail!(
                "Service '{}' not found. No services are declared in the project.",
                service
            );
        } else {
            anyhow::bail!(
                "Service '{}' not found. Available services: {}",
                service,
                available.join(", ")
            );
        }
    }

    let entry = &proj.load.services[service];

    println!("Dependency graph for: {}", service);
    println!("  Service class: {}", entry.service_class);
    println!();

    // ── Slot dependencies ─────────────────────────────────────────────────────
    // Show which project-level service slots this service might consume.
    let slots = &proj.services;
    let deps = collect_slot_deps(entry.service_class.as_str(), slots);

    if deps.is_empty() {
        println!("{} (no declared slot dependencies)", service);
    } else {
        println!("{}", service);
        let last = deps.len().saturating_sub(1);
        for (i, (slot, instance)) in deps.iter().enumerate() {
            let prefix = if i == last {
                "└── "
            } else {
                "├── "
            };
            println!("{}{}  [{}]", prefix, instance, slot);

            // Second-level: if that dependency itself has further slot deps,
            // show them (one level deep is enough for a useful display).
            if let Some(dep_entry) = proj.load.services.get(*instance) {
                let sub_deps = collect_slot_deps(dep_entry.service_class.as_str(), slots);
                let sub_last = sub_deps.len().saturating_sub(1);
                let indent = if i == last { "    " } else { "│   " };
                for (j, (sub_slot, sub_inst)) in sub_deps.iter().enumerate() {
                    let sub_prefix = if j == sub_last {
                        "└── "
                    } else {
                        "├── "
                    };
                    println!("{}{}{}  [{}]", indent, sub_prefix, sub_inst, sub_slot);
                }
            }
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return a list of `(slot_name, instance_name)` pairs that the given service
/// class depends on, using `ServiceType::consumed_slots` and `ServiceSlots::find`.
///
/// OOP: ServiceType knows what it needs; ServiceSlots knows what is assigned.
/// No heuristics here — both objects carry their own knowledge.
fn collect_slot_deps<'a>(
    class: &str,
    slots: &'a fs_node_core::config::ServiceSlots,
) -> Vec<(&'static str, &'a str)> {
    let prefix = class.split('/').next().unwrap_or("");
    let svc_type = ServiceType::from_class_prefix(prefix).unwrap_or(ServiceType::Custom);

    svc_type
        .consumed_slots()
        .iter()
        .filter_map(|slot| slots.find(slot).map(|inst| (*slot, inst)))
        .collect()
}
