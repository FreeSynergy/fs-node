use std::path::Path;
use anyhow::Result;
use fsn_podman::systemd::{self, UnitStatus};

pub async fn run(_root: &Path, _project: Option<&Path>) -> Result<()> {
    let units: Vec<String> = systemd::list_fsn_units().await?;

    if units.is_empty() {
        println!("No FSN-managed services found.");
        return Ok(());
    }

    println!("{:<30} {}", "SERVICE", "STATE");
    println!("{}", "─".repeat(42));

    for unit in &units {
        let name = unit.trim_end_matches(".service");
        let state = match systemd::status(name).await {
            Ok(UnitStatus::Active)   => "active",
            Ok(UnitStatus::Inactive) => "inactive",
            Ok(UnitStatus::Failed)   => "FAILED",
            Ok(UnitStatus::NotFound) => "not-found",
            Err(_)                   => "error",
        };
        println!("{:<30} {}", name, state);
    }

    Ok(())
}
