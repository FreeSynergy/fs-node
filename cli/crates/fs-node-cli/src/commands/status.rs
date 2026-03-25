use anyhow::Result;
use fs_container::SystemctlManager;
use std::path::Path;

/// Print the systemd state of all FSN-managed services.
pub async fn run(_root: &Path, _project: Option<&Path>) -> Result<()> {
    let systemd = SystemctlManager::user();
    let units = fs_deploy::observe::list_fs_units(&systemd).await?;

    if units.is_empty() {
        println!("{}", fs_i18n::t("status.no-services"));
        return Ok(());
    }

    println!(
        "{:<30} {}",
        fs_i18n::t("status.header-service"),
        fs_i18n::t("status.header-state")
    );
    println!("{}", "─".repeat(42));

    for unit in &units {
        let name = unit.trim_end_matches(".service");
        let state = match systemd.service_status(unit).await {
            Ok(s) => s.active_state.to_string(),
            Err(_) => "error".to_string(),
        };
        println!("{:<30} {}", name, state);
    }

    Ok(())
}
