// Zentinel KDL block generation.
// Manages FSN-owned blocks in the Zentinel proxy config.
// Each block is wrapped in FSN-MANAGED-START/END markers so the deployer
// can update its own blocks without touching manually-edited config.

use anyhow::Result;
use fsn_core::state::desired::ServiceInstance;

const MARKER_START: &str = "# === FSN-MANAGED-START:";
const MARKER_END: &str = "# === FSN-MANAGED-END:";

/// Generate the KDL block for a module instance's proxy route.
pub fn generate_block(instance: &ServiceInstance) -> String {
    let name = &instance.name;
    let port = instance.class.meta.port;
    let mut domains = vec![instance.service_domain.clone()];
    domains.extend(instance.alias_domains.clone());

    let mut out = String::new();
    out.push_str(&format!("{} {} ===\n", MARKER_START, name));

    for domain in &domains {
        out.push_str(&format!(
            "{} {{\n    reverse_proxy {}:{}\n}}\n",
            domain, name, port
        ));
    }

    out.push_str(&format!("{} {} ===\n", MARKER_END, name));
    out
}

/// Replace or insert an FSN-managed block in the full Zentinel config.
pub fn upsert_block(config: &str, instance: &ServiceInstance) -> String {
    let block = generate_block(instance);
    let start_marker = format!("{} {} ===", MARKER_START, instance.name);
    let end_marker = format!("{} {} ===", MARKER_END, instance.name);

    // Remove old block if present
    let cleaned = remove_block(config, &instance.name);
    // Append new block
    format!("{}\n{}", cleaned.trim_end(), block)
}

/// Remove an FSN-managed block for the given service name.
pub fn remove_block(config: &str, service_name: &str) -> String {
    let start_marker = format!("{} {} ===", MARKER_START, service_name);
    let end_marker = format!("{} {} ===", MARKER_END, service_name);

    let mut out = String::new();
    let mut in_block = false;

    for line in config.lines() {
        if line.trim_start().starts_with(&start_marker) {
            in_block = true;
            continue;
        }
        if line.trim_start().starts_with(&end_marker) {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }

    out
}
