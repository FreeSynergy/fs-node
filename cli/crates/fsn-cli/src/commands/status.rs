use std::path::Path;
use anyhow::Result;
use fsn_container::{SystemdManager, UnitActiveState};

/// Print the systemd state of all FSN-managed services.
pub async fn run(_root: &Path, _project: Option<&Path>) -> Result<()> {
    let systemd = SystemdManager::new();
    let units = fsn_deploy::observe::list_fsn_units(&systemd).await?;

    if units.is_empty() {
        println!("No FSN-managed services found.");
        return Ok(());
    }

    println!("{:<30} {}", "SERVICE", "STATE");
    println!("{}", "─".repeat(42));

    for unit in &units {
        let name = unit.trim_end_matches(".service");
        let state = match systemd.status(unit).await {
            Ok(s) => match s.active_state {
                UnitActiveState::Active       => "active",
                UnitActiveState::Inactive     => "inactive",
                UnitActiveState::Activating   => "activating",
                UnitActiveState::Deactivating => "deactivating",
                UnitActiveState::Failed       => "FAILED",
                UnitActiveState::Unknown      => "unknown",
            },
            Err(_) => "error",
        };
        println!("{:<30} {}", name, state);
    }

    Ok(())
}
