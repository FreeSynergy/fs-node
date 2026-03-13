use std::path::Path;
use anyhow::{bail, Result};
use fsy_container::SystemdManager;
use fsn_engine::deploy::{DeployOpts, undeploy_instance};

/// Remove one or all deployed services (stops units, deletes Quadlet files).
pub async fn run(_root: &Path, _project: Option<&Path>, service: Option<&str>, confirm: bool) -> Result<()> {
    if !confirm {
        bail!(
            "Remove deletes ALL data for {}. Re-run with --confirm to proceed.",
            service.unwrap_or("all services")
        );
    }
    let opts = DeployOpts::default_for_user();
    if let Some(name) = service {
        undeploy_instance(name, &opts).await?;
        println!("Removed {}", name);
    } else {
        let systemd = SystemdManager::new();
        let units = fsn_engine::observe::list_fsn_units(&systemd).await?;
        for unit in &units {
            let name = unit.trim_end_matches(".service");
            undeploy_instance(name, &opts).await?;
            println!("Removed {}", name);
        }
    }
    Ok(())
}
