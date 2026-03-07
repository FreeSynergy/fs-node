// Constraint enforcement – validates per_host, per_ip, locality rules.
// Replaces playbooks/tasks/check-constraints.yml

use anyhow::{bail, Result};
use fsn_core::{
    config::module::Locality,
    state::DesiredState,
};

/// Check all deployment constraints for the resolved desired state.
/// Returns Err if any constraint is violated.
pub fn check(desired: &DesiredState) -> Result<()> {
    check_per_host(desired)?;
    // per_ip requires multi-host context (Phase 4)
    // locality is checked during deploy (sub-module must be on same host)
    Ok(())
}

/// per_host: at most N instances of the same module class per host.
fn check_per_host(desired: &DesiredState) -> Result<()> {
    // Collect all instances (including sub-modules) with their class key
    let all = collect_all_instances(desired);

    // Group by class key
    let mut counts: std::collections::HashMap<&str, Vec<&str>> = Default::default();
    for (class_key, name, limit) in &all {
        if let Some(limit) = limit {
            counts
                .entry(class_key)
                .or_default()
                .push(name);

            let group = &counts[class_key];
            if group.len() > *limit as usize {
                bail!(
                    "Constraint violation: module class '{}' has per_host={}, \
                     but {} instance(s) found: {}",
                    class_key,
                    limit,
                    group.len(),
                    group.join(", ")
                );
            }
        }
    }
    Ok(())
}

/// Returns (class_key, instance_name, per_host_limit) for every instance.
fn collect_all_instances(
    desired: &DesiredState,
) -> Vec<(String, String, Option<u32>)> {
    let mut out = Vec::new();
    for m in &desired.modules {
        push_instance(m, &mut out);
    }
    out
}

fn push_instance(
    instance: &fsn_core::state::desired::ModuleInstance,
    out: &mut Vec<(String, String, Option<u32>)>,
) {
    out.push((
        instance.class_key.clone(),
        instance.name.clone(),
        instance.class.module.constraints.per_host,
    ));
    for sub in &instance.sub_modules {
        push_instance(sub, out);
    }
}
