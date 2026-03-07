// State diff – compare desired vs actual to determine what needs to change.

use fsn_core::state::{ActualState, DesiredState, RunState, StateDiff};

/// Compare desired state with actual state and return what needs to change.
pub fn compute_diff(desired: &DesiredState, actual: &ActualState) -> StateDiff {
    let mut diff = StateDiff::default();

    // Check each desired module against actual state
    for instance in &desired.modules {
        check_instance(instance, actual, &mut diff);
    }

    // Find services running that are NOT in desired state → remove them
    for service in &actual.services {
        let still_desired = desired
            .modules
            .iter()
            .any(|m| m.name == service.name || m.sub_modules.iter().any(|s| s.name == service.name));

        if !still_desired && service.state == RunState::Running {
            diff.to_remove.push(service.name.clone());
        }
    }

    diff
}

fn check_instance(
    instance: &fsn_core::state::desired::ModuleInstance,
    actual: &ActualState,
    diff: &mut StateDiff,
) {
    match actual.find(&instance.name) {
        None => {
            diff.to_deploy.push(instance.clone());
        }
        Some(status) => {
            if status.state == RunState::Missing {
                diff.to_deploy.push(instance.clone());
            } else if status.deployed_version != instance.version {
                diff.to_update.push(instance.clone());
            } else {
                diff.ok.push(instance.name.clone());
            }
        }
    }

    // Recurse into sub-modules
    for sub in &instance.sub_modules {
        check_instance(sub, actual, diff);
    }
}
