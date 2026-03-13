use std::path::Path;
use anyhow::Result;
use fsy_container::SystemdManager;
use fsn_engine::deploy::{DeployOpts, undeploy_instance};

/// Stop and remove Quadlet files for one or all services.
pub async fn run(_root: &Path, _project: Option<&Path>, service: Option<&str>) -> Result<()> {
    let opts = DeployOpts::default_for_user();
    if let Some(name) = service {
        undeploy_instance(name, &opts).await?;
        println!("Undeployed {}", name);
    } else {
        let systemd = SystemdManager::new();
        let units = fsn_engine::observe::list_fsn_units(&systemd).await?;
        for unit in &units {
            let name = unit.trim_end_matches(".service");
            undeploy_instance(name, &opts).await?;
            println!("Undeployed {}", name);
        }
    }
    Ok(())
}
